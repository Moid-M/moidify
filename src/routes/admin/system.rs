//! Admin system endpoints: stats, dashboard, listening trends, play stats,
//! DB tools (vacuum/integrity/stats), resource meters, server info.

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use std::path::Path;

use crate::auth::extractors::AdminUser;
use crate::error::{AppError, AppResult};
use crate::services::resources::{get_resource_stats, walk_music_dir};
use crate::state::AppState;

/// Row shape for the top-tracks query (clippy type_complexity).
type TopTrackRow = (
    i64,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
);

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/admin/stats", get(stats))
        .route("/api/admin/dashboard", get(dashboard))
        .route("/api/admin/listening-trends", get(listening_trends))
        .route("/api/admin/plays", get(play_stats))
        .route("/api/admin/resources", get(resources))
        .route("/api/admin/server-info", get(server_info))
        .route("/api/admin/db/vacuum", post(db_vacuum))
        .route("/api/admin/db/integrity", get(db_integrity))
        .route("/api/admin/db/stats", get(db_stats))
}

/// Python `admin_stats`.
async fn stats(State(app): State<AppState>, AdminUser(_user): AdminUser) -> AppResult<Json<Value>> {
    let (tracks,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks")
        .fetch_one(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let (artists,): (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT COALESCE(NULLIF(album_artist,''), artist)) FROM tracks",
    )
    .fetch_one(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    let (albums,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM (SELECT DISTINCT album, COALESCE(NULLIF(album_artist,''), artist) FROM tracks WHERE album IS NOT NULL)",
    )
    .fetch_one(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    // SUM over an empty/NULL column yields NULL (Python COALESCE → int 0);
    // with REAL values it yields a float. Mirror both JSON types.
    let (total_dur,): (Option<f64>,) = sqlx::query_as("SELECT SUM(duration) FROM tracks")
        .fetch_one(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let total_duration = match total_dur {
        Some(v) => json!((v * 10.0).round() / 10.0),
        None => json!(0),
    };
    let (total_bytes, _) = walk_music_dir(&app.config.music_dir);
    Ok(Json(json!({
        "tracks": tracks,
        "artists": artists,
        "albums": albums,
        "total_duration": total_duration,
        "disk_usage_bytes": total_bytes,
    })))
}

/// Python `admin_dashboard`.
async fn dashboard(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
) -> AppResult<Json<Value>> {
    let genres: Vec<(String, i64)> = sqlx::query_as(
        r#"SELECT COALESCE(NULLIF(genre,''), 'Unknown') as genre, COUNT(*) as count
           FROM tracks GROUP BY genre ORDER BY count DESC"#,
    )
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let monthly: Vec<(String, i64)> = sqlx::query_as(
        r#"SELECT strftime('%Y-%m', created_at) as month, COUNT(*) as count
           FROM tracks
           WHERE created_at >= date('now', '-12 months')
           GROUP BY month ORDER BY month"#,
    )
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let plays_day: Vec<(String, i64)> = sqlx::query_as(
        r#"SELECT strftime('%Y-%m-%d', played_at) as day, COUNT(*) as count
           FROM play_history
           WHERE played_at >= date('now', '-14 days')
           GROUP BY day ORDER BY day"#,
    )
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let (total_users,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let (_, disk_fmt) = walk_music_dir(&app.config.music_dir);
    let mut disk: Vec<(String, u64)> = disk_fmt;
    disk.sort_by_key(|(_, bytes)| std::cmp::Reverse(*bytes));
    let disk: Vec<Value> = disk
        .into_iter()
        .map(|(format, bytes)| json!({ "format": format, "bytes": bytes }))
        .collect();

    Ok(Json(json!({
        "genres": genres
            .into_iter()
            .map(|(genre, count)| json!({ "genre": genre, "count": count }))
            .collect::<Vec<Value>>(),
        "monthly_adds": monthly
            .into_iter()
            .map(|(month, count)| json!({ "month": month, "count": count }))
            .collect::<Vec<Value>>(),
        "plays_per_day": plays_day
            .into_iter()
            .map(|(day, count)| json!({ "day": day, "count": count }))
            .collect::<Vec<Value>>(),
        "disk_by_format": disk,
        "total_users": total_users,
        "disk_free": disk_free_bytes(&app.config.music_dir),
    })))
}

/// Python `shutil.disk_usage(music_dir).free`.
fn disk_free_bytes(music_dir: &Path) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        if let Ok(cp) = std::ffi::CString::new(music_dir.as_os_str().as_bytes()) {
            unsafe {
                let mut buf: libc::statvfs = std::mem::zeroed();
                if libc::statvfs(cp.as_ptr(), &mut buf) == 0 {
                    return buf.f_frsize as u64 * buf.f_bavail as u64;
                }
            }
        }
    }
    #[allow(unreachable_code)]
    0
}

/// Python `admin_listening_trends`.
async fn listening_trends(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
) -> AppResult<Json<Value>> {
    let by_hour: Vec<(i64, i64)> = sqlx::query_as(
        r#"SELECT CAST(strftime('%H', played_at) AS INTEGER) as hour, COUNT(*) as count
           FROM play_history GROUP BY hour ORDER BY hour"#,
    )
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let by_day: Vec<(String, i64)> = sqlx::query_as(
        r#"SELECT CASE CAST(strftime('%w', played_at) AS INTEGER)
                    WHEN 0 THEN 'Sun' WHEN 1 THEN 'Mon' WHEN 2 THEN 'Tue'
                    WHEN 3 THEN 'Wed' WHEN 4 THEN 'Thu' WHEN 5 THEN 'Fri'
                    ELSE 'Sat' END as day,
                COUNT(*) as count
           FROM play_history GROUP BY day ORDER BY MIN(played_at)"#,
    )
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    Ok(Json(json!({
        "by_hour": by_hour
            .into_iter()
            .map(|(hour, count)| json!({ "hour": hour, "count": count }))
            .collect::<Vec<Value>>(),
        "by_day": by_day
            .into_iter()
            .map(|(day, count)| json!({ "day": day, "count": count }))
            .collect::<Vec<Value>>(),
    })))
}

/// Python `admin_play_stats`.
async fn play_stats(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
) -> AppResult<Json<Value>> {
    // play_count is INTEGER → SUM is INTEGER (Python emits an int).
    let (total,): (Option<i64>,) = sqlx::query_as("SELECT SUM(play_count) FROM tracks")
        .fetch_one(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let total = total.unwrap_or(0);

    let top_tracks: Vec<TopTrackRow> = sqlx::query_as(
        "SELECT id, title, artist, album, play_count FROM tracks ORDER BY play_count DESC LIMIT 10",
    )
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let top_artists: Vec<(Option<String>, Option<i64>)> = sqlx::query_as(
        r#"SELECT COALESCE(NULLIF(album_artist,''), artist) as artist, COALESCE(SUM(play_count),0) as play_count
           FROM tracks WHERE artist IS NOT NULL
           GROUP BY artist ORDER BY play_count DESC LIMIT 10"#,
    )
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let top_albums: Vec<(Option<String>, Option<String>, Option<i64>)> = sqlx::query_as(
        r#"SELECT album, COALESCE(NULLIF(album_artist,''), artist) as artist, COALESCE(SUM(play_count),0) as play_count
           FROM tracks WHERE album IS NOT NULL
           GROUP BY album, artist ORDER BY play_count DESC LIMIT 10"#,
    )
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    // duration is REAL → SUM is REAL or NULL (empty) — Python round(0,1)
    // yields the int 0, otherwise a float.
    let (listen,): (Option<f64>,) = sqlx::query_as(
        r#"SELECT SUM(t.duration) as total_time
           FROM play_history ph JOIN tracks t ON ph.track_id = t.id"#,
    )
    .fetch_one(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    let listen_time = match listen {
        Some(v) => json!((v * 10.0).round() / 10.0),
        None => json!(0),
    };

    Ok(Json(json!({
        "total_plays": total,
        "total_listen_time": listen_time,
        "top_tracks": top_tracks
            .into_iter()
            .map(|(id, title, artist, album, play_count)| json!({
                "id": id, "title": title, "artist": artist,
                "album": album, "play_count": play_count,
            }))
            .collect::<Vec<Value>>(),
        "top_artists": top_artists
            .into_iter()
            .map(|(artist, play_count)| {
                json!({ "artist": artist, "play_count": play_count.unwrap_or(0) })
            })
            .collect::<Vec<Value>>(),
        "top_albums": top_albums
            .into_iter()
            .map(|(album, artist, play_count)| json!({
                "album": album, "artist": artist, "play_count": play_count.unwrap_or(0),
            }))
            .collect::<Vec<Value>>(),
    })))
}

/// Python `admin_resources`.
async fn resources(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
) -> AppResult<Json<Value>> {
    Ok(Json(get_resource_stats(&app.config.music_dir)))
}

/// Python `admin_server_info`.
///
/// Key set matches Python exactly (`python_version`, `platform`, …) because
/// the untouched frontend renders those keys. Values differ where the stack
/// differs: `python_version` reports the Rust build, `mutagen_version`
/// reports lofty. `yt_dlp_version` shells out like Python.
async fn server_info(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
) -> AppResult<Json<Value>> {
    let server_time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let process_uptime = process_uptime_string();

    let mutagen_version = "n/a (lofty)".to_string();

    let yt_dlp_version = yt_dlp_version().await;

    let platform = platform_string();

    Ok(Json(json!({
        "python_version": format!("n/a (rust {})", env!("CARGO_PKG_VERSION")),
        "platform": platform,
        "server_time": server_time,
        "process_uptime": process_uptime,
        "mutagen_version": mutagen_version,
        "yt_dlp_version": yt_dlp_version,
        "music_dir": app.config.music_dir.to_string_lossy(),
        "base_dir": Path::new(env!("CARGO_MANIFEST_DIR")).to_string_lossy(),
    })))
}

/// Python psutil branch: `Xd Yh Zm` from /proc uptime − process start.
fn process_uptime_string() -> Option<String> {
    let uptime_text = std::fs::read_to_string("/proc/uptime").ok()?;
    let sys_up: f64 = uptime_text.split_whitespace().next()?.parse().ok()?;
    let self_text = std::fs::read_to_string("/proc/self/stat").ok()?;
    let parts: Vec<&str> = self_text.split_whitespace().collect();
    if parts.len() < 22 {
        return None;
    }
    let start_time: f64 = parts[21].parse().ok()?;
    let up = sys_up - start_time / 100.0; // CLK_TCK = 100
    if up < 0.0 {
        return None;
    }
    let secs = up as i64;
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    Some(format!("{days}d {hours}h {mins}m"))
}

/// Python `platform.platform()` ≈ `Linux-<release>-<arch>`.
fn platform_string() -> String {
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".into());
    format!("Linux-{release}-{}", std::env::consts::ARCH)
}

/// Python: `subprocess.run(["yt-dlp", "--version"], timeout=5)`.
async fn yt_dlp_version() -> String {
    let cmd = tokio::process::Command::new("yt-dlp")
        .arg("--version")
        .output();
    match tokio::time::timeout(std::time::Duration::from_secs(5), cmd).await {
        Ok(Ok(out)) if out.status.success() => {
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        }
        _ => "not found".to_string(),
    }
}

// ── DB tools ──────────────────────────────────────────────────────────────────

async fn db_vacuum(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
) -> AppResult<Json<Value>> {
    sqlx::query("VACUUM")
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({ "ok": true, "message": "Database vacuumed" })))
}

async fn db_integrity(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
) -> AppResult<Json<Value>> {
    let (result,): (String,) = sqlx::query_as("PRAGMA integrity_check")
        .fetch_one(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({ "ok": result == "ok", "result": result })))
}

async fn db_stats(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
) -> AppResult<Json<Value>> {
    let (page_count,): (i64,) = sqlx::query_as("PRAGMA page_count")
        .fetch_one(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let (page_size,): (i64,) = sqlx::query_as("PRAGMA page_size")
        .fetch_one(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let db_size = page_count * page_size;
    let (track_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks")
        .fetch_one(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let (user_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let (playlist_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM playlists")
        .fetch_one(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let (play_history,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM play_history")
        .fetch_one(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let db_size_display = if db_size > 1024 * 1024 {
        format!("{:.1} MB", db_size as f64 / 1024.0 / 1024.0)
    } else {
        format!("{:.1} KB", db_size as f64 / 1024.0)
    };
    Ok(Json(json!({
        "db_size_bytes": db_size,
        "db_size_display": db_size_display,
        "track_count": track_count,
        "user_count": user_count,
        "playlist_count": playlist_count,
        "play_history_count": play_history,
    })))
}
