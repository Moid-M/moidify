//! Streaming: `/api/stream/{id}` (original + ffmpeg transcode tiers) and
//! `/api/cover/{id}`. Byte-compatible with Python `routes/streaming.py`.

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderName, HeaderValue, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde::Deserialize;
use std::path::{Path as FsPath, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::ServiceExt;
use tower_http::services::ServeFile;

use crate::error::{AppError, AppResult};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/stream/{track_id}", get(stream_track))
        .route("/api/cover/{track_id}", get(get_cover))
}

/// (quality, codec, bitrate) — mirrors Python `TRANSCODE_MAP`.
/// All tiers are libopus → ogg; `original` is absent (never transcoded).
const TRANSCODE_MAP: &[(&str, &str, &str)] = &[
    ("high", "libopus", "192k"),
    ("medium", "libopus", "128k"),
    ("low", "libopus", "96k"),
    ("voice", "libopus", "64k"),
];

/// Resolved once: `which ffmpeg`, or None (mirrors Python `shutil.which`).
fn ffmpeg_path() -> Option<&'static str> {
    use std::sync::OnceLock;
    static PATH: OnceLock<Option<String>> = OnceLock::new();
    PATH.get_or_init(|| {
        std::process::Command::new("which")
            .arg("ffmpeg")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    })
    .as_deref()
}

fn transcode_cfg(quality: &str) -> Option<(&'static str, &'static str)> {
    TRANSCODE_MAP
        .iter()
        .find(|(q, _, _)| *q == quality)
        .map(|(_, codec, bitrate)| (*codec, *bitrate))
}

/// Same extension → media-type table as Python `_media_type_for`.
fn media_type_for(path: &FsPath, quality: &str) -> &'static str {
    if quality != "original" && transcode_cfg(quality).is_some() {
        // Every tier is libopus → ogg container.
        return "audio/ogg";
    }
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
        Some("wma") => "audio/x-ms-wma",
        Some("aac") => "audio/aac",
        _ => "application/octet-stream",
    }
}

/// Python `_within_music_dir`: canonical path must live under MUSIC_DIR.
fn within_music_dir(path: &FsPath, music_dir: &FsPath) -> bool {
    let (Ok(canon), Ok(music)) = (path.canonicalize(), music_dir.canonicalize()) else {
        return false;
    };
    canon.starts_with(&music)
}

#[derive(Deserialize)]
pub struct StreamQuery {
    #[serde(default = "default_quality")]
    quality: String,
}

fn default_quality() -> String {
    "high".to_string()
}

async fn stream_track(
    State(app): State<AppState>,
    Path(track_id): Path<i64>,
    Query(q): Query<StreamQuery>,
    req: Request<Body>,
) -> AppResult<Response> {
    let row: Option<(String,)> = sqlx::query_as("SELECT file_path FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_optional(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let Some((file_path,)) = row else {
        return Err(AppError::NotFound("Track not found".into()));
    };

    // Python resolves before the exists check (symlinks followed).
    let path = tokio::fs::canonicalize(FsPath::new(&file_path))
        .await
        .map_err(|_| AppError::NotFound("File not found on disk".into()))?;

    if !within_music_dir(&path, &app.config.music_dir) {
        return Err(AppError::Forbidden("Forbidden".into()));
    }

    let quality = q.quality.as_str();
    let wants_transcode = quality != "original" && transcode_cfg(quality).is_some();

    if wants_transcode && ffmpeg_path().is_some() {
        return Ok(transcode_response(app, path, quality).await);
    }

    // Original (or no ffmpeg): ServeFile gives native Range/206 + Accept-Ranges,
    // same as Starlette FileResponse.
    let media = media_type_for(&path, "original");
    let mut resp = ServeFile::new(&path)
        .oneshot(req)
        .await
        .expect("ServeFile is infallible")
        .map(Body::new);
    if let Ok(ct) = HeaderValue::from_str(media) {
        resp.headers_mut().insert(header::CONTENT_TYPE, ct);
    }
    Ok(resp)
}

