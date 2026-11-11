use crate::config::Config;
use sqlx::SqlitePool;
use std::sync::Arc;

/// Shared server state passed to every handler via `State<AppState>`.
#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Arc<Config>,
}

impl AppState {
    pub fn new(pool: SqlitePool, config: Arc<Config>) -> Self {
        Self { pool, config }
    }
}
