//! Admin user management: list, create, toggle admin, password reset, delete,
//! unlock, sessions, failed logins, registration status/toggle.

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::extractors::AdminUser;
use crate::auth::password::{new_password_async, validate_password};
use crate::auth::store_subsonic_password;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// Row shape for the user list (clippy type_complexity).
type UserListRow = (i64, String, Option<String>, i64, Option<String>);

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/admin/users", get(list_users).post(create_user))
        .route("/api/admin/users/{user_id}/admin", post(toggle_admin))
        .route("/api/admin/users/{user_id}/password", post(change_password))
        .route(
            "/api/admin/users/{user_id}",
            axum::routing::delete(delete_user),
        )
        .route("/api/admin/users/{user_id}/unlock", post(unlock_user))
        .route(
            "/api/admin/users/{user_id}/sessions",
            get(list_sessions).delete(revoke_sessions),
        )
        .route("/api/admin/failed-logins", get(failed_logins))
        .route("/api/admin/registration-status", get(registration_status))
        .route("/api/admin/registration-toggle", post(registration_toggle))
}

#[derive(Deserialize)]
struct ToggleAdminBody {
    is_admin: bool,
}

#[derive(Deserialize)]
struct CreateUserBody {
    username: String,
    password: String,
    #[serde(default)]
    is_admin: bool,
}

#[derive(Deserialize)]
struct ChangePasswordBody {
    password: String,
}

/// Python `admin_list_users`.
async fn list_users(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
) -> AppResult<Json<Value>> {
    let rows: Vec<UserListRow> = sqlx::query_as(
        "SELECT id, username, email, is_admin, created_at FROM users ORDER BY username",
    )
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    let users: Vec<Value> = rows
        .into_iter()
        .map(|(id, username, email, is_admin, created_at)| {
            json!({
                "id": id,
                "username": username,
                "email": email,
                // Python sqlite returns the INTEGER column as 0/1 — the admin
                // UI reads `data-admin === '1'`, so keep the integer.
                "is_admin": is_admin,
                "created_at": created_at,
            })
        })
        .collect();
    Ok(Json(users.into()))
}

/// Python `admin_toggle_user` — root `admin` account is immutable.
async fn toggle_admin(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
    Path(user_id): Path<i64>,
    Json(body): Json<ToggleAdminBody>,
) -> AppResult<Json<Value>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT username FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let Some((username,)) = row else {
        return Err(AppError::NotFound("User not found".into()));
    };
    if username == "admin" {
        return Err(AppError::BadRequest(
            "Cannot change the root admin account".into(),
        ));
    }
    sqlx::query("UPDATE users SET is_admin = ? WHERE id = ?")
        .bind(if body.is_admin { 1 } else { 0 })
        .bind(user_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({
        "ok": true,
        "username": username,
        "is_admin": body.is_admin,
    })))
}

/// Python `admin_create_user` — validates policy, stores PBKDF2 + Fernet copy.
async fn create_user(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
    Json(body): Json<CreateUserBody>,
) -> AppResult<Json<Value>> {
    validate_password(&body.password).map_err(AppError::BadRequest)?;
    let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM users WHERE username = ?")
        .bind(&body.username)
        .fetch_optional(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    if existing.is_some() {
        return Err(AppError::BadRequest("Username already taken".into()));
    }
    let (pwd_hash, salt) = new_password_async(body.password.clone()).await;
    let result = sqlx::query(
        "INSERT INTO users (username, password_hash, salt, is_admin) VALUES (?, ?, ?, ?)",
    )
    .bind(&body.username)
    .bind(&pwd_hash)
    .bind(&salt)
    .bind(if body.is_admin { 1 } else { 0 })
    .execute(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    let user_id = result.last_insert_rowid();
    store_subsonic_password(&app.pool, &app.fernet, user_id, &body.password).await?;
    Ok(Json(json!({
        "ok": true,
        "username": body.username,
        "is_admin": body.is_admin,
    })))
}

/// Python `admin_change_password`.
async fn change_password(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
    Path(user_id): Path<i64>,
    Json(body): Json<ChangePasswordBody>,
) -> AppResult<Json<Value>> {
    validate_password(&body.password).map_err(AppError::BadRequest)?;
    let row: Option<(String,)> = sqlx::query_as("SELECT username FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let Some((username,)) = row else {
        return Err(AppError::NotFound("User not found".into()));
    };
    let (pwd_hash, salt) = new_password_async(body.password.clone()).await;
    sqlx::query("UPDATE users SET password_hash = ?, salt = ? WHERE id = ?")
        .bind(&pwd_hash)
        .bind(&salt)
        .bind(user_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    store_subsonic_password(&app.pool, &app.fernet, user_id, &body.password).await?;
    Ok(Json(json!({ "ok": true, "username": username })))
}

/// Python `admin_delete_user` — cascades sessions/favorites/shares/playlists.
async fn delete_user(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
    Path(user_id): Path<i64>,
) -> AppResult<Json<Value>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT username FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let Some((username,)) = row else {
        return Err(AppError::NotFound("User not found".into()));
    };
    if username == "admin" {
        return Err(AppError::BadRequest(
            "Cannot delete the root admin account".into(),
        ));
    }
    let mut tx = app
        .pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    for stmt in [
        "DELETE FROM sessions WHERE user_id = ?",
        "DELETE FROM favorites WHERE user_id = ?",
        "DELETE FROM shared_albums WHERE user_id = ?",
        "DELETE FROM shared_playlists WHERE playlist_id IN (SELECT id FROM playlists WHERE user_id = ?)",
        "DELETE FROM playlist_tracks WHERE playlist_id IN (SELECT id FROM playlists WHERE user_id = ?)",
        "DELETE FROM playlists WHERE user_id = ?",
        "DELETE FROM users WHERE id = ?",
    ] {
        sqlx::query(stmt)
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    }
    tx.commit()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({ "ok": true, "username": username })))
}

