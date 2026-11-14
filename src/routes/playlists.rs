//! Playlists, folders, favorites, shares, play counter, export/import.
//! Byte-compatible with Python `routes/playlists.py`.

use axum::extract::{Multipart, Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;

use crate::auth::extractors::{AuthUser, OptionalUser};
use crate::db::models::User;
use crate::error::{AppError, AppResult};
use crate::routes::tracks::row_to_track;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/playlists", get(list_playlists).post(create_playlist))
        .route(
            "/api/playlists/{playlist_id}",
            axum::routing::delete(delete_playlist),
        )
        .route(
            "/api/playlists/{playlist_id}/tracks",
            get(get_playlist_tracks).post(add_to_playlist),
        )
        .route(
            "/api/playlists/{playlist_id}/tracks/{track_id}",
            axum::routing::delete(remove_from_playlist),
        )
        .route(
            "/api/playlists/{playlist_id}/tracks/reorder",
            axum::routing::put(reorder_playlist_tracks),
        )
        .route(
            "/api/playlists/{playlist_id}/share",
            post(share_playlist)
                .get(get_share_status)
                .delete(unshare_playlist),
        )
        .route("/api/shared/{token}", get(get_shared_playlist))
        .route(
            "/api/playlists/{playlist_id}/folder",
            axum::routing::put(set_playlist_folder),
        )
        .route("/api/playlists/{playlist_id}/export", get(export_playlist))
        .route("/api/playlists/import", post(import_playlist))
        .route("/api/albums/share", post(share_album).delete(unshare_album))
        .route("/api/album-shared/{token}", get(get_shared_album))
        .route("/api/albums/share-status", get(album_share_status))
        .route(
            "/api/playlist-folders",
            get(list_folders).post(create_folder),
        )
        .route(
            "/api/playlist-folders/{folder_id}",
            axum::routing::put(rename_folder).delete(delete_folder),
        )
        .route("/api/favorites", get(get_favorites))
        .route(
            "/api/favorites/{track_id}",
            post(add_favorite).delete(remove_favorite),
        )
        .route("/api/favorites/check/{track_id}", get(check_favorite))
        .route("/api/play/{track_id}", post(increment_play_count))
}

// ── Bodies ───────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct NameBody {
    name: String,
}

#[derive(Deserialize)]
struct AddTrackBody {
    track_id: i64,
}

#[derive(Deserialize)]
struct ReorderBody {
    order: Vec<i64>,
}

#[derive(Deserialize)]
struct SetFolderBody {
    folder_id: Option<i64>,
}

#[derive(Deserialize)]
struct ShareAlbumBody {
    album: String,
    artist: Option<String>,
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Python: playlist not found OR not owned → 404 (never leaks existence).
async fn require_playlist_owner(
    pool: &sqlx::SqlitePool,
    playlist_id: i64,
    user_id: i64,
) -> AppResult<()> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM playlists WHERE id = ? AND user_id = ?")
            .bind(playlist_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    if row.is_none() {
        return Err(AppError::NotFound("Playlist not found".into()));
    }
    Ok(())
}

/// `secrets.token_urlsafe(16)` equivalent — URL-safe base64, no padding.
fn share_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, bytes)
}

/// Python `_safe_name` for Content-Disposition filenames.
fn safe_name(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

// ── Playlists CRUD ───────────────────────────────────────────────────────────

async fn list_playlists(
    State(app): State<AppState>,
    OptionalUser(user): OptionalUser,
) -> AppResult<Json<Value>> {
    let Some(user) = user else {
        return Ok(Json(json!([])));
    };
    let rows = sqlx::query(
        r#"SELECT p.*, COUNT(pt.id) as track_count
           FROM playlists p
           LEFT JOIN playlist_tracks pt ON p.id = pt.playlist_id
           WHERE p.user_id = ?
           GROUP BY p.id
           ORDER BY p.name"#,
    )
    .bind(user.id)
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<i64, _>("id"),
                "name": r.get::<String, _>("name"),
                "user_id": r.get::<i64, _>("user_id"),
                "folder_id": r.get::<Option<i64>, _>("folder_id"),
                "created_at": r.get::<Option<String>, _>("created_at"),
                "track_count": r.get::<i64, _>("track_count"),
            })
        })
        .collect();
    Ok(Json(Value::Array(out)))
}

