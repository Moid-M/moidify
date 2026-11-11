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

/// Phase 0: health check only. Other routers attach as phases land.
pub fn health_router() -> Router<AppState> {
    Router::new().route("/health", get(health))
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}
