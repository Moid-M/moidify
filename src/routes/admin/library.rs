//! Admin library operations: upload, metadata edit, track delete, lyrics
//! ops (upload/delete/fetch/scan-all), album/artist listings, cover
//! re-extraction.

use axum::extract::{Multipart, Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path as FsPath, PathBuf};

use crate::auth::extractors::AdminUser;
use crate::error::{AppError, AppResult};
use crate::scanner::{extract_metadata, process_file, save_cover};
use crate::services::lyrics::fetch_from_lrclib;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/admin/upload", post(upload))
        .route(
            "/api/admin/tracks/{track_id}/metadata",
            axum::routing::patch(update_metadata),
        )
        .route(
            "/api/admin/tracks/{track_id}/lyrics-upload",
            post(upload_lyrics),
        )
        .route(
            "/api/admin/tracks/{track_id}/lyrics",
            axum::routing::delete(delete_lyrics),
        )
        .route(
            "/api/admin/tracks/{track_id}/lyrics/fetch",
            post(fetch_lyrics),
        )
        .route("/api/admin/lyrics/scan-all", post(scan_all_lyrics))
        .route("/api/admin/albums", get(list_albums))
        .route("/api/admin/artists", get(list_artists))
        .route("/api/admin/albums/{album_name}", get(album_tracks))
        .route("/api/admin/re-extract-covers", post(re_extract_covers))
        .route(
            "/api/admin/tracks/{track_id}",
            axum::routing::delete(delete_track),
        )
}

// ── Upload ────────────────────────────────────────────────────────────────────

/// Python `AUDIO_EXTS` in admin.py — note: no `.opus` (unlike the scanner).
const UPLOAD_EXTS: [&str; 7] = ["mp3", "flac", "ogg", "m4a", "wav", "aac", "wma"];

/// Row shapes shared by the lyrics/album queries (clippy type_complexity).
type LyricsMetaRow = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
);
type LyricsMetaIdRow = (i64, Option<String>, Option<String>, Option<String>, String);
type AlbumRow = (String, String, i64, Option<f64>, Option<i64>);
type AlbumTracksRow = (
    i64,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<f64>,
    Option<i64>,
    Option<i64>,
);

/// Python `_is_audio_content()`: ftyp (MP4/M4A), MPEG sync, or magic bytes.
fn is_audio_content(data: &[u8]) -> bool {
    if data.len() < 8 {
        return false;
    }
    if &data[4..8] == b"ftyp" {
        return true;
    }
    if data[0] == 0xFF && (data[1] & 0xE0) == 0xE0 {
        return true;
    }
    for magic in [b"ID3".as_slice(), b"fLaC", b"OggS", b"RIFF"] {
        if data.starts_with(magic) {
            return true;
        }
    }
    false
}

/// Effective upload cap: env `MOIDIFY_MAX_UPLOAD_SIZE` (already folded into
/// `config.max_upload_size` by `Config::load`) > DB `settings.max_upload_size`
/// > config.json/default — same precedence as Python `config.py`.
async fn effective_max_upload_size(app: &AppState) -> i64 {
    if std::env::var("MOIDIFY_MAX_UPLOAD_SIZE").is_ok() {
        return app.config.max_upload_size;
    }
    let row: Option<(String,)> =
        sqlx::query_as("SELECT value FROM settings WHERE key = 'max_upload_size'")
            .fetch_optional(&app.pool)
            .await
            .ok()
            .flatten();
    row.and_then(|(v,)| v.parse::<i64>().ok())
        .unwrap_or(app.config.max_upload_size)
}