async fn create_playlist(
    State(app): State<AppState>,
    AuthUser(user): AuthUser,
    Json(body): Json<NameBody>,
) -> AppResult<Json<Value>> {
    let result = sqlx::query("INSERT INTO playlists (name, user_id) VALUES (?, ?)")
        .bind(&body.name)
        .bind(user.id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({
        "id": result.last_insert_rowid(),
        "name": body.name,
    })))
}

async fn delete_playlist(
    State(app): State<AppState>,
    AuthUser(user): AuthUser,
    Path(playlist_id): Path<i64>,
) -> AppResult<Json<Value>> {
    require_playlist_owner(&app.pool, playlist_id, user.id).await?;
    sqlx::query("DELETE FROM shared_playlists WHERE playlist_id = ?")
        .bind(playlist_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    sqlx::query("DELETE FROM playlist_tracks WHERE playlist_id = ?")
        .bind(playlist_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    sqlx::query("DELETE FROM playlists WHERE id = ?")
        .bind(playlist_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({ "ok": true })))
}

// ── Playlist tracks ──────────────────────────────────────────────────────────

async fn get_playlist_tracks(
    State(app): State<AppState>,
    Path(playlist_id): Path<i64>,
) -> AppResult<Json<Value>> {
    // Public read (Python has no auth here).
    let rows = sqlx::query(
        r#"SELECT t.*, pt.position, pt.added_at
           FROM playlist_tracks pt
           JOIN tracks t ON pt.track_id = t.id
           WHERE pt.playlist_id = ?
           ORDER BY pt.position"#,
    )
    .bind(playlist_id)
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            let mut v = row_to_track(r);
            v["position"] = json!(r.get::<i64, _>("position"));
            v["added_at"] = json!(r.get::<Option<String>, _>("added_at"));
            v
        })
        .collect();
    Ok(Json(Value::Array(out)))
}

async fn add_to_playlist(
    State(app): State<AppState>,
    AuthUser(user): AuthUser,
    Path(playlist_id): Path<i64>,
    Json(body): Json<AddTrackBody>,
) -> AppResult<Json<Value>> {
    require_playlist_owner(&app.pool, playlist_id, user.id).await?;
    let max_pos: Option<(Option<i64>,)> =
        sqlx::query_as("SELECT MAX(position) FROM playlist_tracks WHERE playlist_id = ?")
            .bind(playlist_id)
            .fetch_optional(&app.pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    let pos = max_pos.and_then(|(m,)| m).unwrap_or(0) + 1;
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)")
        .bind(playlist_id)
        .bind(body.track_id)
        .bind(pos)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({ "ok": true })))
}

async fn remove_from_playlist(
    State(app): State<AppState>,
    AuthUser(user): AuthUser,
    Path((playlist_id, track_id)): Path<(i64, i64)>,
) -> AppResult<Json<Value>> {
    require_playlist_owner(&app.pool, playlist_id, user.id).await?;
    sqlx::query("DELETE FROM playlist_tracks WHERE playlist_id = ? AND track_id = ?")
        .bind(playlist_id)
        .bind(track_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({ "ok": true })))
}

