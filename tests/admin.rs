//! Phase 6: admin — users, upload, metadata edit, lyrics ops, stats/
//! dashboard, db tools, resources, server-info, registration toggle.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::path::PathBuf;
use tower::ServiceExt;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tagged.mp3")
}

async fn post_auth(
    app: &axum::Router,
    uri: &str,
    token: &str,
    body: Option<&str>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method("POST")
        .uri(uri)
        .header("token", token);
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    let response = app
        .clone()
        .oneshot(
            builder
                .body(Body::from(body.unwrap_or("").to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

async fn patch_auth(app: &axum::Router, uri: &str, token: &str, body: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(uri)
                .header("token", token)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

async fn delete_auth(app: &axum::Router, uri: &str, token: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(uri)
                .header("token", token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

/// Minimal multipart/form-data body builder: (field, filename, bytes).
fn multipart(fields: &[(&str, Option<&str>, &[u8])]) -> (Vec<u8>, String) {
    let boundary = "----moidifytestboundary123";
    let mut body = Vec::new();
    for (name, filename, data) in fields {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{name}\"").as_bytes(),
        );
        if let Some(f) = filename {
            body.extend_from_slice(format!("; filename=\"{f}\"").as_bytes());
        }
        body.extend_from_slice(b"\r\nContent-Type: application/octet-stream\r\n\r\n");
        body.extend_from_slice(data);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (body, boundary.to_string())
}

async fn post_multipart(
    app: &axum::Router,
    uri: &str,
    token: &str,
    fields: &[(&str, Option<&str>, &[u8])],
) -> (StatusCode, Value) {
    let (body, boundary) = multipart(fields);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("token", token)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

/// Insert a minimal track row directly; returns the new id.
async fn insert_track(pool: &sqlx::SqlitePool, file_path: &str) -> i64 {
    let result = sqlx::query(
        "INSERT INTO tracks (file_path, title, artist, album, duration) VALUES (?, 'T', 'A', 'Alb', 1.0)",
    )
    .bind(file_path)
    .execute(pool)
    .await
    .unwrap();
    result.last_insert_rowid()
}

// ── Auth matrix ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn admin_endpoints_auth_matrix() {
    let t = test_app().await;
    let app = t.router();
    let admin = setup_admin(app).await;
    let bob = {
        let (status, json) = post_json(
            app,
            "/api/auth/register",
            r#"{"username":"bob","password":"TestPass123!"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{json}");
        let (_, json) = post_json(
            app,
            "/api/auth/login",
            r#"{"username":"bob","password":"TestPass123!"}"#,
        )
        .await;
        json["token"].as_str().unwrap().to_string()
    };

    let gets = [
        "/api/admin/stats",
        "/api/admin/users",
        "/api/admin/dashboard",
        "/api/admin/plays",
        "/api/admin/listening-trends",
        "/api/admin/server-info",
        "/api/admin/resources",
        "/api/admin/db/stats",
        "/api/admin/db/integrity",
        "/api/admin/registration-status",
        "/api/admin/failed-logins",
        "/api/admin/albums",
        "/api/admin/artists",
    ];
    for uri in gets {
        let (status, _) = get(app, uri).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "anon GET {uri}");
        let (status, _) = get_auth(app, uri, &bob).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "bob GET {uri}");
        let (status, _) = get_auth(app, uri, &admin).await;
        assert_eq!(status, StatusCode::OK, "admin GET {uri}");
    }

    let posts = [
        "/api/admin/db/vacuum",
        "/api/admin/re-extract-covers",
        "/api/admin/lyrics/scan-all",
        "/api/admin/registration-toggle",
        "/api/admin/rescan",
    ];
    for uri in posts {
        let (status, _) = post_auth(app, uri, &bob, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "bob POST {uri}");
        let (status, _) = post_auth(app, uri, "bogus", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "bad POST {uri}");
    }
}

// ── Stats / dashboard / plays / trends ───────────────────────────────────────

#[tokio::test]
async fn admin_stats_shape() {
    let t = test_app().await;
    let token = setup_admin(t.router()).await;
    insert_track(&t.pool, "/music/a.mp3").await;

    let (status, json) = get_auth(t.router(), "/api/admin/stats", &token).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["tracks"], 1);
    assert!(json["artists"].is_number());
    assert!(json["albums"].is_number());
    assert!(json["total_duration"].is_number());
    assert!(json["disk_usage_bytes"].is_number());
}

#[tokio::test]
async fn admin_dashboard_and_plays_and_trends() {
    let t = test_app().await;
    let token = setup_admin(t.router()).await;
    let tid = insert_track(&t.pool, "/music/g.mp3").await;
    sqlx::query("UPDATE tracks SET genre = 'Rock', play_count = 7 WHERE id = ?")
        .bind(tid)
        .execute(&t.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO play_history (track_id, played_at) VALUES (?, '2026-11-14 10:00:00')")
        .bind(tid)
        .execute(&t.pool)
        .await
        .unwrap();

    let (status, json) = get_auth(t.router(), "/api/admin/dashboard", &token).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["genres"][0]["genre"], "Rock");
    assert_eq!(json["genres"][0]["count"], 1);
    assert!(json["monthly_adds"].is_array());
    assert!(json["plays_per_day"].is_array());
    assert!(json["disk_by_format"].is_array());
    assert_eq!(json["total_users"], 1);
    assert!(json["disk_free"].is_number());

    let (status, json) = get_auth(t.router(), "/api/admin/plays", &token).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["total_plays"], 7);
    assert!(json["total_listen_time"].is_number());
    assert_eq!(json["top_tracks"][0]["title"], "T");
    assert_eq!(json["top_artists"][0]["play_count"], 7);

    let (status, json) = get_auth(t.router(), "/api/admin/listening-trends", &token).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert!(json["by_hour"].is_array());
    assert!(json["by_day"].is_array());
    // played_at '2026-11-14 10:00:00' → hour 10, Saturday.
    assert_eq!(json["by_hour"][0]["hour"], 10);
    assert_eq!(json["by_hour"][0]["count"], 1);
    assert_eq!(json["by_day"][0]["day"], "Sat");
}

// ── Server info / resources / db tools ───────────────────────────────────────

#[tokio::test]
async fn admin_server_info_shape() {
    let t = test_app().await;
    let token = setup_admin(t.router()).await;
    let (status, json) = get_auth(t.router(), "/api/admin/server-info", &token).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    // Frontend renders these exact keys.
    for key in [
        "python_version",
        "platform",
        "server_time",
        "process_uptime",
        "mutagen_version",
        "yt_dlp_version",
        "music_dir",
        "base_dir",
    ] {
        assert!(json.get(key).is_some(), "missing {key}: {json}");
    }
    assert!(json["platform"].as_str().unwrap().starts_with("Linux-"));
}

#[tokio::test]
async fn admin_resources_shape() {
    let t = test_app().await;
    let token = setup_admin(t.router()).await;
    let (status, json) = get_auth(t.router(), "/api/admin/resources", &token).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert!(json["system"]["mem_total_bytes"].is_number(), "{json}");
    assert!(json["system"]["uptime_seconds"].is_number(), "{json}");
    assert!(json["process"]["rss_bytes"].is_number(), "{json}");
    assert_eq!(
        json["disk"]["music_dir"],
        t.music_dir.to_string_lossy().as_ref()
    );
}

#[tokio::test]
async fn admin_db_tools() {
    let t = test_app().await;
    let token = setup_admin(t.router()).await;

    let (status, json) = get_auth(t.router(), "/api/admin/db/stats", &token).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    for key in [
        "db_size_bytes",
        "db_size_display",
        "track_count",
        "user_count",
        "playlist_count",
        "play_history_count",
    ] {
        assert!(json.get(key).is_some(), "missing {key}");
    }
    assert!(
        json["db_size_display"].as_str().unwrap().ends_with(" MB")
            || json["db_size_display"].as_str().unwrap().ends_with(" KB")
    );

    let (status, json) = get_auth(t.router(), "/api/admin/db/integrity", &token).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["ok"], true);
    assert_eq!(json["result"], "ok");

    let (status, json) = post_auth(t.router(), "/api/admin/db/vacuum", &token, None).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["ok"], true);
    assert_eq!(json["message"], "Database vacuumed");
}

// ── Registration toggle ───────────────────────────────────────────────────────

#[tokio::test]
async fn admin_registration_toggle_cycle() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;

    // Default after setup: open (admin exists, no settings row).
    let (status, json) = get_auth(app, "/api/admin/registration-status", &token).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["registration_open"], true);

    // First toggle closes → register 403 (Python test).
    let (status, json) = post_auth(app, "/api/admin/registration-toggle", &token, None).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["registration_open"], false);
    let (status, json) = post_json(
        app,
        "/api/auth/register",
        r#"{"username":"late","password":"TestPass123!"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{json}");

    // Toggle again re-opens.
    let (status, json) = post_auth(app, "/api/admin/registration-toggle", &token, None).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["registration_open"], true);
    let (status, _) = post_json(
        app,
        "/api/auth/register",
        r#"{"username":"late","password":"TestPass123!"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

// ── Users ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn admin_users_list_shape() {
    let t = test_app().await;
    let token = setup_admin(t.router()).await;
    let (status, json) = get_auth(t.router(), "/api/admin/users", &token).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert!(!json.as_array().unwrap().is_empty());
    assert_eq!(json[0]["username"], "admin");
    // Python sqlite emits the INTEGER column — the UI checks `=== '1'`.
    assert_eq!(json[0]["is_admin"], 1);
}

