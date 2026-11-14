//! Admin endpoints — router assembly + scanner/rescan (Phase 5).
//! Byte-compatible with Python `routes/admin.py`:
//!   - `users.rs`   — user CRUD, sessions, unlock, registration toggle
//!   - `library.rs` — upload, metadata edit, delete, lyrics ops, covers
//!   - `system.rs`  — stats, dashboard, db tools, resources, server-info
//!   - this file    — scanner status, rescan, rescan schedule
//!
//! Import endpoints (import-url/import-album/search-album) come in Phase 8.

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::extractors::AdminUser;
use crate::error::AppResult;
use crate::scanner;
use crate::state::AppState;

mod library;
mod system;
mod users;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(users::router())
        .merge(library::router())
        .merge(system::router())
        .route("/api/admin/scanner", get(scanner_status))
        .route("/api/admin/rescan", post(rescan))
        .route(
            "/api/admin/schedule-rescan",
            get(get_schedule).post(set_schedule),
        )
}

/// Python `admin_scanner_status` → `get_scan_status()`.
async fn scanner_status(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
) -> AppResult<Json<Value>> {
    Ok(Json(app.scanner.status_json().await))
}

#[derive(Deserialize, Default)]
struct RescanBody {
    #[serde(default)]
    clean: bool,
}

/// Python `admin_rescan` — body is optional (`body: RescanBody = None`).
async fn rescan(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
    body: Option<Json<RescanBody>>,
) -> AppResult<Json<Value>> {
    let clean = body.map(|Json(b)| b.clean).unwrap_or(false);
    scanner::scan_existing(&app, clean).await;
    Ok(Json(app.scanner.status_json().await))
}

/// Python `get_rescan_schedule` → `_load_schedule()` (returns the parsed
/// dict verbatim; missing file → `{"interval_hours": 0, ...}` with an int 0).
async fn get_schedule(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
) -> AppResult<Json<Value>> {
    let path = scanner::schedule_path(&app);
    let value: Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| json!({ "interval_hours": 0, "last_scan": null }));
    Ok(Json(value))
}

#[derive(Deserialize)]
struct ScheduleBody {
    interval_hours: f64,
}

/// Python `set_rescan_schedule`.
async fn set_schedule(
    State(app): State<AppState>,
    AdminUser(_user): AdminUser,
    Json(body): Json<ScheduleBody>,
) -> AppResult<Json<Value>> {
    if body.interval_hours < 0.0 {
        return Err(crate::error::AppError::BadRequest(
            "Interval must be >= 0".into(),
        ));
    }
    let path = scanner::schedule_path(&app);
    let mut schedule = scanner::load_schedule(&path);
    schedule.interval_hours = body.interval_hours;
    scanner::save_schedule(&path, &schedule)
        .map_err(|e| crate::error::AppError::Internal(e.into()))?;
    Ok(Json(
        json!({ "ok": true, "interval_hours": body.interval_hours }),
    ))
}
