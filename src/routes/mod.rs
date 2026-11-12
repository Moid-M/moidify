use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;

use crate::state::AppState;

pub mod admin;
pub mod auth;
pub mod lastfm;
pub mod pages;
pub mod player;
pub mod playlists;
pub mod streaming;
pub mod subsonic;
pub mod tracks;

/// Phase 0: health only. Auth attaches in Phase 1; more routers per phase.
pub fn health_router() -> Router<AppState> {
    Router::new().route("/health", get(health))
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

/// Full API router assembled from phase modules.
pub fn api_router() -> Router<AppState> {
    Router::new()
        .merge(health_router())
        .merge(auth::router())
        .merge(tracks::router())
        .merge(streaming::router())
}

/// JSON 404 for unknown paths (matches FastAPI's `{"detail":"Not Found"}`).
pub async fn not_found() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "detail": "Not Found" })),
    )
}
