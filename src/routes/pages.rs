//! HTML pages: /, /setup, /admin, /s/{t}, /a/{t}, /track/{id}, now-playing.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use sqlx::Row;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/setup", get(setup_page))
        .route("/admin", get(admin_page))
        .route("/", get(index))
        .route("/s/{token}", get(shared_playlist_page))
        .route("/a/{token}", get(shared_album_page))
        .route("/track/{track_id}", get(shared_track_page))
        .route("/api/player/now-playing", get(now_playing))
}

fn static_file(name: &str) -> Result<Response, StatusCode> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("static")
        .join(name);
    match std::fs::read(&path) {
        Ok(bytes) => Ok((
            StatusCode::OK,
            [("content-type", "text/html; charset=utf-8")],
            bytes,
        )
            .into_response()),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

async fn index() -> Result<Response, StatusCode> {
    static_file("index.html")
}

async fn admin_page() -> Result<Response, StatusCode> {
    static_file("admin.html")
}

async fn shared_playlist_page() -> Result<Response, StatusCode> {
    static_file("shared.html")
}

async fn shared_album_page() -> Result<Response, StatusCode> {
    static_file("shared-album.html")
}

/// Setup page — redirect to / if an admin already exists (same as Python).
async fn setup_page(State(app): State<AppState>) -> Result<Response, StatusCode> {
    let (admin_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE is_admin = 1")
        .fetch_one(&app.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if admin_count > 0 {
        return Ok(
            Html("<html><body><script>window.location.href='/'</script></body></html>")
                .into_response(),
        );
    }
    static_file("setup.html")
}

/// Server-generated share page with OG tags + inline player.
async fn shared_track_page(
    State(app): State<AppState>,
    Path(track_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    let row = sqlx::query("SELECT id, title, artist, album, duration FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_optional(&app.pool)
        .await;

    let row = match row {
        Ok(Some(r)) => r,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                [("content-type", "text/html; charset=utf-8")],
                "<html><body><h1>Track not found</h1></body></html>",
            )
                .into_response();
        }
    };

    let title = escape_html(
        &row.get::<Option<String>, _>("title")
            .unwrap_or_default_or("Unknown"),
    );
    let artist = escape_html(
        &row.get::<Option<String>, _>("artist")
            .unwrap_or_default_or("Unknown Artist"),
    );
    let album_raw = row.get::<Option<String>, _>("album").unwrap_or_default();
    let album = escape_html(&album_raw);
    let duration = row.get::<Option<f64>, _>("duration").unwrap_or(0.0);
    let dur_str = if duration > 0.0 {
        format!("{}:{:02}", (duration as i64) / 60, (duration as i64) % 60)
    } else {
        "?".to_string()
    };

    let base = base_url(&headers);
    let cover_url = format!("{base}api/cover/{track_id}");
    let stream_url = format!("{base}api/stream/{track_id}?quality=low");
    let app_url = format!("{base}?track={track_id}");
    let album_meta = if album_raw.is_empty() {
        dur_str.clone()
    } else {
        format!("{album} · {dur_str}")
    };

    let page = SHARE_PAGE
        .replace("{title}", &title)
        .replace("{artist}", &artist)
        .replace("{album_meta}", &album_meta)
        .replace("{cover_url}", &cover_url)
        .replace("{stream_url}", &stream_url)
        .replace("{app_url}", &app_url)
        .replace("{dur_str}", &dur_str);

    (
        StatusCode::OK,
        [("content-type", "text/html; charset=utf-8")],
        page,
    )
        .into_response()
}

async fn now_playing(State(app): State<AppState>) -> Json<serde_json::Value> {
    let row = sqlx::query(
        r#"SELECT t.id, t.title, t.artist, t.album, t.duration
           FROM play_history ph JOIN tracks t ON ph.track_id = t.id
           ORDER BY ph.played_at DESC LIMIT 1"#,
    )
    .fetch_optional(&app.pool)
    .await
    .ok()
    .flatten();

    match row {
        Some(r) => Json(json!({
            "id": r.get::<Option<i64>, _>("id"),
            "title": r.get::<Option<String>, _>("title"),
            "artist": r.get::<Option<String>, _>("artist"),
            "album": r.get::<Option<String>, _>("album"),
            "duration": r.get::<Option<f64>, _>("duration"),
        })),
        None => Json(json!({
            "id": null, "title": null, "artist": null, "album": null, "duration": null
        })),
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

trait UnwrapOrDefault {
    fn unwrap_or_default_or(self, fallback: &str) -> String;
}
impl UnwrapOrDefault for Option<String> {
    fn unwrap_or_default_or(self, fallback: &str) -> String {
        match self {
            Some(s) if !s.is_empty() => s,
            _ => fallback.to_string(),
        }
    }
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn base_url(headers: &HeaderMap) -> String {
    let host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");
    let proto = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("http");
    format!("{proto}://{host}/")
}

const SHARE_PAGE: &str = r##"<!DOCTYPE html>
<html>
<head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1.0">
<title>{title} — {artist} | Moidify</title>
<meta property="og:title" content="{title} — {artist}">
<meta property="og:description" content="Listen to {title} by {artist} on Moidify">
<meta property="og:image" content="{cover_url}">
<meta property="og:url" content="{app_url}">
<meta property="og:type" content="music.song">
<meta property="og:site_name" content="Moidify">
<meta name="theme-color" content="#a0a0a0">
<style>
*,*::before,*::after{box-sizing:border-box;margin:0;padding:0}
:root{--bg:#0d0d0d;--bg-el:#131313;--text:#fff;--text2:#b3b3b3;--text3:#6a6a6a;--accent:#a0a0a0}
body{background:var(--bg);color:var(--text);font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;min-height:100vh;display:flex;align-items:center;justify-content:center;padding:20px}
.wrap{max-width:400px;width:100%;text-align:center}
img{width:200px;height:200px;border-radius:16px;object-fit:cover;margin-bottom:20px;box-shadow:0 8px 32px rgba(0,0,0,0.5)}
h1{font-size:22px;margin-bottom:4px}
.artist{color:var(--text2);font-size:15px;margin-bottom:4px}
.meta{color:var(--text3);font-size:13px;margin-bottom:24px}
.controls{display:flex;align-items:center;justify-content:center;gap:16px;margin-bottom:20px}
.play-btn{width:56px;height:56px;border-radius:50%;border:none;background:var(--accent);color:#000;cursor:pointer;display:flex;align-items:center;justify-content:center;transition:transform .15s}
.play-btn:hover{transform:scale(1.08)}
.play-btn svg{width:24px;height:24px}
.progress-wrap{width:100%;display:flex;align-items:center;gap:10px;font-size:12px;color:var(--text3)}
progress{flex:1;height:4px;border-radius:2px;appearance:none;background:var(--bg-el)}
progress::-webkit-progress-bar{background:var(--bg-el);border-radius:2px}
progress::-webkit-progress-value{background:var(--accent);border-radius:2px}
progress::-moz-progress-bar{background:var(--accent);border-radius:2px}
.footer{margin-top:24px;font-size:13px;color:var(--text3)}
.footer a{color:var(--accent);text-decoration:none}
</style>
</head>
<body>
<div class="wrap">
  <img id="cover" src="{cover_url}" alt="Cover">
  <h1>{title}</h1>
  <div class="artist">{artist}</div>
  <div class="meta">{album_meta}</div>
  <div class="controls">
    <button class="play-btn" id="play-btn" onclick="togglePlay()">
      <svg viewBox="0 0 24 24" fill="currentColor" id="play-icon"><polygon points="5,3 19,12 5,21"/></svg>
    </button>
  </div>
  <div class="progress-wrap">
    <span id="current">0:00</span>
    <progress id="progress" value="0" max="100"></progress>
    <span id="total">{dur_str}</span>
  </div>
  <div class="footer">
    <a href="{app_url}">Open in Moidify</a> · Streamed via <a href="https://github.com/Moid-M/moidify">Moidify</a>
  </div>
</div>
<script>
var audio = new Audio('{stream_url}');
var playing = false;
function togglePlay() {
  if (playing) { audio.pause(); } else { audio.play(); }
}
audio.addEventListener('play', function() {
  playing = true;
  document.getElementById('play-icon').innerHTML = '<rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/>';
});
audio.addEventListener('pause', function() {
  playing = false;
  document.getElementById('play-icon').innerHTML = '<polygon points="5,3 19,12 5,21"/>';
});
audio.addEventListener('timeupdate', function() {
  if (audio.duration) {
    var pct = (audio.currentTime / audio.duration) * 100;
    document.getElementById('progress').value = pct;
    var m = Math.floor(audio.currentTime / 60);
    var s = Math.floor(audio.currentTime % 60);
    document.getElementById('current').textContent = m + ':' + (s < 10 ? '0' : '') + s;
  }
});
audio.addEventListener('ended', function() {
  playing = false;
  document.getElementById('play-icon').innerHTML = '<polygon points="5,3 19,12 5,21"/>';
  document.getElementById('progress').value = 0;
  document.getElementById('current').textContent = '0:00';
});
</script>
</body>
</html>"##;