/// Python `admin_upload` — streams each part to `<name>.part`, sniffs the
/// first chunk, enforces the size cap, then renames + reprocesses.
async fn upload(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
    mut multipart: Multipart,
) -> AppResult<Json<Value>> {
    let max_size = effective_max_upload_size(&app).await;
    let mut imported: Vec<String> = Vec::new();

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("multipart: {e}")))?
    {
        if field.name() != Some("files") {
            continue;
        }
        let Some(filename) = field.file_name().map(str::to_string) else {
            continue;
        };
        let ext = FsPath::new(&filename)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();
        if !UPLOAD_EXTS.contains(&ext.as_str()) {
            continue;
        }
        let Some(base) = FsPath::new(&filename).file_name() else {
            continue;
        };
        let dest = app.config.music_dir.join(base);
        let temp = dest.with_file_name(format!("{}.part", base.to_string_lossy()));

        // Stream chunks to the temp file (Python reads 64 KiB at a time).
        let mut fh = tokio::fs::File::create(&temp)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        let mut first_chunk = true;
        let mut file_size: i64 = 0;
        loop {
            let chunk = field
                .chunk()
                .await
                .map_err(|e| AppError::BadRequest(format!("read file: {e}")))?
                .map(|b| b.to_vec());
            let Some(chunk) = chunk else { break };
            if first_chunk {
                if !is_audio_content(&chunk) {
                    let _ = tokio::fs::remove_file(&temp).await;
                    return Err(AppError::UnsupportedMediaType(format!(
                        "Invalid audio file: {filename}"
                    )));
                }
                first_chunk = false;
            }
            file_size += chunk.len() as i64;
            if file_size > max_size {
                let limit_gb = max_size as f64 / (1024.0 * 1024.0 * 1024.0);
                let _ = tokio::fs::remove_file(&temp).await;
                return Err(AppError::PayloadTooLarge(format!(
                    "File too large (max {limit_gb:.1} GB)"
                )));
            }
            tokio::io::AsyncWriteExt::write_all(&mut fh, &chunk)
                .await
                .map_err(|e| AppError::Internal(e.into()))?;
        }
        drop(fh);

        // Replace any existing file, then re-extract.
        let mut renamed = false;
        let outcome: Result<(), String> = async {
            if dest.exists() {
                tokio::fs::remove_file(&dest)
                    .await
                    .map_err(|e| format!("replace: {e}"))?;
            }
            tokio::fs::rename(&temp, &dest)
                .await
                .map_err(|e| format!("rename: {e}"))?;
            renamed = true;
            // Python: DELETE the stale row so process_file re-inserts.
            sqlx::query("DELETE FROM tracks WHERE file_path = ?")
                .bind(dest.to_string_lossy().to_string())
                .execute(&app.pool)
                .await
                .map_err(|e| format!("db: {e}"))?;
            process_file(&app, &dest, false).await
        }
        .await;

        match outcome {
            Ok(()) => imported.push(filename),
            // Python quirk: a hash failure inside process_file returns
            // normally, so the file still counts as imported.
            Err(msg) if msg.starts_with("Failed to hash:") => imported.push(filename),
            Err(msg) => {
                // Python generic-except: drop the temp + replaced file and
                // report the error for this file (loop continues).
                let _ = tokio::fs::remove_file(&temp).await;
                if renamed {
                    let _ = tokio::fs::remove_file(&dest).await;
                }
                imported.push(format!("{filename}: error - {msg}"));
            }
        }
    }
    Ok(Json(json!({ "imported": imported })))
}

// ── Metadata edit ─────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct MetadataBody {
    title: Option<String>,
    artist: Option<String>,
    album_artist: Option<String>,
    album: Option<String>,
    genre: Option<String>,
    year: Option<i64>,
    track_number: Option<i64>,
    disc_number: Option<i64>,
}

