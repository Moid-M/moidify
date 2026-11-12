mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{get, get_auth, post_json, setup_admin, test_app};
use tower::ServiceExt;

// ── Health / version ────────────────────────────────────────────────────────

#[tokio::test]
async fn health_returns_ok() {
    let t = test_app().await;
    let app = t.router();
    let (status, json) = get(app, "/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json, serde_json::json!({ "status": "ok" }));
}

#[tokio::test]
async fn version_returns_string() {
    let t = test_app().await;
    let app = t.router();
    let (status, json) = get(app, "/api/version").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["version"].is_string());
}

// ── Schema ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn schema_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("idem.db");
    let pool = db::connect(&db_path).await.unwrap();
    db::init(&pool).await.unwrap();
    db::init(&pool).await.unwrap();

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

// Need `db` in scope for schema tests.
use moidify::db;

// ── Setup flow ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn setup_status_needs_setup_initially() {
    let t = test_app().await;
    let app = t.router();
    let (status, json) = get(app, "/api/setup/status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["setup_needed"], true);
    assert_eq!(json["has_admin"], false);
}

#[tokio::test]
async fn setup_init_creates_admin_and_token() {
    let t = test_app().await;
    let app = t.router();
    let (status, json) = post_json(
        app,
        "/api/setup/init",
        r#"{"username":"admin","password":"TestPass123!"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert!(json["token"].is_string());
    assert_eq!(json["user"]["username"], "admin");

    let (status, json) = get(app, "/api/setup/status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["setup_needed"], false);
    assert_eq!(json["has_admin"], true);
}

#[tokio::test]
async fn setup_init_twice_fails() {
    let t = test_app().await;
    let app = t.router();
    setup_admin(app).await;
    let (status, json) = post_json(
        app,
        "/api/setup/init",
        r#"{"username":"other","password":"TestPass123!"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["detail"], "Setup already completed");
}

#[tokio::test]
async fn setup_init_rejects_weak_password() {
    let t = test_app().await;
    let app = t.router();
    let (status, json) = post_json(
        app,
        "/api/setup/init",
        r#"{"username":"admin","password":"weak"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(json["detail"]
        .as_str()
        .unwrap()
        .contains("at least 8 characters"));
}

// ── Login ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn login_success_returns_token() {
    let t = test_app().await;
    let app = t.router();
    setup_admin(app).await;
    let (status, json) = post_json(
        app,
        "/api/auth/login",
        r#"{"username":"admin","password":"TestPass123!"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert!(json["token"].is_string());
    assert_eq!(json["user"]["username"], "admin");
}

#[tokio::test]
async fn login_wrong_password_401() {
    let t = test_app().await;
    let app = t.router();
    setup_admin(app).await;
    let (status, _) = post_json(
        app,
        "/api/auth/login",
        r#"{"username":"admin","password":"wrong"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn login_unknown_user_401() {
    let t = test_app().await;
    let app = t.router();
    let (status, _) = post_json(
        app,
        "/api/auth/login",
        r#"{"username":"ghost","password":"TestPass123!"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ── /api/auth/me ────────────────────────────────────────────────────────────

#[tokio::test]
async fn me_with_valid_token() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;
    let (status, json) = get_auth(app, "/api/auth/me", &token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["username"], "admin");
    assert_eq!(json["is_admin"], 1);
}

#[tokio::test]
async fn me_without_token_401() {
    let t = test_app().await;
    let app = t.router();
    let (status, _) = get(app, "/api/auth/me").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn me_with_bogus_token_401() {
    let t = test_app().await;
    let app = t.router();
    let (status, _) = get_auth(app, "/api/auth/me", "not-a-real-token").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ── Logout ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn logout_revokes_token() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/logout")
                .header("token", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let (status, body) = get_auth(app, "/api/auth/me", &token).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "body={body}");
}

// ── Registration gating (security regressions) ──────────────────────────────

#[tokio::test]
async fn registration_blocked_before_setup() {
    let t = test_app().await;
    let app = t.router();
    let (status, json) = post_json(
        app,
        "/api/auth/register",
        r#"{"username":"sneaky","password":"TestPass123!"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(json["detail"], "Registration is disabled");
}

#[tokio::test]
async fn registration_open_after_setup() {
    let t = test_app().await;
    let app = t.router();
    setup_admin(app).await;
    let (status, json) = post_json(
        app,
        "/api/auth/register",
        r#"{"username":"alice","password":"TestPass123!"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["ok"], true);
}

// ── Password storage (security regression) ──────────────────────────────────

#[tokio::test]
async fn password_hash_not_plaintext_in_db() {
    let t = test_app().await;
    let app = t.router();
    setup_admin(app).await;

    // Reach into AppState via a second setup path — query DB directly.
    // We rebuild a connection from the same temp path by re-reading via login
    // side effects is hard here; instead assert through a fresh app is not
    // possible. Use SQLx on the same DB the app uses: re-open via env is
    // messy, so verify the hash shape through the users table by making the
    // app do a login and then checking via a raw query on a new connection
    // to the SAME file — which we can't easily get from the Router.
    //
    // Simpler: use the known DB path from the fixture by constructing the
    // app ourselves here.
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("pwtest.db");
    let config = moidify::config::Config {
        db_path: db_path.clone(),
        ..Default::default()
    };
    config.ensure_dirs().unwrap();
    let pool = db::connect(&db_path).await.unwrap();
    db::init(&pool).await.unwrap();
    let fernet = std::sync::Arc::new(moidify::auth::load_or_create_fernet(&db_path).unwrap());
    let state = moidify::state::AppState::new(pool.clone(), std::sync::Arc::new(config), fernet);
    let app = moidify::build_router(state);

    setup_admin(&app).await;

    let row: (String, String, Option<String>) = sqlx::query_as(
        "SELECT password_hash, salt, password_encrypted FROM users WHERE username = 'admin'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let (hash, salt, enc) = row;
    assert!(
        !hash.contains("TestPass123!"),
        "hash must not contain plaintext"
    );
    assert!(!salt.contains("TestPass123!"));
    assert!(enc.is_some(), "password_encrypted must be set");
    assert!(
        !enc.as_ref().unwrap().contains("TestPass123!"),
        "encrypted copy must not be plaintext"
    );
}

// ── No admin auto-promote (security regression) ─────────────────────────────

#[tokio::test]
async fn no_admin_autopromote_on_init() {
    use moidify::auth::password::new_password;
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("auto.db");
    let pool = db::connect(&db_path).await.unwrap();
    db::init(&pool).await.unwrap();

    // Insert a NON-admin user literally named "admin".
    let (hash, salt) = new_password("TestPass123!");
    sqlx::query(
        "INSERT INTO users (username, password_hash, salt, is_admin) VALUES ('admin', ?, ?, 0)",
    )
    .bind(&hash)
    .bind(&salt)
    .execute(&pool)
    .await
    .unwrap();

    // Re-run migrate — must NOT flip is_admin.
    db::init(&pool).await.unwrap();

    let (is_admin,): (i64,) = sqlx::query_as("SELECT is_admin FROM users WHERE username = 'admin'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(is_admin, 0, "init must not auto-promote user 'admin'");
}