async fn reorder_playlist_tracks(
    State(app): State<AppState>,
    AuthUser(user): AuthUser,
    Path(playlist_id): Path<i64>,
    Json(body): Json<ReorderBody>,
) -> AppResult<Json<Value>> {
    require_playlist_owner(&app.pool, playlist_id, user.id).await?;
    for (i, track_id) in body.order.iter().enumerate() {
        sqlx::query(
            "UPDATE playlist_tracks SET position = ? WHERE playlist_id = ? AND track_id = ?",
        )
        .bind((i + 1) as i64)
        .bind(playlist_id)
        .bind(track_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    }
    Ok(Json(json!({ "ok": true })))
}

// ── Shared playlists ─────────────────────────────────────────────────────────

async fn share_playlist(
    State(app): State<AppState>,
    AuthUser(user): AuthUser,
    Path(playlist_id): Path<i64>,
) -> AppResult<Json<Value>> {
    require_playlist_owner(&app.pool, playlist_id, user.id).await?;
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT token FROM shared_playlists WHERE playlist_id = ?")
            .bind(playlist_id)
            .fetch_optional(&app.pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    if let Some((token,)) = existing {
        return Ok(Json(json!({ "token": token })));
    }
    let token = share_token();
    sqlx::query("INSERT INTO shared_playlists (token, playlist_id) VALUES (?, ?)")
        .bind(&token)
        .bind(playlist_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({ "token": token })))
}

async fn get_share_status(
    State(app): State<AppState>,
    AuthUser(user): AuthUser,
    Path(playlist_id): Path<i64>,
) -> AppResult<Json<Value>> {
    require_playlist_owner(&app.pool, playlist_id, user.id).await?;
    let row: Option<(String,)> =
        sqlx::query_as("SELECT token FROM shared_playlists WHERE playlist_id = ?")
            .bind(playlist_id)
            .fetch_optional(&app.pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(match row {
        Some((token,)) => json!({ "shared": true, "token": token }),
        None => json!({ "shared": false, "token": null }),
    }))
}

async fn unshare_playlist(
    State(app): State<AppState>,
    AuthUser(user): AuthUser,
    Path(playlist_id): Path<i64>,
) -> AppResult<Json<Value>> {
    require_playlist_owner(&app.pool, playlist_id, user.id).await?;
    sqlx::query("DELETE FROM shared_playlists WHERE playlist_id = ?")
        .bind(playlist_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({ "ok": true })))
}

