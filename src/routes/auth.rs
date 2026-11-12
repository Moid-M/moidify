//! Auth routes — byte-compatible with Python `routes/auth.py`.
//!
//! Endpoints:
//!   GET  /api/version
//!   POST /api/auth/register
//!   POST /api/auth/login
//!   GET  /api/auth/me
//!   POST /api/auth/password
//!   POST /api/auth/logout
//!   POST /api/auth/logout-all
//!   GET  /api/setup/status
//!   POST /api/setup/init

use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Executor;
use std::time::Duration;

use crate::auth::extractors::{AuthUser, RawToken};
use crate::auth::password::{
    new_password_async, validate_email, validate_password, verify_password_async,
};
use crate::auth::session::{
    create_session, delete_all_sessions, delete_session, get_user_from_token, token_hash,
};
use crate::auth::{is_registration_open, store_subsonic_password};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/version", get(version))
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/me", get(me))
        .route("/api/auth/password", post(change_own_password))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/logout-all", post(logout_all))
        .route("/api/setup/status", get(setup_status))
        .route("/api/setup/init", post(setup_init))
}

// ── Bodies ──────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RegisterBody {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub email: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginBody {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct ChangeOwnPasswordBody {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Deserialize)]
pub struct SetupInitBody {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub music_dir: Option<String>,
    #[serde(default)]
    pub max_upload_size: Option<i64>,
}

// ── Handlers ────────────────────────────────────────────────────────────────

async fn version() -> Json<Value> {
    Json(json!({ "version": crate::config::Config::version() }))
}

async fn register(
    State(app): State<AppState>,
    Json(body): Json<RegisterBody>,
) -> AppResult<Json<Value>> {
    if !is_registration_open(&app.pool).await? {
        return Err(AppError::Forbidden("Registration is disabled".into()));
    }
    if !app.rate.check(
        &format!("register:{}", body.username),
        3,
        Duration::from_secs(3600),
    ) {
        return Err(AppError::TooManyRequests(
            "Too many registration attempts. Try again later.".into(),
        ));
    }
    validate_password(&body.password).map_err(AppError::BadRequest)?;
    if body.username.is_empty() {
        return Err(AppError::BadRequest("Username is required".into()));
    }
    if let Some(email) = body.email.as_deref() {
        if !email.trim().is_empty() {
            validate_email(email).map_err(AppError::BadRequest)?;
        }
    }

    let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM users WHERE username = ?")
        .bind(&body.username)
        .fetch_optional(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    if existing.is_some() {
        return Err(AppError::BadRequest("Username already taken".into()));
    }

    let (pwd_hash, salt) = new_password_async(body.password.clone()).await;
    let email = body
        .email
        .as_deref()
        .map(|e| e.trim())
        .filter(|e| !e.is_empty());

    let mut tx = app
        .pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let result =
        sqlx::query("INSERT INTO users (username, email, password_hash, salt) VALUES (?, ?, ?, ?)")
            .bind(&body.username)
            .bind(email)
            .bind(&pwd_hash)
            .bind(&salt)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    let user_id = result.last_insert_rowid();
    store_subsonic_password_tx(&mut tx, &app.fernet, user_id, &body.password).await?;
    tx.commit()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    Ok(Json(json!({ "ok": true })))
}

async fn login(State(app): State<AppState>, Json(body): Json<LoginBody>) -> AppResult<Json<Value>> {
    if !app.rate.check(
        &format!("login:{}", body.username),
        10,
        Duration::from_secs(900),
    ) {
        return Err(AppError::TooManyRequests(
            "Too many login attempts. Try again later.".into(),
        ));
    }

    // Account lock check (raises 429 if locked).
    check_account_locked(&app, &body.username).await?;

    let row = sqlx::query_as::<_, (i64, String, String, String, Option<String>, Option<String>)>(
        "SELECT id, username, password_hash, salt, email, password_encrypted FROM users WHERE username = ?",
    )
    .bind(&body.username)
    .fetch_optional(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let Some((id, username, password_hash, salt, email, password_encrypted)) = row else {
        return Err(AppError::Unauthorized("Invalid credentials".into()));
    };
    if salt.is_empty() {
        return Err(AppError::Unauthorized("Invalid credentials".into()));
    }

    if !verify_password_async(body.password.clone(), salt.clone(), password_hash.clone()).await {
        increment_failed_attempts(&app, &body.username).await?;
        return Err(AppError::Unauthorized("Invalid credentials".into()));
    }
    reset_failed_attempts(&app, &body.username).await?;

    // Backfill encrypted copy for Subsonic token auth if missing.
    if password_encrypted.is_none() {
        store_subsonic_password(&app.pool, &app.fernet, id, &body.password).await?;
    }

    let token = create_session(&app.pool, id)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({
        "token": token,
        "user": { "id": id, "username": username, "email": email },
    })))
}