#[tokio::test]
async fn admin_create_user_validates_and_stores_subsonic_copy() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;

    // Weak password → Python policy message.
    let (status, json) = post_auth(
        app,
        "/api/admin/users",
        &token,
        Some(r#"{"username":"weak","password":"short"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["detail"], "Password must be at least 8 characters");

    // Happy path.
    let (status, json) = post_auth(
        app,
        "/api/admin/users",
        &token,
        Some(r#"{"username":"carol","password":"TestPass123!","is_admin":false}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["ok"], true);
    assert_eq!(json["username"], "carol");

    // Fernet-encrypted Subsonic copy stored (Python test).
    let row: (Option<String>,) =
        sqlx::query_as("SELECT password_encrypted FROM users WHERE username = 'carol'")
            .fetch_one(&t.pool)
            .await
            .unwrap();
    assert!(row.0.is_some(), "password_encrypted must be set");

    // Duplicate → 400.
    let (status, json) = post_auth(
        app,
        "/api/admin/users",
        &token,
        Some(r#"{"username":"carol","password":"TestPass123!"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["detail"], "Username already taken");
}

#[tokio::test]
async fn admin_toggle_and_delete_guards() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;

    // Root admin is immutable.
    let (status, json) = post_auth(
        app,
        "/api/admin/users/1/admin",
        &token,
        Some(r#"{"is_admin":false}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["detail"], "Cannot change the root admin account");

    // Create + promote a normal user.
    post_auth(
        app,
        "/api/admin/users",
        &token,
        Some(r#"{"username":"dave","password":"TestPass123!"}"#),
    )
    .await;
    let (uid,): (i64,) = sqlx::query_as("SELECT id FROM users WHERE username='dave'")
        .fetch_one(&t.pool)
        .await
        .unwrap();
    let (status, json) = post_auth(
        app,
        &format!("/api/admin/users/{uid}/admin"),
        &token,
        Some(r#"{"is_admin":true}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["is_admin"], true);

    // Unknown user → 404.
    let (status, json) = post_auth(
        app,
        "/api/admin/users/9999/admin",
        &token,
        Some(r#"{"is_admin":true}"#),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["detail"], "User not found");

    // Root delete guard.
    let (status, json) = delete_auth(app, "/api/admin/users/1", &token).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["detail"], "Cannot delete the root admin account");

    // Delete dave → cascades (no sessions left).
    sqlx::query("INSERT INTO sessions (token, user_id, expires_at) VALUES ('tok', ?, 9999999999)")
        .bind(uid)
        .execute(&t.pool)
        .await
        .unwrap();
    let (status, json) = delete_auth(app, &format!("/api/admin/users/{uid}"), &token).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["username"], "dave");
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE user_id = ?")
        .bind(uid)
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "sessions must be cascaded");
}

#[tokio::test]
async fn admin_change_password_resets_login() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;
    let (uid,): (i64,) = sqlx::query_as("SELECT id FROM users WHERE username='admin'")
        .fetch_one(&t.pool)
        .await
        .unwrap();

    // Change bob's password after creating him.
    post_json(
        app,
        "/api/auth/register",
        r#"{"username":"eve","password":"TestPass123!"}"#,
    )
    .await;
    let (bob_uid,): (i64,) = sqlx::query_as("SELECT id FROM users WHERE username='eve'")
        .fetch_one(&t.pool)
        .await
        .unwrap();
    let (status, json) = post_auth(
        app,
        &format!("/api/admin/users/{bob_uid}/password"),
        &token,
        Some(r#"{"password":"NewPass456!"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["username"], "eve");

    // Old password fails, new one works.
    let (status, _) = post_json(
        app,
        "/api/auth/login",
        r#"{"username":"eve","password":"TestPass123!"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = post_json(
        app,
        "/api/auth/login",
        r#"{"username":"eve","password":"NewPass456!"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Unknown target → 404.
    let (status, _) = post_auth(
        app,
        "/api/admin/users/9999/password",
        &token,
        Some(r#"{"password":"NewPass456!"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let _ = uid;
}

#[tokio::test]
async fn admin_failed_logins_and_unlock() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;

    let (uid,): (i64,) = sqlx::query_as("SELECT id FROM users WHERE username='admin'")
        .fetch_one(&t.pool)
        .await
        .unwrap();
    // Simulate a lockout (30 min from the 5th failure).
    sqlx::query("UPDATE users SET failed_attempts = 5, locked_until = ? WHERE id = ?")
        .bind(chrono::Utc::now().timestamp() + 1800)
        .bind(uid)
        .execute(&t.pool)
        .await
        .unwrap();

    let (status, json) = get_auth(app, "/api/admin/failed-logins", &token).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert!(!json.as_array().unwrap().is_empty());
    assert_eq!(json[0]["failed_attempts"], 5);
    assert_eq!(json[0]["locked"], true);

    let (status, json) =
        post_auth(app, &format!("/api/admin/users/{uid}/unlock"), &token, None).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["ok"], true);

    let (status, json) = get_auth(app, "/api/admin/failed-logins", &token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.as_array().unwrap().len(), 0, "cleared: {json}");
}

#[tokio::test]
async fn admin_sessions_list_and_revoke() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;
    post_json(
        app,
        "/api/auth/register",
        r#"{"username":"frank","password":"TestPass123!"}"#,
    )
    .await;
    let (uid,): (i64,) = sqlx::query_as("SELECT id FROM users WHERE username='frank'")
        .fetch_one(&t.pool)
        .await
        .unwrap();
    let (_, login) = post_json(
        app,
        "/api/auth/login",
        r#"{"username":"frank","password":"TestPass123!"}"#,
    )
    .await;
    let session_token = login["token"].as_str().unwrap().to_string();

    let (status, json) = get_auth(app, &format!("/api/admin/users/{uid}/sessions"), &token).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert!(!json.as_array().unwrap().is_empty());
    assert!(json[0]["token"].is_string());
    assert!(json[0]["expires_at"].is_number());

    // Revoke → live session dies.
    let (status, json) =
        delete_auth(app, &format!("/api/admin/users/{uid}/sessions"), &token).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["message"], "All sessions revoked");
    let (status, _) = get_auth(app, "/api/auth/me", &session_token).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ── Metadata edit / delete ────────────────────────────────────────────────────

#[tokio::test]
async fn admin_metadata_edit() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;
    let tid = insert_track(&t.pool, "/music/edit.mp3").await;

    let (status, json) = patch_auth(
        app,
        &format!("/api/admin/tracks/{tid}/metadata"),
        &token,
        r#"{"title":"New Title","year":2020,"track_number":4}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["ok"], true);
    assert_eq!(
        json["updated"],
        json!(["title = ?", "year = ?", "track_number = ?"])
    );

    let row: (String, Option<i64>, Option<i64>) =
        sqlx::query_as("SELECT title, year, track_number FROM tracks WHERE id = ?")
            .bind(tid)
            .fetch_one(&t.pool)
            .await
            .unwrap();
    assert_eq!(row.0, "New Title");
    assert_eq!(row.1, Some(2020));
    assert_eq!(row.2, Some(4));

    // No fields → 400; unknown track → 404.
    let (status, json) = patch_auth(
        app,
        &format!("/api/admin/tracks/{tid}/metadata"),
        &token,
        "{}",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["detail"], "No fields to update");
    let (status, json) = patch_auth(
        app,
        "/api/admin/tracks/9999/metadata",
        &token,
        r#"{"title":"x"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["detail"], "Track not found");
}

#[tokio::test]
async fn admin_delete_track() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;
    let file = t.music_dir.join("deleteme.mp3");
    std::fs::write(&file, b"fake").unwrap();
    let tid = insert_track(&t.pool, &file.to_string_lossy()).await;

    let (status, json) = delete_auth(app, &format!("/api/admin/tracks/{tid}"), &token).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["deleted"], tid);
    assert!(!file.exists(), "file removed from disk");
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE id = ?")
        .bind(tid)
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);

    let (status, json) = delete_auth(app, "/api/admin/tracks/9999", &token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["detail"], "Track not found");
}

// ── Upload ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn admin_upload_valid_and_invalid() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;
    let bytes = std::fs::read(fixture()).unwrap();

    // Valid audio → imported + row created.
    let (status, json) = post_multipart(
        app,
        "/api/admin/upload",
        &token,
        &[("files", Some("tagged.mp3"), &bytes)],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["imported"], json!(["tagged.mp3"]));
    let dest = t.music_dir.join("tagged.mp3");
    assert!(dest.is_file());
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE file_path = ?")
        .bind(dest.to_string_lossy().to_string())
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    // Non-audio content → 415 with the Python message.
    let (status, json) = post_multipart(
        app,
        "/api/admin/upload",
        &token,
        &[("files", Some("junk.mp3"), b"definitely not audio")],
    )
    .await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE, "{json}");
    assert_eq!(json["detail"], "Invalid audio file: junk.mp3");
    assert!(!t.music_dir.join("junk.mp3").exists());

    // Unsupported extension → silently skipped (Python behavior).
    let (status, json) = post_multipart(
        app,
        "/api/admin/upload",
        &token,
        &[("files", Some("notes.txt"), b"hello")],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["imported"], json!([]));
    assert!(!t.music_dir.join("notes.txt").exists());
}

#[tokio::test]
async fn admin_upload_size_limit() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;
    // Runtime override via the settings table (DB > config.json/default).
    sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES ('max_upload_size', '10')")
        .execute(&t.pool)
        .await
        .unwrap();
    let bytes = std::fs::read(fixture()).unwrap();

    let (status, json) = post_multipart(
        app,
        "/api/admin/upload",
        &token,
        &[("files", Some("big.mp3"), &bytes)],
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "{json}");
    assert_eq!(json["detail"], "File too large (max 0.0 GB)");
    assert!(!t.music_dir.join("big.mp3").exists());
}

// ── Lyrics ops ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn admin_lyrics_upload_and_delete() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;
    let file = t.music_dir.join("lyr.mp3");
    std::fs::write(&file, b"fake").unwrap();
    let tid = insert_track(&t.pool, &file.to_string_lossy()).await;

    // Non-.lrc → 400.
    let (status, json) = post_multipart(
        app,
        &format!("/api/admin/tracks/{tid}/lyrics-upload"),
        &token,
        &[("file", Some("x.txt"), b"nope")],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
    assert_eq!(json["detail"], "Only .lrc files are accepted");

    // Unknown track → 404.
    let (status, json) = post_multipart(
        app,
        "/api/admin/tracks/9999/lyrics-upload",
        &token,
        &[("file", Some("x.lrc"), b"[00:01.00]hi")],
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["detail"], "Track not found");

    // Valid .lrc → stored in row + companion file.
    let lrc = "[00:01.00]hello\n[00:02.00]world";
    let (status, json) = post_multipart(
        app,
        &format!("/api/admin/tracks/{tid}/lyrics-upload"),
        &token,
        &[("file", Some("lyr.lrc"), lrc.as_bytes())],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["ok"], true);
    assert_eq!(json["lyrics"], lrc);
    let companion = file.with_extension("lrc");
    assert_eq!(std::fs::read_to_string(&companion).unwrap(), lrc);
    let row: (Option<String>,) = sqlx::query_as("SELECT lyrics FROM tracks WHERE id = ?")
        .bind(tid)
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(row.0.as_deref(), Some(lrc));

    // Delete → row NULL + companion gone.
    let (status, json) = delete_auth(app, &format!("/api/admin/tracks/{tid}/lyrics"), &token).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["ok"], true);
    assert!(!companion.exists());
    let row: (Option<String>,) = sqlx::query_as("SELECT lyrics FROM tracks WHERE id = ?")
        .bind(tid)
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(row.0, None);

    // Delete lyrics on unknown track → 404.
    let (status, _) = delete_auth(app, "/api/admin/tracks/9999/lyrics", &token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn admin_lyrics_scan_all_empty_library() {
    let t = test_app().await;
    let token = setup_admin(t.router()).await;
    // Empty library → no LRCLIB network calls, exact counts.
    let (status, json) = post_auth(t.router(), "/api/admin/lyrics/scan-all", &token, None).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["scanned"], 0);
    assert_eq!(json["found"], 0);
    assert_eq!(json["errors"], 0);
}

// ── Albums / artists / covers ─────────────────────────────────────────────────

#[tokio::test]
async fn admin_albums_artists_and_tracks() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;
    let a = insert_track(&t.pool, "/music/1.mp3").await;
    let b = insert_track(&t.pool, "/music/2.mp3").await;
    sqlx::query("UPDATE tracks SET title = 'Song One', track_number = 1 WHERE id = ?")
        .bind(a)
        .execute(&t.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE tracks SET title = 'Song Two', track_number = 2 WHERE id = ?")
        .bind(b)
        .execute(&t.pool)
        .await
        .unwrap();

    let (status, json) = get_auth(app, "/api/admin/albums", &token).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json.as_array().unwrap().len(), 1);
    assert_eq!(json[0]["album"], "Alb");
    assert_eq!(json[0]["artist"], "A");
    assert_eq!(json[0]["track_count"], 2);

    let (status, json) = get_auth(app, "/api/admin/artists", &token).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json[0]["artist"], "A");
    assert_eq!(json[0]["track_count"], 2);
    assert_eq!(json[0]["album_count"], 1);

    let (status, json) = get_auth(app, "/api/admin/albums/Alb", &token).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json.as_array().unwrap().len(), 2);
    assert_eq!(json[0]["title"], "Song One");
    assert_eq!(json[1]["title"], "Song Two");
}

#[tokio::test]
async fn admin_re_extract_covers() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;
    // Real file with an embedded cover.
    let dest = t.music_dir.join("cover.mp3");
    std::fs::copy(fixture(), &dest).unwrap();
    let good = insert_track(&t.pool, &dest.to_string_lossy()).await;
    sqlx::query("UPDATE tracks SET has_cover = 0 WHERE id = ?")
        .bind(good)
        .execute(&t.pool)
        .await
        .unwrap();
    // Row whose file is gone.
    let missing = insert_track(&t.pool, "/nowhere/gone.mp3").await;
    sqlx::query("UPDATE tracks SET has_cover = 0 WHERE id = ?")
        .bind(missing)
        .execute(&t.pool)
        .await
        .unwrap();

    let (status, json) = post_auth(app, "/api/admin/re-extract-covers", &token, None).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["processed"], 1);
    assert_eq!(json["errors"], 1);
    assert_eq!(json["total_checked"], 2);

    let row: (i64, Option<String>) =
        sqlx::query_as("SELECT has_cover, cover_hash FROM tracks WHERE id = ?")
            .bind(good)
            .fetch_one(&t.pool)
            .await
            .unwrap();
    assert_eq!(row.0, 1);
    assert!(row.1.is_some());
    assert!(t
        .covers_dir
        .join(format!("{}.jpg", row.1.unwrap()))
        .is_file());
}
