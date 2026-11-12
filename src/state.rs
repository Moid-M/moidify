use crate::auth::fernet::Fernet;
use crate::auth::ratelimit::RateLimiter;
use crate::config::Config;
use sqlx::SqlitePool;
use std::sync::Arc;

/// Shared server state passed to every handler via `State<AppState>`.
#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Arc<Config>,
    pub rate: Arc<RateLimiter>,
    pub fernet: Arc<Fernet>,
}

impl AppState {
    pub fn new(pool: SqlitePool, config: Arc<Config>, fernet: Arc<Fernet>) -> Self {
        Self {
            pool,
            config,
            rate: Arc::new(RateLimiter::new()),
            fernet,
        }
    }
}
