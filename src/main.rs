use moidify::auth::load_or_create_fernet;
use moidify::config::Config;
use moidify::{build_router, db, state};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "moidify=info,tower_http=info".into()),
        )
        .init();

    let config = Arc::new(Config::load());
    config.ensure_dirs()?;

    tracing::info!(
        port = config.port,
        music = %config.music_dir.display(),
        covers = %config.covers_dir.display(),
        db = %config.db_path.display(),
        "starting moidify"
    );

    let pool = db::connect(&config.db_path).await?;
    db::init(&pool).await?;
    tracing::info!("database ready");

    let fernet = Arc::new(load_or_create_fernet(&config.db_path)?);
    tracing::info!("fernet key ready");

    let state = state::AppState::new(pool, config.clone(), fernet);
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", config.port)).await?;
    tracing::info!("listening on 0.0.0.0:{}", config.port);
    axum::serve(listener, app).await?;
    Ok(())
}
