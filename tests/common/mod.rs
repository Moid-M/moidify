// Each test binary compiles this module independently and uses a different
// subset of the helpers — silence per-binary dead_code warnings.
#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use moidify::auth::load_or_create_fernet;
use moidify::config::Config;
use moidify::{build_router, db, state};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;

/// Test app + the TempDir that must outlive it (dropping TempDir deletes the DB).
pub struct TestApp {
    pub router: axum::Router,
    /// Direct DB access for fixture inserts.
    pub pool: SqlitePool,
    /// Temp music dir (path-safety fixtures live here).
    pub music_dir: PathBuf,
    /// Temp covers dir (cover fixtures live here).
    pub covers_dir: PathBuf,
    /// Kept alive so the temp DB file isn't deleted mid-test.
    _dir: tempfile::TempDir,
}

impl TestApp {
    pub fn router(&self) -> &axum::Router {
        &self.router
    }
}

/// Build an isolated app with a fresh temp DB + Fernet key.
pub async fn test_app() -> TestApp {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("test.db");

    let config = Config {
        db_path: db_path.clone(),
        music_dir: dir.path().join("music"),
        covers_dir: dir.path().join("covers"),
        ..Default::default()
    };
    config.ensure_dirs().expect("dirs");

    let pool = db::connect(&db_path).await.expect("pool");
    db::init(&pool).await.expect("init");

    let fernet = Arc::new(load_or_create_fernet(&db_path).expect("fernet"));
    let state = state::AppState::new(pool.clone(), Arc::new(config.clone()), fernet);
    TestApp {
        router: build_router(state),
        pool,
        music_dir: config.music_dir.clone(),
        covers_dir: config.covers_dir.clone(),
        _dir: dir,
    }
}

/// POST JSON helper.
pub async fn post_json(
    app: &axum::Router,
    uri: &str,
    body: &str,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

/// GET helper.
pub async fn get(app: &axum::Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

/// GET with token header.
pub async fn get_auth(
    app: &axum::Router,
    uri: &str,
    token: &str,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri)
                .header("token", token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

/// Run setup init and return the token.
pub async fn setup_admin(app: &axum::Router) -> String {
    let (status, json) = post_json(
        app,
        "/api/setup/init",
        r#"{"username":"admin","password":"TestPass123!"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "setup failed: {json}");
    json["token"].as_str().expect("token").to_string()
}