async fn get_shared_playlist(
    State(app): State<AppState>,
    Path(token): Path<String>,
) -> AppResult<Json<Value>> {
    let row = sqlx::query(
        r#"SELECT p.id, p.name, p.user_id, u.username
           FROM shared_playlists sp
           JOIN playlists p ON sp.playlist_id = p.id
           JOIN users u ON p.user_id = u.id
           WHERE sp.token = ?"#,
    )
    .bind(&token)
    .fetch_optional(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    let Some(row) = row else {
        return Err(AppError::NotFound("Shared playlist not found".into()));
    };
    let playlist_id: i64 = row.get("id");
    let name: String = row.get("name");
    let username: String = row.get("username");

    let tracks = sqlx::query(
        r#"SELECT t.*, pt.position
           FROM playlist_tracks pt
           JOIN tracks t ON pt.track_id = t.id
           WHERE pt.playlist_id = ?
           ORDER BY pt.position"#,
    )
    .bind(playlist_id)
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let out: Vec<Value> = tracks
        .iter()
        .map(|r| {
            let mut v = row_to_track(r);
            v["position"] = json!(r.get::<i64, _>("position"));
            v
        })
        .collect();

    Ok(Json(
        json!({ "name": name, "username": username, "tracks": out }),
    ))
}

// ── Shared albums ────────────────────────────────────────────────────────────

/// Python `artist = body.artist.strip() or None if body.artist else None`.
fn normalize_album_artist(artist: &Option<String>) -> Option<String> {
    artist
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

async fn share_album(
    State(app): State<AppState>,
    AuthUser(user): AuthUser,
    Json(body): Json<ShareAlbumBody>,
) -> AppResult<Json<Value>> {
    let album = body.album.trim().to_string();
    let artist = normalize_album_artist(&body.artist);
    if album.is_empty() {
        return Err(AppError::BadRequest("Album name required".into()));
    }

    let existing: Option<(String,)> = sqlx::query_as(
        "SELECT token FROM shared_albums WHERE album = ? AND (artist IS ? OR artist = ?)",
    )
    .bind(&album)
    .bind(&artist)
    .bind(&artist)
    .fetch_optional(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    if let Some((token,)) = existing {
        return Ok(Json(json!({ "token": token })));
    }

    let token = share_token();
    sqlx::query("INSERT INTO shared_albums (token, album, artist, user_id) VALUES (?, ?, ?, ?)")
        .bind(&token)
        .bind(&album)
        .bind(&artist)
        .bind(user.id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({ "token": token })))
}

async fn get_shared_album(
    State(app): State<AppState>,
    Path(token): Path<String>,
) -> AppResult<Json<Value>> {
    let row = sqlx::query("SELECT album, artist, user_id FROM shared_albums WHERE token = ?")
        .bind(&token)
        .fetch_optional(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let Some(row) = row else {
        return Err(AppError::NotFound("Shared album not found".into()));
    };
    let album: Option<String> = row.get("album");
    let artist: Option<String> = row.get("artist");

    let tracks = if let Some(ref a) = artist {
        sqlx::query(
            "SELECT * FROM tracks WHERE album = ? AND artist = ? ORDER BY disc_number, track_number",
        )
        .bind(album.as_deref().unwrap_or(""))
        .bind(a)
        .fetch_all(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    } else {
        sqlx::query("SELECT * FROM tracks WHERE album = ? ORDER BY disc_number, track_number")
            .bind(album.as_deref().unwrap_or(""))
            .fetch_all(&app.pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?
    };

    let out: Vec<Value> = tracks.iter().map(row_to_track).collect();
    Ok(Json(
        json!({ "album": album, "artist": artist, "tracks": out }),
    ))
}

async fn unshare_album(
    State(app): State<AppState>,
    AuthUser(_user): AuthUser,
    Json(body): Json<ShareAlbumBody>,
) -> AppResult<Json<Value>> {
    let album = body.album.trim().to_string();
    let artist = normalize_album_artist(&body.artist);
    if album.is_empty() {
        return Err(AppError::BadRequest("Album name required".into()));
    }
    sqlx::query("DELETE FROM shared_albums WHERE album = ? AND (artist IS ? OR artist = ?)")
        .bind(&album)
        .bind(&artist)
        .bind(&artist)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct ShareStatusQuery {
    album: String,
    artist: Option<String>,
}

async fn album_share_status(
    State(app): State<AppState>,
    AuthUser(_user): AuthUser,
    Query(q): Query<ShareStatusQuery>,
) -> AppResult<Json<Value>> {
    let album = q.album.trim().to_string();
    let artist = normalize_album_artist(&q.artist);
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT token FROM shared_albums WHERE album = ? AND (artist IS ? OR artist = ?)",
    )
    .bind(&album)
    .bind(&artist)
    .bind(&artist)
    .fetch_optional(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(match row {
        Some((token,)) => json!({ "shared": true, "token": token }),
        None => json!({ "shared": false, "token": null }),
    }))
}

// ── Folders ──────────────────────────────────────────────────────────────────

async fn list_folders(
    State(app): State<AppState>,
    OptionalUser(user): OptionalUser,
) -> AppResult<Json<Value>> {
    let Some(user) = user else {
        return Ok(Json(json!([])));
    };
    let rows = sqlx::query(
        r#"SELECT pf.*, COUNT(p.id) as playlist_count
           FROM playlist_folders pf
           LEFT JOIN playlists p ON p.folder_id = pf.id
           WHERE pf.user_id = ?
           GROUP BY pf.id
           ORDER BY pf.sort_order, pf.name"#,
    )
    .bind(user.id)
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<i64, _>("id"),
                "name": r.get::<String, _>("name"),
                "user_id": r.get::<i64, _>("user_id"),
                "sort_order": r.get::<Option<i64>, _>("sort_order"),
                "created_at": r.get::<Option<String>, _>("created_at"),
                "playlist_count": r.get::<i64, _>("playlist_count"),
            })
        })
        .collect();
    Ok(Json(Value::Array(out)))
}

async fn create_folder(
    State(app): State<AppState>,
    AuthUser(user): AuthUser,
    Json(body): Json<NameBody>,
) -> AppResult<Json<Value>> {
    let max_order: Option<(Option<i64>,)> = sqlx::query_as(
        "SELECT COALESCE(MAX(sort_order), -1) FROM playlist_folders WHERE user_id = ?",
    )
    .bind(user.id)
    .fetch_optional(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    let next = max_order.and_then(|(m,)| m).unwrap_or(-1) + 1;
    let result =
        sqlx::query("INSERT INTO playlist_folders (name, user_id, sort_order) VALUES (?, ?, ?)")
            .bind(&body.name)
            .bind(user.id)
            .bind(next)
            .execute(&app.pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({
        "id": result.last_insert_rowid(),
        "name": body.name,
    })))
}

async fn rename_folder(
    State(app): State<AppState>,
    AuthUser(user): AuthUser,
    Path(folder_id): Path<i64>,
    Json(body): Json<NameBody>,
) -> AppResult<Json<Value>> {
    require_folder_owner(&app.pool, folder_id, user.id).await?;
    sqlx::query("UPDATE playlist_folders SET name = ? WHERE id = ?")
        .bind(&body.name)
        .bind(folder_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({ "ok": true })))
}