/// Python `admin_update_track_metadata` — dynamic UPDATE of provided fields
/// (field order matches Python's tuple: title, artist, album_artist, album,
/// genre, year, track_number, disc_number).
async fn update_metadata(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
    Path(track_id): Path<i64>,
    Json(body): Json<MetadataBody>,
) -> AppResult<Json<Value>> {
    let exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_optional(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    if exists.is_none() {
        return Err(AppError::NotFound("Track not found".into()));
    }

    let mut assignments: Vec<String> = Vec::new();
    let mut values: Vec<Value> = Vec::new();
    let add_str =
        |name: &str, v: Option<String>, assignments: &mut Vec<String>, values: &mut Vec<Value>| {
            if let Some(v) = v {
                assignments.push(format!("{name} = ?"));
                values.push(Value::String(v));
            }
        };
    let add_int =
        |name: &str, v: Option<i64>, assignments: &mut Vec<String>, values: &mut Vec<Value>| {
            if let Some(v) = v {
                assignments.push(format!("{name} = ?"));
                values.push(json!(v));
            }
        };
    add_str("title", body.title, &mut assignments, &mut values);
    add_str("artist", body.artist, &mut assignments, &mut values);
    add_str(
        "album_artist",
        body.album_artist,
        &mut assignments,
        &mut values,
    );
    add_str("album", body.album, &mut assignments, &mut values);
    add_str("genre", body.genre, &mut assignments, &mut values);
    add_int("year", body.year, &mut assignments, &mut values);
    add_int(
        "track_number",
        body.track_number,
        &mut assignments,
        &mut values,
    );
    add_int(
        "disc_number",
        body.disc_number,
        &mut assignments,
        &mut values,
    );

    if assignments.is_empty() {
        return Err(AppError::BadRequest("No fields to update".into()));
    }
    let sql = format!("UPDATE tracks SET {} WHERE id = ?", assignments.join(", "));
    let mut query = sqlx::query(&sql);
    for v in &values {
        query = match v {
            Value::String(s) => query.bind(s.as_str()),
            Value::Number(n) => query.bind(n.as_i64().unwrap_or(0)),
            _ => query.bind(""),
        };
    }
    query
        .bind(track_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({ "ok": true, "updated": assignments })))
}

// ── Lyrics ops ────────────────────────────────────────────────────────────────

