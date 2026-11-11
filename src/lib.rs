//! Moidify library crate — shared by the `moidify-server` binary and tests.

pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod routes;
pub mod state;

use axum::Router;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

/// Build the full application router (same stack the binary serves).
pub fn build_router(state: state::AppState) -> Router {
    Router::new()
        .merge(routes::health_router())
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
