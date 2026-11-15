//! Playback/media endpoints: `stream`, `download`, `getCoverArt`,
//! `getAvatar`.
//!
//! Port notes:
//! - `stream`/`download` are the same handler (Python stacks two
//!   `api_route` decorators); they always serve the *original* file via
//!   `ServeFile` (native Range/206 like Starlette's `FileResponse`) with the
//!   Subsonic-specific media-type table (`unknown → audio/mpeg`,
//!   `.opus → audio/ogg`) and `Content-Disposition: inline`.
//! - Unlike Python (which serves whatever absolute path sits in the DB),
//!   files must resolve inside `MUSIC_DIR` — same containment rule the web
//!   `/api/stream` got in the Python hardening pass.
//! - `getCoverArt` ignores `size` (Python does too) and 404s with an empty
//!   body when no cover exists (the web route falls back to a placeholder).

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{header, HeaderValue, Request, StatusCode};
use axum::response::IntoResponse;
use std::path::Path as FsPath;
use tower::ServiceExt;
use tower_http::services::ServeFile;

use super::auth;
use super::response::error;
use super::{respond, Params, SubResult};
use crate::error::AppError;
use crate::routes::streaming::{cover_mime, file_response, within_music_dir};
use crate::state::AppState;

/// Python's Subsonic media-type dict — deliberately *not* shared with the
/// web route (that one maps unknown extensions to `application/octet-stream`
/// and has no `.wma` entry parity question; Subsonic falls back to
/// `audio/mpeg` for everything unknown).
fn subsonic_media_type(path: &FsPath) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("mp3") => "audio/mpeg",
        Some("flac") => "audio/flac",
        Some("ogg") | Some("opus") => "audio/ogg",
        Some("m4a") => "audio/mp4",
        Some("wav") => "audio/wav",
        Some("aac") => "audio/aac",
        Some("wma") => "audio/x-ms-wma",
        _ => "audio/mpeg",
    }
}

/// `GET|POST /rest/stream` — Python shares this handler with `/download`.
pub async fn stream(
    State(app): State<AppState>,
    Query(q): Query<Params>,
    req: Request<Body>,
) -> SubResult {
    serve_track_file(app, q, req).await
}

/// `GET|POST /rest/download` — byte-identical to `stream`.
pub async fn download(
    State(app): State<AppState>,
    Query(q): Query<Params>,
    req: Request<Body>,
) -> SubResult {
    serve_track_file(app, q, req).await
}

async fn serve_track_file(app: AppState, q: Params, req: Request<Body>) -> SubResult {
    let id = q.require_id()?;
    let _user = auth::require(&app, &q).await?;

    let Ok(track_id) = id.parse::<i64>() else {
        return Ok(respond(&q, error(70, "File not found")));
    };
    let row: Option<(String,)> = sqlx::query_as("SELECT file_path FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_optional(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let Some((file_path,)) = row else {
        return Ok(respond(&q, error(70, "File not found")));
    };

    // Python only checks `path.exists()`; the port additionally requires the
    // canonical path to stay inside MUSIC_DIR (hardening parity with
    // `/api/stream`).
    let Ok(path) = tokio::fs::canonicalize(FsPath::new(&file_path)).await else {
        return Ok(respond(&q, error(70, "File not found on disk")));
    };
    if !within_music_dir(&path, &app.config.music_dir) {
        return Ok(respond(&q, error(70, "File not found on disk")));
    }

    let media = subsonic_media_type(&path);
    // tower-http's ServeFile answers 405 for POST; Python's FileResponse
    // serves any method, so coerce non-GET/HEAD verbs to GET (Range etc.
    // still pass through unchanged).
    let mut req = req;
    if req.method() != axum::http::Method::GET && req.method() != axum::http::Method::HEAD {
        *req.method_mut() = axum::http::Method::GET;
    }
    let mut resp = ServeFile::new(&path)
        .oneshot(req)
        .await
        .expect("ServeFile is infallible")
        .map(Body::new);
    if let Ok(ct) = HeaderValue::from_str(media) {
        resp.headers_mut().insert(header::CONTENT_TYPE, ct);
    }
    resp.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("inline"),
    );
    Ok(resp)
}

/// `GET|POST /rest/getCoverArt` — `cover_hash` first, then legacy
/// `{track_id}.{ext}` files; empty 404 otherwise. `size` is accepted and
/// ignored, exactly like Python.
pub async fn get_cover_art(State(app): State<AppState>, Query(q): Query<Params>) -> SubResult {
    let id = q.require_id()?;
    let _user = auth::require(&app, &q).await?;

    // Python: `int(id)` → None when non-numeric; `if track_id:` skips 0.
    let track_id: Option<i64> = id.parse().ok().filter(|&t| t != 0);

    if let Some(tid) = track_id {
        let row: Option<(Option<String>,)> =
            sqlx::query_as("SELECT cover_hash FROM tracks WHERE id = ?")
                .bind(tid)
                .fetch_optional(&app.pool)
                .await
                .map_err(|e| AppError::Internal(e.into()))?;

        if let Some((Some(hash),)) = row {
            for ext in ["jpg", "png", "webp"] {
                let p = app.config.covers_dir.join(format!("{hash}.{ext}"));
                if p.exists() {
                    return Ok(file_response(&p, cover_mime(ext), "public, max-age=86400").await);
                }
            }
        }
        for ext in ["jpg", "png", "webp"] {
            let p = app.config.covers_dir.join(format!("{tid}.{ext}"));
            if p.exists() {
                return Ok(file_response(&p, cover_mime(ext), "public, max-age=86400").await);
            }
        }
    }

    Ok(StatusCode::NOT_FOUND.into_response())
}

/// `GET|POST /rest/getAvatar` — unconditional 204, no auth (Python never
/// calls `_require_auth` here).
pub async fn get_avatar(State(_app): State<AppState>, Query(_q): Query<Params>) -> SubResult {
    Ok(StatusCode::NO_CONTENT.into_response())
}