async fn delete_folder(
    State(app): State<AppState>,
    AuthUser(user): AuthUser,
    Path(folder_id): Path<i64>,
) -> AppResult<Json<Value>> {
    require_folder_owner(&app.pool, folder_id, user.id).await?;
    sqlx::query("UPDATE playlists SET folder_id = NULL WHERE folder_id = ?")
        .bind(folder_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    sqlx::query("DELETE FROM playlist_folders WHERE id = ?")
        .bind(folder_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({ "ok": true })))
}

async fn require_folder_owner(
    pool: &sqlx::SqlitePool,
    folder_id: i64,
    user_id: i64,
) -> AppResult<()> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM playlist_folders WHERE id = ? AND user_id = ?")
            .bind(folder_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    if row.is_none() {
        return Err(AppError::NotFound("Folder not found".into()));
    }
    Ok(())
}

async fn set_playlist_folder(
    State(app): State<AppState>,
    AuthUser(user): AuthUser,
    Path(playlist_id): Path<i64>,
    Json(body): Json<SetFolderBody>,
) -> AppResult<Json<Value>> {
    require_playlist_owner(&app.pool, playlist_id, user.id).await?;
    sqlx::query("UPDATE playlists SET folder_id = ? WHERE id = ?")
        .bind(body.folder_id)
        .bind(playlist_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({ "ok": true })))
}

// ── Favorites ────────────────────────────────────────────────────────────────

async fn get_favorites(
    State(app): State<AppState>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    let rows = sqlx::query(
        r#"SELECT t.*, f.added_at
           FROM favorites f
           JOIN tracks t ON f.track_id = t.id
           WHERE f.user_id = ?
           ORDER BY f.added_at DESC"#,
    )
    .bind(user.id)
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            let mut v = row_to_track(r);
            v["added_at"] = json!(r.get::<Option<String>, _>("added_at"));
            v
        })
        .collect();
    Ok(Json(Value::Array(out)))
}

async fn add_favorite(
    State(app): State<AppState>,
    AuthUser(user): AuthUser,
    Path(track_id): Path<i64>,
) -> AppResult<Json<Value>> {
    sqlx::query("INSERT OR IGNORE INTO favorites (user_id, track_id) VALUES (?, ?)")
        .bind(user.id)
        .bind(track_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({ "ok": true })))
}

async fn remove_favorite(
    State(app): State<AppState>,
    AuthUser(user): AuthUser,
    Path(track_id): Path<i64>,
) -> AppResult<Json<Value>> {
    sqlx::query("DELETE FROM favorites WHERE user_id = ? AND track_id = ?")
        .bind(user.id)
        .bind(track_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({ "ok": true })))
}

async fn check_favorite(
    State(app): State<AppState>,
    OptionalUser(user): OptionalUser,
    Path(track_id): Path<i64>,
) -> AppResult<Json<Value>> {
    let Some(user) = user else {
        return Ok(Json(json!({ "favorite": false })));
    };
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT 1 FROM favorites WHERE user_id = ? AND track_id = ?")
            .bind(user.id)
            .bind(track_id)
            .fetch_optional(&app.pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({ "favorite": row.is_some() })))
}

// ── Play count ───────────────────────────────────────────────────────────────