async fn me(State(app): State<AppState>, headers: HeaderMap) -> AppResult<Json<Value>> {
    let raw = header_token(&headers);
    let user = match raw {
        Some(t) => get_user_from_token(&app.pool, &t)
            .await
            .map_err(|e| AppError::Internal(e.into()))?,
        None => None,
    };
    match user {
        Some(u) => Ok(Json(serde_json::to_value(&u).unwrap_or_default())),
        None => Err(AppError::Unauthorized("Not logged in".into())),
    }
}

async fn change_own_password(
    State(app): State<AppState>,
    AuthUser(user): AuthUser,
    RawToken(raw): RawToken,
    Json(body): Json<ChangeOwnPasswordBody>,
) -> AppResult<Json<Value>> {
    let raw = raw.ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let row =
        sqlx::query_as::<_, (String, String)>("SELECT password_hash, salt FROM users WHERE id = ?")
            .bind(user.id)
            .fetch_optional(&app.pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;

    let Some((password_hash, salt)) = row else {
        return Err(AppError::NotFound("User not found".into()));
    };

    if !verify_password_async(
        body.current_password.clone(),
        salt.clone(),
        password_hash.clone(),
    )
    .await
    {
        return Err(AppError::BadRequest("Current password is incorrect".into()));
    }
    validate_password(&body.new_password).map_err(AppError::BadRequest)?;

    let (new_hash, new_salt) = new_password_async(body.new_password.clone()).await;
    let mut tx = app
        .pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    sqlx::query("UPDATE users SET password_hash = ?, salt = ? WHERE id = ?")
        .bind(&new_hash)
        .bind(&new_salt)
        .bind(user.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    store_subsonic_password_tx(&mut tx, &app.fernet, user.id, &body.new_password).await?;
    // Revoke all other sessions (keep current).
    let keep = token_hash(&raw);
    sqlx::query("DELETE FROM sessions WHERE user_id = ? AND token_hash != ?")
        .bind(user.id)
        .bind(&keep)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    Ok(Json(json!({ "ok": true })))
}

async fn logout(State(app): State<AppState>, headers: HeaderMap) -> AppResult<Json<Value>> {
    let raw = header_token(&headers);
    let user = match raw {
        Some(ref t) => get_user_from_token(&app.pool, t)
            .await
            .map_err(|e| AppError::Internal(e.into()))?,
        None => None,
    };
    if user.is_none() {
        return Err(AppError::Unauthorized("Not logged in".into()));
    }
    if let Some(t) = raw {
        delete_session(&app.pool, &t)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    }
    Ok(Json(json!({ "ok": true })))
}

async fn logout_all(
    State(app): State<AppState>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    delete_all_sessions(&app.pool, user.id)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(
        json!({ "ok": true, "message": "All sessions revoked" }),
    ))
}

async fn setup_status(State(app): State<AppState>) -> AppResult<Json<Value>> {
    let (admin_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE is_admin = 1")
        .fetch_one(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({
        "setup_needed": admin_count == 0,
        "has_admin": admin_count > 0,
    })))
}

async fn setup_init(
    State(app): State<AppState>,
    Json(body): Json<SetupInitBody>,
) -> AppResult<Json<Value>> {
    validate_password(&body.password).map_err(AppError::BadRequest)?;

    // Mirror Python's BEGIN IMMEDIATE so concurrent setup can't race.
    let mut conn = app
        .pool
        .acquire()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    conn.execute("BEGIN IMMEDIATE")
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let result: AppResult<i64> = async {
        let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM users WHERE username = ?")
            .bind(&body.username)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        if existing.is_some() {
            return Err(AppError::BadRequest("Username already taken".into()));
        }
        let (admin_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM users WHERE is_admin = 1")
                .fetch_one(&mut *conn)
                .await
                .map_err(|e| AppError::Internal(e.into()))?;
        if admin_count > 0 {
            return Err(AppError::BadRequest("Setup already completed".into()));
        }

        if let Some(dir) = body.music_dir.as_deref().filter(|s| !s.is_empty()) {
            sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES ('music_dir', ?)")
                .bind(dir)
                .execute(&mut *conn)
                .await
                .map_err(|e| AppError::Internal(e.into()))?;
        }
        if let Some(size) = body.max_upload_size {
            sqlx::query(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('max_upload_size', ?)",
            )
            .bind(size.to_string())
            .execute(&mut *conn)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        }

        let (pwd_hash, salt) = new_password_async(body.password.clone()).await;
        let res = sqlx::query(
            "INSERT INTO users (username, password_hash, salt, is_admin) VALUES (?, ?, ?, 1)",
        )
        .bind(&body.username)
        .bind(&pwd_hash)
        .bind(&salt)
        .execute(&mut *conn)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
        let user_id = res.last_insert_rowid();

        let enc = app.fernet.encrypt(body.password.as_bytes());
        let enc = String::from_utf8(enc).map_err(|e| AppError::Internal(e.into()))?;
        sqlx::query("UPDATE users SET password_encrypted = ? WHERE id = ?")
            .bind(&enc)
            .bind(user_id)
            .execute(&mut *conn)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;

        Ok(user_id)
    }
    .await;

    let user_id = match result {
        Ok(id) => {
            conn.execute("COMMIT")
                .await
                .map_err(|e| AppError::Internal(e.into()))?;
            id
        }
        Err(e) => {
            let _ = conn.execute("ROLLBACK").await;
            return Err(e);
        }
    };
    drop(conn);

    let token = create_session(&app.pool, user_id)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({
        "token": token,
        "user": { "id": user_id, "username": body.username },
    })))
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn header_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("token")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

async fn store_subsonic_password_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    fernet: &crate::auth::fernet::Fernet,
    user_id: i64,
    password: &str,
) -> AppResult<()> {
    let enc = fernet.encrypt(password.as_bytes());
    let enc = String::from_utf8(enc).map_err(|e| AppError::Internal(e.into()))?;
    sqlx::query("UPDATE users SET password_encrypted = ? WHERE id = ?")
        .bind(&enc)
        .bind(user_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(())
}

/// Mirror Python `_check_account_locked` — raises 429 with remaining seconds.
async fn check_account_locked(app: &AppState, username: &str) -> AppResult<()> {
    let row: Option<(Option<i64>, Option<i64>)> =
        sqlx::query_as("SELECT failed_attempts, locked_until FROM users WHERE username = ?")
            .bind(username)
            .fetch_optional(&app.pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    if let Some((_attempts, Some(until))) = row {
        let now = chrono::Utc::now().timestamp();
        if now < until {
            let remaining = until - now;
            return Err(AppError::TooManyRequests(format!(
                "Account locked. Try again in {remaining} seconds."
            )));
        }
    }
    Ok(())
}

/// Mirror Python `_increment_failed_attempts` with exponential lockout.
async fn increment_failed_attempts(app: &AppState, username: &str) -> AppResult<()> {
    let row: Option<(i64, Option<i64>)> =
        sqlx::query_as("SELECT id, failed_attempts FROM users WHERE username = ?")
            .bind(username)
            .fetch_optional(&app.pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    if let Some((id, attempts)) = row {
        let attempts = (attempts.unwrap_or(0)) + 1;
        if attempts >= 5 {
            // 30m, 1h, 2h, 4h, … max 24h
            let lock_duration = std::cmp::min(30 * 60 * 2i64.pow((attempts - 5) as u32), 86_400);
            let locked_until = chrono::Utc::now().timestamp() + lock_duration;
            sqlx::query("UPDATE users SET failed_attempts = ?, locked_until = ? WHERE id = ?")
                .bind(attempts)
                .bind(locked_until)
                .bind(id)
                .execute(&app.pool)
                .await
                .map_err(|e| AppError::Internal(e.into()))?;
        } else {
            sqlx::query("UPDATE users SET failed_attempts = ? WHERE id = ?")
                .bind(attempts)
                .bind(id)
                .execute(&app.pool)
                .await
                .map_err(|e| AppError::Internal(e.into()))?;
        }
    }
    Ok(())
}

async fn reset_failed_attempts(app: &AppState, username: &str) -> AppResult<()> {
    sqlx::query("UPDATE users SET failed_attempts = 0, locked_until = NULL WHERE username = ?")
        .bind(username)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(())
}
