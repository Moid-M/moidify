use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use moidify::config::Config;
use moidify::{build_router, db, state};
use std::sync::Arc;
use tower::ServiceExt;

async fn test_app() -> axum::Router {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("test.db");

    let config = Config {
        db_path: db_path.clone(),
        ..Default::default()
    };
    config.ensure_dirs().expect("dirs");

    let pool = db::connect(&db_path).await.expect("pool");
    db::init(&pool).await.expect("init");

    let state = state::AppState::new(pool, Arc::new(config));
    build_router(state)
}

#[tokio::test]
async fn health_returns_ok() {
    let app = test_app().await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(body, r#"{"status":"ok"}"#);
}

#[tokio::test]
async fn schema_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("idem.db");
    let pool = db::connect(&db_path).await.unwrap();
    db::init(&pool).await.unwrap();
    db::init(&pool).await.unwrap(); // second run must not fail

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sqlite_master WHERE type='table'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(count >= 13, "expected >=13 tables, got {count}");
}

#[tokio::test]
async fn column_migration_adds_password_encrypted() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("mig.db");
    let pool = db::connect(&db_path).await.unwrap();

    // Simulate an old pre-password_encrypted database.
    sqlx::query(
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT UNIQUE NOT NULL,
            password_hash TEXT NOT NULL,
            salt TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    db::init(&pool).await.unwrap();

    let cols: Vec<(i64, String)> = sqlx::query_as("PRAGMA table_info(users)")
        .fetch_all(&pool)
        .await
        .unwrap();
    let names: Vec<String> = cols.into_iter().map(|(_, n)| n).collect();
    assert!(names.contains(&"password_encrypted".to_string()));
    assert!(names.contains(&"is_admin".to_string()));
}