async fn increment_play_count(
    State(app): State<AppState>,
    Path(track_id): Path<i64>,
) -> AppResult<Json<Value>> {
    // Anonymous by design (Python has no auth here).
    sqlx::query("UPDATE tracks SET play_count = COALESCE(play_count,0) + 1 WHERE id = ?")
        .bind(track_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    sqlx::query("INSERT INTO play_history (track_id) VALUES (?)")
        .bind(track_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({ "ok": true })))
}

// ── Export / import ──────────────────────────────────────────────────────────

fn build_m3u(tracks: &[Value], playlist_name: &str) -> String {
    let mut lines = vec!["#EXTM3U".to_string(), format!("#PLAYLIST: {playlist_name}")];
    for t in tracks {
        let dur = t["duration"].as_f64().map(|d| d as i64).unwrap_or(-1);
        let artist = t["artist"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or("Unknown");
        let title = t["title"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or("Unknown");
        lines.push(format!("#EXTINF:{dur},{artist} - {title}"));
        lines.push(t["file_path"].as_str().unwrap_or("").to_string());
    }
    lines.join("\n")
}

fn parse_m3u(content: &str) -> Vec<(String, String, String)> {
    // (artist, title, file_path)
    let mut entries = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        if let Some(info) = line.strip_prefix("#EXTINF:") {
            let dur_and_title: Vec<&str> = info.splitn(2, ',').collect();
            let artist_title = if dur_and_title.len() > 1 {
                dur_and_title[1]
            } else {
                ""
            };
            let (artist, title) = if let Some(idx) = artist_title.find(" - ") {
                (
                    artist_title[..idx].trim().to_string(),
                    artist_title[idx + 3..].trim().to_string(),
                )
            } else {
                (String::new(), artist_title.trim().to_string())
            };
            i += 1;
            while i < lines.len() && (lines[i].trim().is_empty() || lines[i].starts_with('#')) {
                i += 1;
            }
            let path = if i < lines.len() { lines[i].trim() } else { "" };
            entries.push((artist, title, path.to_string()));
        } else if !line.starts_with('#') && !line.is_empty() {
            entries.push((String::new(), String::new(), line.to_string()));
        }
        i += 1;
    }
    entries
}

#[derive(Deserialize)]
struct ExportQuery {
    format: Option<String>,
    /// UI uses `<a href>` (no headers) — accept `?token=` like the ZIP fix.
    token: Option<String>,
}

async fn export_playlist(
    State(app): State<AppState>,
    Path(playlist_id): Path<i64>,
    Query(q): Query<ExportQuery>,
    headers: HeaderMap,
) -> AppResult<Response> {
    // Header token wins; query token covers <a href> downloads.
    let raw = headers
        .get("token")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .or(q.token);
    let user = match raw {
        Some(t) => crate::auth::session::get_user_from_token(&app.pool, &t)
            .await
            .map_err(|e| AppError::Internal(e.into()))?,
        None => None,
    };
    let user: User = user.ok_or_else(|| AppError::Unauthorized("Login required".into()))?;

    require_playlist_owner(&app.pool, playlist_id, user.id).await?;

    let pl_row = sqlx::query("SELECT name FROM playlists WHERE id = ?")
        .bind(playlist_id)
        .fetch_optional(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::NotFound("Playlist not found".into()))?;
    let name: String = pl_row.get("name");

    let rows = sqlx::query(
        r#"SELECT t.* FROM playlist_tracks pt
           JOIN tracks t ON pt.track_id = t.id
           WHERE pt.playlist_id = ?
           ORDER BY pt.position"#,
    )
    .bind(playlist_id)
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    let tracks: Vec<Value> = rows.iter().map(row_to_track).collect();

    let format = q.format.unwrap_or_else(|| "m3u".to_string());
    let safe = safe_name(&name);

    if format == "json" {
        let payload = json!({
            "playlist_name": name,
            "format_version": 1,
            "tracks": tracks.iter().map(|t| json!({
                "title": t["title"],
                "artist": t["artist"],
                "album": t["album"],
                "duration": t["duration"],
                "file_path": t["file_path"],
            })).collect::<Vec<_>>(),
        });
        let body =
            serde_json::to_string_pretty(&payload).map_err(|e| AppError::Internal(e.into()))?;
        let mut resp = (StatusCode::OK, body).into_response();
        resp.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        resp.headers_mut().insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!("attachment; filename=\"{safe}.json\"")).unwrap(),
        );
        return Ok(resp);
    }

    let content = build_m3u(&tracks, &name);
    let mut resp = (StatusCode::OK, content).into_response();
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("audio/x-mpegurl"),
    );
    resp.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{safe}.m3u\"")).unwrap(),
    );
    Ok(resp)
}