/// Python `_save_lyrics_to_track` — UPDATE row + write companion `.lrc`
/// (write failures are logged, not raised).
async fn save_lyrics_to_track(
    app: &AppState,
    track_id: i64,
    content: &str,
    file_path: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE tracks SET lyrics = ? WHERE id = ?")
        .bind(content)
        .bind(track_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let lrc_dest = FsPath::new(file_path).with_extension("lrc");
    if let Err(e) = std::fs::write(&lrc_dest, content) {
        tracing::warn!(
            "Could not write companion .lrc at {}: {}",
            lrc_dest.display(),
            e
        );
    }
    Ok(())
}

async fn upload_lyrics(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
    Path(track_id): Path<i64>,
    mut multipart: Multipart,
) -> AppResult<Json<Value>> {
    let mut filename = String::new();
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
    if !filename.to_ascii_lowercase().ends_with(".lrc") {
        return Err(AppError::BadRequest("Only .lrc files are accepted".into()));
    }
    let row: Option<(String,)> = sqlx::query_as("SELECT file_path FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_optional(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let Some((file_path,)) = row else {
        return Err(AppError::NotFound("Track not found".into()));
    };
    let content = String::from_utf8_lossy(&bytes).into_owned();
    sqlx::query("UPDATE tracks SET lyrics = ? WHERE id = ?")
        .bind(&content)
        .bind(track_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let lrc_dest = FsPath::new(&file_path).with_extension("lrc");
    if let Err(e) = std::fs::write(&lrc_dest, &content) {
        tracing::warn!(
            "Could not write companion .lrc at {}: {}",
            lrc_dest.display(),
            e
        );
    }
    Ok(Json(json!({ "ok": true, "lyrics": content })))
}

async fn delete_lyrics(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
    Path(track_id): Path<i64>,
) -> AppResult<Json<Value>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT file_path FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_optional(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let Some((file_path,)) = row else {
        return Err(AppError::NotFound("Track not found".into()));
    };
    sqlx::query("UPDATE tracks SET lyrics = NULL WHERE id = ?")
        .bind(track_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let lrc_path = FsPath::new(&file_path).with_extension("lrc");
    if lrc_path.exists() {
        if let Err(e) = std::fs::remove_file(&lrc_path) {
            tracing::warn!(
                "Could not remove companion .lrc at {}: {}",
                lrc_path.display(),
                e
            );
        }
    }
    Ok(Json(json!({ "ok": true })))
}

async fn fetch_lyrics(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
    Path(track_id): Path<i64>,
) -> AppResult<Json<Value>> {
    let row: Option<LyricsMetaRow> =
        sqlx::query_as("SELECT artist, title, album, lyrics, file_path FROM tracks WHERE id = ?")
            .bind(track_id)
            .fetch_optional(&app.pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    let Some((artist, title, album, _lyrics, file_path)) = row else {
        return Err(AppError::NotFound("Track not found".into()));
    };
    let lyrics = fetch_from_lrclib(
        artist.as_deref().unwrap_or(""),
        title.as_deref().unwrap_or(""),
        album.as_deref().unwrap_or(""),
    )
    .await;
    if let Some(ref l) = lyrics {
        save_lyrics_to_track(&app, track_id, l, &file_path).await?;
    }
    Ok(Json(json!({ "ok": true, "lyrics": lyrics })))
}

#[derive(Deserialize, Default)]
struct ScanAllQuery {
    #[serde(default)]
    force: bool,
}

async fn scan_all_lyrics(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
    Query(q): Query<ScanAllQuery>,
) -> AppResult<Json<Value>> {
    let rows: Vec<LyricsMetaIdRow> = if q.force {
            sqlx::query_as(
                "SELECT id, artist, title, album, file_path FROM tracks",
            )
            .fetch_all(&app.pool)
            .await
        } else {
            sqlx::query_as(
                "SELECT id, artist, title, album, file_path FROM tracks WHERE lyrics IS NULL OR lyrics = ''",
            )
            .fetch_all(&app.pool)
            .await
        }
        .map_err(|e| AppError::Internal(e.into()))?;

    let mut scanned = 0;
    let mut found = 0;
    let mut errors = 0;
    for (id, artist, title, album, file_path) in rows {
        scanned += 1;
        let lyrics = fetch_from_lrclib(
            artist.as_deref().unwrap_or(""),
            title.as_deref().unwrap_or(""),
            album.as_deref().unwrap_or(""),
        )
        .await;
        if let Some(l) = lyrics {
            // Python counts `_save_lyrics_to_track` exceptions as errors.
            match save_lyrics_to_track(&app, id, &l, &file_path).await {
                Ok(()) => found += 1,
                Err(_) => errors += 1,
            }
        }
    }
    Ok(Json(
        json!({ "scanned": scanned, "found": found, "errors": errors }),
    ))
}

// ── Listings ──────────────────────────────────────────────────────────────────

async fn list_albums(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
) -> AppResult<Json<Value>> {
    let rows: Vec<AlbumRow> = sqlx::query_as(
        r#"SELECT
           COALESCE(NULLIF(album,''), 'Unknown') as album,
           COALESCE(NULLIF(album_artist,''), artist, 'Unknown') as artist,
           COUNT(*) as track_count,
           SUM(duration) as total_duration,
           MAX(has_cover) as has_cover
           FROM tracks
           GROUP BY album, artist
           ORDER BY artist, album"#,
    )
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    let out: Vec<Value> = rows
        .into_iter()
        .map(|(album, artist, track_count, total_duration, has_cover)| {
            json!({
                "album": album,
                "artist": artist,
                "track_count": track_count,
                "total_duration": total_duration,
                "has_cover": has_cover,
            })
        })
        .collect();
    Ok(Json(out.into()))
}

async fn list_artists(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
) -> AppResult<Json<Value>> {
    let rows: Vec<(String, i64, i64, Option<f64>)> = sqlx::query_as(
        r#"SELECT
           COALESCE(NULLIF(album_artist,''), artist, 'Unknown') as artist,
           COUNT(*) as track_count,
           COUNT(DISTINCT album) as album_count,
           SUM(duration) as total_duration
           FROM tracks
           GROUP BY artist
           ORDER BY artist"#,
    )
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    let out: Vec<Value> = rows
        .into_iter()
        .map(|(artist, track_count, album_count, total_duration)| {
            json!({
                "artist": artist,
                "track_count": track_count,
                "album_count": album_count,
                "total_duration": total_duration,
            })
        })
        .collect();
    Ok(Json(out.into()))
}

async fn album_tracks(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
    Path(album_name): Path<String>,
) -> AppResult<Json<Value>> {
    let rows: Vec<AlbumTracksRow> = sqlx::query_as(
        r#"SELECT id, title, artist, album, duration, track_number, has_cover
               FROM tracks WHERE album = ?
               ORDER BY track_number, title"#,
    )
    .bind(&album_name)
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    let out: Vec<Value> = rows
        .into_iter()
        .map(
            |(id, title, artist, album, duration, track_number, has_cover)| {
                json!({
                    "id": id,
                    "title": title,
                    "artist": artist,
                    "album": album,
                    "duration": duration,
                    "track_number": track_number,
                    "has_cover": has_cover,
                })
            },
        )
        .collect();
    Ok(Json(out.into()))
}

// ── Covers / delete ───────────────────────────────────────────────────────────

async fn re_extract_covers(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
) -> AppResult<Json<Value>> {
    let rows: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, file_path FROM tracks WHERE has_cover = 0")
            .fetch_all(&app.pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    let mut processed = 0;
    let mut errors = 0;
    for (tid, fp) in &rows {
        let path = FsPath::new(fp);
        if !path.exists() {
            errors += 1;
            continue;
        }
        if let Some(metadata) = extract_metadata(path) {
            if let Some(cover_data) = &metadata.cover_data {
                if let Ok(cover_hash) = save_cover(&app.config.covers_dir, cover_data) {
                    sqlx::query("UPDATE tracks SET has_cover = 1, cover_hash = ? WHERE id = ?")
                        .bind(&cover_hash)
                        .bind(tid)
                        .execute(&app.pool)
                        .await
                        .map_err(|e| AppError::Internal(e.into()))?;
                    processed += 1;
                }
            }
        }
    }
    Ok(Json(json!({
        "processed": processed,
        "errors": errors,
        "total_checked": rows.len(),
    })))
}

async fn delete_track(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
    Path(track_id): Path<i64>,
) -> AppResult<Json<Value>> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT file_path FROM tracks WHERE id = ?")
            .bind(track_id)
            .fetch_optional(&app.pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    let Some((fp,)) = row else {
        return Err(AppError::NotFound("Track not found".into()));
    };
    let mut tx = app
        .pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    for stmt in [
        "DELETE FROM playlist_tracks WHERE track_id = ?",
        "DELETE FROM favorites WHERE track_id = ?",
        "DELETE FROM tracks WHERE id = ?",
    ] {
        sqlx::query(stmt)
            .bind(track_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    }
    tx.commit()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    if let Some(path) = fp.filter(|p| !p.is_empty()) {
        let p = PathBuf::from(&path);
        if p.exists() {
            let _ = std::fs::remove_file(&p);
        }
    }
    // Python removes legacy id-named covers (current covers are hash-named).
    for ext in ["jpg", "png", "webp"] {
        let p = app.config.covers_dir.join(format!("{track_id}.{ext}"));
        if p.exists() {
            let _ = std::fs::remove_file(&p);
        }
    }
    Ok(Json(json!({ "deleted": track_id })))
}
