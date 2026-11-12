//! Moidify library crate — shared by the `moidify-server` binary and tests.

pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod routes;
pub mod services;
pub mod state;
pub mod util;

use axum::Router;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

/// Build the full application router (same stack the binary serves).
pub fn build_router(state: state::AppState) -> Router {
    let static_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("static");
    Router::new()
        .merge(routes::api_router())
        .merge(routes::pages::router())
        .nest_service("/static", tower_http::services::ServeDir::new(static_dir))
        .fallback(routes::not_found)
        .layer(axum::middleware::from_fn(
            crate::util::security::security_headers,
        ))
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