async fn import_playlist(
    State(app): State<AppState>,
    AuthUser(user): AuthUser,
    mut multipart: Multipart,
) -> AppResult<Json<Value>> {
    let mut filename = "import".to_string();
    let mut bytes: Vec<u8> = Vec::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("multipart: {e}")))?
    {
        if field.name() == Some("file") {
            if let Some(name) = field.file_name() {
                filename = name.to_string();
            }
            bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::BadRequest(format!("read file: {e}")))?
                .to_vec();
            break;
        }
    }
    if bytes.is_empty() {
        return Err(AppError::BadRequest("No file uploaded".into()));
    }

    let text = String::from_utf8_lossy(&bytes).into_owned();
    let mut name_hint = filename
        .rsplit_once('.')
        .map(|(base, _)| base.to_string())
        .unwrap_or_else(|| filename.clone());

    // (artist, title, file_path)
    let entries: Vec<(String, String, String)> = if filename.ends_with(".json") {
        let data: Value = serde_json::from_str(&text)
            .map_err(|_| AppError::BadRequest("Invalid JSON file".into()))?;
        let Some(obj) = data.as_object() else {
            return Err(AppError::BadRequest("Invalid JSON format".into()));
        };
        let Some(tracks) = obj.get("tracks").and_then(|t| t.as_array()) else {
            return Err(AppError::BadRequest("Invalid JSON format".into()));
        };
        if let Some(n) = obj.get("playlist_name").and_then(|n| n.as_str()) {
            name_hint = n.to_string();
        }
        tracks
            .iter()
            .map(|t| {
                (
                    t.get("artist")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    t.get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    t.get("file_path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                )
            })
            .collect()
    } else {
        parse_m3u(&text)
    };

    if entries.is_empty() {
        return Err(AppError::BadRequest("No tracks found in file".into()));
    }

    let result = sqlx::query("INSERT INTO playlists (name, user_id) VALUES (?, ?)")
        .bind(&name_hint)
        .bind(user.id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let pid = result.last_insert_rowid();

    let mut matched: i64 = 0;
    let mut position: i64 = 0;
    for (artist, title, file_path) in &entries {
        let mut track_id: Option<i64> = None;
        if !file_path.is_empty() {
            let row: Option<(i64,)> = sqlx::query_as("SELECT id FROM tracks WHERE file_path = ?")
                .bind(file_path)
                .fetch_optional(&app.pool)
                .await
                .map_err(|e| AppError::Internal(e.into()))?;
            track_id = row.map(|(id,)| id);
        }
        if track_id.is_none() && !title.is_empty() {
            let row: Option<(i64,)> = if !artist.is_empty() {
                sqlx::query_as("SELECT id FROM tracks WHERE title = ? AND artist = ?")
                    .bind(title)
                    .bind(artist)
                    .fetch_optional(&app.pool)
                    .await
                    .map_err(|e| AppError::Internal(e.into()))?
            } else {
                sqlx::query_as("SELECT id FROM tracks WHERE title = ?")
                    .bind(title)
                    .fetch_optional(&app.pool)
                    .await
                    .map_err(|e| AppError::Internal(e.into()))?
            };
            track_id = row.map(|(id,)| id);
        }
        if let Some(id) = track_id {
            position += 1;
            sqlx::query(
                "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)",
            )
            .bind(pid)
            .bind(id)
            .bind(position)
            .execute(&app.pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
            matched += 1;
        }
    }

    Ok(Json(json!({
        "ok": true,
        "playlist_id": pid,
        "name": name_hint,
        "matched": matched,
        "total": entries.len(),
    })))
}