/// ffmpeg → stdout → chunked body. The permit + child live inside the
/// AsyncRead wrapper so they're dropped (→ killed) only when the body ends.
async fn transcode_response(app: AppState, path: PathBuf, quality: &str) -> Response {
    let (codec, bitrate) = transcode_cfg(quality).expect("checked by caller");
    let ffmpeg = ffmpeg_path().expect("checked by caller");
    let fmt = if codec == "libopus" { "ogg" } else { "mp3" };

    // Queue like Python's `async with semaphore` — wait for a free slot.
    let permit = match app.transcode_sem.clone().acquire_owned().await {
        Ok(p) => p,
        Err(_) => return serve_original_fallback(&path).await,
    };

    let child = tokio::process::Command::new(ffmpeg)
        .arg("-i")
        .arg(&path)
        .args(["-f", fmt, "-acodec", codec, "-ab", bitrate])
        .args(["-vn", "-nostdin", "-loglevel", "error", "-"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn();

    let mut child = match child {
        Ok(c) => c,
        Err(_) => return serve_original_fallback(&path).await,
    };
    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => return serve_original_fallback(&path).await,
    };

    let guarded = GuardedStdout {
        stdout,
        _child: ChildGuard(child),
        _permit: PermitGuard { _permit: permit },
    };
    let body = Body::from_stream(tokio_util::io::ReaderStream::new(guarded));

    let media = media_type_for(&path, quality);
    let mut resp = Response::new(body);
    *resp.status_mut() = StatusCode::OK;
    resp.headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_str(media).unwrap());
    resp.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("inline"),
    );
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    resp.headers_mut().insert(
        HeaderName::from_static("x-transcoded"),
        HeaderValue::from_str(quality).unwrap(),
    );
    resp
}

/// Dropping this kills ffmpeg (Python's post-stream cleanup).
struct ChildGuard(tokio::process::Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.start_kill();
    }
}

struct PermitGuard {
    /// Held for the stream lifetime; released on drop.
    _permit: tokio::sync::OwnedSemaphorePermit,
}

/// stdout reader that owns the child + semaphore permit for its lifetime.
struct GuardedStdout {
    stdout: tokio::process::ChildStdout,
    _child: ChildGuard,
    _permit: PermitGuard,
}

impl tokio::io::AsyncRead for GuardedStdout {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        // GuardedStdout is Unpin (all fields Unpin).
        Pin::new(&mut self.get_mut().stdout).poll_read(cx, buf)
    }
}

async fn serve_original_fallback(path: &FsPath) -> Response {
    let media = media_type_for(path, "original");
    match ServeFile::new(path)
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
    {
        Ok(resp) => {
            let mut resp = resp.map(Body::new);
            if let Ok(ct) = HeaderValue::from_str(media) {
                resp.headers_mut().insert(header::CONTENT_TYPE, ct);
            }
            resp
        }
        Err(_) => (StatusCode::NOT_FOUND, "file not found").into_response(),
    }
}

// ── Cover ────────────────────────────────────────────────────────────────────

async fn get_cover(State(app): State<AppState>, Path(track_id): Path<i64>) -> Response {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT cover_hash FROM tracks WHERE id = ?")
            .bind(track_id)
            .fetch_optional(&app.pool)
            .await
            .ok()
            .flatten();

    // 1) cover_hash file in covers dir
    if let Some((Some(hash),)) = row.filter(|(h,)| h.is_some()) {
        for ext in ["jpg", "png", "webp"] {
            let p = app.config.covers_dir.join(format!("{hash}.{ext}"));
            if p.exists() {
                return file_response(&p, cover_mime(ext), "public, max-age=86400").await;
            }
        }
    }

    // 2) legacy {track_id}.{ext} fallback
    for ext in ["jpg", "png", "webp"] {
        let p = app.config.covers_dir.join(format!("{track_id}.{ext}"));
        if p.exists() {
            return file_response(&p, cover_mime(ext), "public, max-age=86400").await;
        }
    }

    // 3) static placeholder
    let placeholder = FsPath::new(env!("CARGO_MANIFEST_DIR")).join("static/placeholder-cover.svg");
    file_response(&placeholder, "image/svg+xml", "public, max-age=3600").await
}

fn cover_mime(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "webp" => "image/webp",
        _ => "image/jpeg",
    }
}

async fn file_response(path: &FsPath, mime: &'static str, cache: &'static str) -> Response {
    match tokio::fs::read(path).await {
        Ok(bytes) => {
            let mut resp = (StatusCode::OK, bytes).into_response();
            resp.headers_mut()
                .insert(header::CONTENT_TYPE, HeaderValue::from_str(mime).unwrap());
            resp.headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
            resp
        }
        Err(_) => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}
