use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

pub mod models;
pub mod schema;

/// Open a SQLite pool with the same PRAGMAs the Python server used:
/// WAL, NORMAL synchronous, foreign keys ON.
pub async fn connect(db_path: &std::path::Path) -> anyhow::Result<SqlitePool> {
    if let Some(parent) = db_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let url = format!("sqlite:{}?mode=rwc", db_path.display());
    let options = SqliteConnectOptions::from_str(&url)?
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        // Small default pool: SQLite serializes writes anyway, and large
        // pools exhaust file descriptors when many tests run in parallel.
        .max_connections(4)
        .connect_with(options)
        .await?;

    Ok(pool)
}

/// Run schema creation + column-add migrations (idempotent).
pub async fn init(pool: &SqlitePool) -> anyhow::Result<()> {
    schema::migrate(pool).await
}