/// Python `admin_unlock_user` — clears failed_attempts + lockout.
async fn unlock_user(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
    Path(user_id): Path<i64>,
) -> AppResult<Json<Value>> {
    sqlx::query("UPDATE users SET failed_attempts = 0, locked_until = NULL WHERE id = ?")
        .bind(user_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({ "ok": true })))
}

/// Python `admin_user_sessions` — stores `token_hash` in both `token` and
/// `token_hash` columns (same as Python `_create_session`), so the JSON
/// `token` field is the SHA-256 hash, never the plaintext.
async fn list_sessions(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
    Path(user_id): Path<i64>,
) -> AppResult<Json<Value>> {
    let rows: Vec<(String, i64, Option<String>, Option<i64>)> = sqlx::query_as(
        "SELECT token, user_id, created_at, expires_at FROM sessions WHERE user_id = ? ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    let sessions: Vec<Value> = rows
        .into_iter()
        .map(|(token, uid, created_at, expires_at)| {
            json!({
                "token": token,
                "user_id": uid,
                "created_at": created_at,
                "expires_at": expires_at,
            })
        })
        .collect();
    Ok(Json(sessions.into()))
}

/// Python `admin_revoke_user_sessions`.
async fn revoke_sessions(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
    Path(user_id): Path<i64>,
) -> AppResult<Json<Value>> {
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(user_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(
        json!({ "ok": true, "message": "All sessions revoked" }),
    ))
}

/// Python `admin_failed_logins` — `locked` is true only when still locked.
async fn failed_logins(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
) -> AppResult<Json<Value>> {
    let rows: Vec<(i64, String, Option<i64>, Option<i64>)> = sqlx::query_as(
        "SELECT id, username, failed_attempts, locked_until FROM users WHERE failed_attempts > 0 OR locked_until IS NOT NULL ORDER BY failed_attempts DESC",
    )
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    let now = chrono::Utc::now().timestamp();
    let out: Vec<Value> = rows
        .into_iter()
        .map(|(id, username, failed_attempts, locked_until)| {
            json!({
                "id": id,
                "username": username,
                "failed_attempts": failed_attempts.unwrap_or(0),
                "locked_until": locked_until,
                "locked": locked_until.map(|u| u > now).unwrap_or(false),
            })
        })
        .collect();
    Ok(Json(out.into()))
}

/// Python `admin_registration_status`.
async fn registration_status(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
) -> AppResult<Json<Value>> {
    let open = crate::auth::is_registration_open(&app.pool).await?;
    Ok(Json(json!({ "registration_open": open })))
}

/// Python `admin_toggle_registration` — closed ("0") → open ("1"),
/// open/unset → closed ("0").
async fn registration_toggle(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
) -> AppResult<Json<Value>> {
    let current: Option<(String,)> =
        sqlx::query_as("SELECT value FROM settings WHERE key = 'registration_open'")
            .fetch_optional(&app.pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    let new_val = if current.as_ref().is_some_and(|(v,)| v == "0") {
        "1"
    } else {
        "0"
    };
    sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES ('registration_open', ?)")
        .bind(new_val)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({ "registration_open": new_val == "1" })))
}
