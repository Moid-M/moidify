//! Phase 5: scanner — metadata, file_hash parity, covers, .lrc, clean scan,
//! admin scanner endpoints, rescan schedule, notify watcher.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use http_body_util::BodyExt;
use serde_json::Value;
use std::path::PathBuf;
use tower::ServiceExt;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tagged.mp3")
}

/// POST with token header, optional raw JSON body.
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

// ── Scan basics ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn scan_empty_dir_via_endpoint() {
    let t = test_app().await;
    let token = setup_admin(t.router()).await;

    let (status, json) = post_auth(t.router(), "/api/admin/rescan", &token, None).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["running"], false);
    assert_eq!(json["files_found"], 0);
    assert_eq!(json["files_imported"], 0);
    assert_eq!(json["errors"], serde_json::json!([]));
    assert!(json["last_scan"].is_string(), "{json}");
}

#[tokio::test]
async fn scan_missing_music_dir_records_error() {
    let t = test_app().await;
    let token = setup_admin(t.router()).await;
    std::fs::remove_dir_all(&t.music_dir).unwrap();

    let (status, json) = post_auth(t.router(), "/api/admin/rescan", &token, None).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["running"], false);
    assert_eq!(json["errors"][0], "Music directory does not exist");
    assert!(json["last_scan"].is_string());
}

// ── Fixture processing (golden values from Python mutagen) ───────────────────

/// Python-parity expectations for `tests/fixtures/tagged.mp3`, captured by
/// running the Python `extract_metadata()` / `get_file_hash()` against the
/// same file:
///   title "Fixture Song", artist "Fixture Artist",
///   album_artist "Fixture Album Artist", album "Fixture Album",
///   track 3, disc 1, genre "Test Genre", year 2019, duration 1,
///   lyrics None, cover 247-byte JPEG → 8b07838b90d24871,
///   file_hash e266f1a53c72403710f2b3865db0ba529da9f08c2181296471b1f0c35b04ea57
const FIXTURE_FILE_HASH: &str = "e266f1a53c72403710f2b3865db0ba529da9f08c2181296471b1f0c35b04ea57";
const FIXTURE_COVER_HASH: &str = "8b07838b90d24871";

#[tokio::test]
#[allow(clippy::type_complexity)]
async fn scan_processes_fixture_with_python_parity() {
    let t = test_app().await;
    let token = setup_admin(t.router()).await;
    let dest = t.music_dir.join("tagged.mp3");
    std::fs::copy(fixture(), &dest).unwrap();

    let (status, json) = post_auth(t.router(), "/api/admin/rescan", &token, None).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["files_found"], 1);
    assert_eq!(json["files_imported"], 1);
    assert_eq!(json["errors"], serde_json::json!([]));

    let row: (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<i64>,
        Option<i64>,
        Option<String>,
        Option<i64>,
        Option<f64>,
        Option<i64>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        r#"SELECT file_hash, title, artist, album_artist, album, track_number, disc_number,
                  genre, year, duration, has_cover, lyrics, cover_hash, file_path
           FROM tracks"#,
    )
    .fetch_one(&t.pool)
    .await
    .unwrap();

    assert_eq!(row.0, FIXTURE_FILE_HASH, "file_hash parity");
    assert_eq!(row.1.as_deref(), Some("Fixture Song"));
    assert_eq!(row.2.as_deref(), Some("Fixture Artist"));
    assert_eq!(row.3.as_deref(), Some("Fixture Album Artist"));
    assert_eq!(row.4.as_deref(), Some("Fixture Album"));
    assert_eq!(row.5, Some(3), "track_number (TRCK 3/12)");
    assert_eq!(row.6, Some(1), "disc_number (TPOS 1/2)");
    assert_eq!(row.7.as_deref(), Some("Test Genre"));
    assert_eq!(row.8, Some(2019), "year (TDRC 2019-05-01)");
    assert_eq!(row.9, Some(1.0), "duration truncated like int(info.length)");
    assert_eq!(row.10, Some(1), "has_cover");
    assert_eq!(row.11, None, "no lyrics");
    assert_eq!(row.12.as_deref(), Some(FIXTURE_COVER_HASH));
    assert_eq!(row.13.as_deref(), Some(dest.to_string_lossy().as_ref()));

    // Cover saved with sniffed extension and exact bytes.
    let cover = t.covers_dir.join(format!("{FIXTURE_COVER_HASH}.jpg"));
    assert!(cover.is_file(), "cover file missing: {}", cover.display());
    assert_eq!(std::fs::metadata(&cover).unwrap().len(), 247);

    // Second scan: existing file skipped (no error, counted as imported).
    let (status, json) = post_auth(t.router(), "/api/admin/rescan", &token, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["files_found"], 1);
    assert_eq!(json["files_imported"], 1);
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks")
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(count.0, 1, "no duplicate rows");
}

#[tokio::test]
async fn existing_track_skipped_unless_clean() {
    let t = test_app().await;
    let token = setup_admin(t.router()).await;
    std::fs::copy(fixture(), t.music_dir.join("tagged.mp3")).unwrap();

    post_auth(t.router(), "/api/admin/rescan", &token, None).await;

    // Tamper with the row — a plain rescan must NOT overwrite it (Python
    // `if existing and not force: return`).
    sqlx::query("UPDATE tracks SET title = 'Hacked'")
        .execute(&t.pool)
        .await
        .unwrap();
    let (_, json) = post_auth(t.router(), "/api/admin/rescan", &token, None).await;
    assert_eq!(json["files_found"], 1);
    let title: (String,) = sqlx::query_as("SELECT title FROM tracks")
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(title.0, "Hacked");

    // A favorite + playlist row must be wiped by clean=True.
    let uid = sqlx::query(
        "INSERT INTO users (username, password_hash, salt, is_admin) VALUES ('u','h','s',0)",
    )
    .execute(&t.pool)
    .await
    .unwrap()
    .last_insert_rowid();
    let tid: (i64,) = sqlx::query_as("SELECT id FROM tracks")
        .fetch_one(&t.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO favorites (user_id, track_id) VALUES (?, ?)")
        .bind(uid)
        .bind(tid.0)
        .execute(&t.pool)
        .await
        .unwrap();

    let (_, json) = post_auth(
        t.router(),
        "/api/admin/rescan",
        &token,
        Some(r#"{"clean":true}"#),
    )
    .await;
    assert_eq!(json["files_found"], 1, "{json}");
    assert_eq!(json["files_imported"], 1, "{json}");

    // Re-extracted from the file.
    let title: (String,) = sqlx::query_as("SELECT title FROM tracks")
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(title.0, "Fixture Song");
    // Favorites wiped (and not re-created by the scan).
    let favs: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM favorites")
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(favs.0, 0);
    // Cover regenerated after the clean wipe of covers_dir.
    let cover = t.covers_dir.join(format!("{FIXTURE_COVER_HASH}.jpg"));
    assert!(cover.is_file());
}

// ── file_hash parity ─────────────────────────────────────────────────────────

#[tokio::test]
async fn hash_parity_small_file() {
    let t = test_app().await;
    let path = t.music_dir.join("small.bin");
    std::fs::write(&path, b"moidify-hash-parity").unwrap();
    // Python get_file_hash() on the same 19-byte file.
    let expected = "8178a210e5485d1e291fce7854c3645d4620b35125d2e433765415f910394e0d";
    let got = moidify::scanner::get_file_hash(&path).unwrap();
    assert_eq!(got, expected);
}

#[tokio::test]
async fn hash_parity_large_file_uses_head_and_tail() {
    let t = test_app().await;
    let path = t.music_dir.join("large.bin");
    let data: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
    std::fs::write(&path, &data).unwrap();
    // Python get_file_hash(): first 64 KiB + last 64 KiB + size, > 128 KiB.
    let expected = "c1387185c579720c2f88bc6a077066c300f08723a01d9e340abaf04e43aa9bde";
    let got = moidify::scanner::get_file_hash(&path).unwrap();
    assert_eq!(got, expected);
}

// ── Fallbacks and .lrc sidecars ──────────────────────────────────────────────

#[tokio::test]
async fn unparseable_mp3_gets_unknown_defaults() {
    let t = test_app().await;
    let token = setup_admin(t.router()).await;
    std::fs::write(t.music_dir.join("junk.mp3"), b"definitely not audio bytes").unwrap();

    let (_, json) = post_auth(t.router(), "/api/admin/rescan", &token, None).await;
    assert_eq!(json["files_found"], 1, "{json}");
    assert_eq!(json["files_imported"], 1, "{json}");
    assert_eq!(json["errors"], serde_json::json!([]));

    let row: (
        String,
        Option<String>,
        Option<String>,
        Option<f64>,
        Option<i64>,
        String,
    ) = sqlx::query_as("SELECT title, artist, album, duration, has_cover, file_hash FROM tracks")
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(row.0, "junk", "title falls back to the file stem");
    assert_eq!(row.1.as_deref(), Some("Unknown"));
    assert_eq!(row.2.as_deref(), Some("Unknown"));
    assert_eq!(row.3, Some(0.0));
    assert_eq!(row.4, Some(0));
    // Hashing works regardless of parseability.
    let expected = moidify::scanner::get_file_hash(&t.music_dir.join("junk.mp3")).unwrap();
    assert_eq!(row.5, expected);
}

#[tokio::test]
async fn companion_lrc_file_supplies_lyrics() {
    let t = test_app().await;
    let token = setup_admin(t.router()).await;
    std::fs::copy(fixture(), t.music_dir.join("song.mp3")).unwrap();
    std::fs::write(
        t.music_dir.join("song.lrc"),
        "[00:01.00]hello there\n[00:02.00]second line",
    )
    .unwrap();

    let (_, json) = post_auth(t.router(), "/api/admin/rescan", &token, None).await;
    assert_eq!(json["files_found"], 1, "{json}");

    let lyrics: (Option<String>,) = sqlx::query_as("SELECT lyrics FROM tracks")
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(
        lyrics.0.as_deref(),
        Some("[00:01.00]hello there\n[00:02.00]second line")
    );
}

// ── Admin endpoint auth ──────────────────────────────────────────────────────

#[tokio::test]
async fn scanner_endpoints_require_admin() {
    let t = test_app().await;
    let app = t.router();

    // Setup first (registration gate), then exercise the auth matrix.
    let admin = setup_admin(app).await;

    // Anonymous → 401.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/admin/scanner")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Non-admin → 403.
    let bob = second_user(app).await;
    let (status, json) = get_auth(app, "/api/admin/scanner", &bob).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(json["detail"], "Admin access required");

    // Admin → 200 with the status shape.
    let (status, json) = get_auth(app, "/api/admin/scanner", &admin).await;
    assert_eq!(status, StatusCode::OK);
    for key in [
        "running",
        "last_scan",
        "files_found",
        "files_imported",
        "errors",
    ] {
        assert!(json.get(key).is_some(), "missing {key}: {json}");
    }

    // Rescan also admin-only.
    let (status, _) = post_auth(app, "/api/admin/rescan", &bob, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = post_auth(app, "/api/admin/rescan", "bogus", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

async fn second_user(app: &axum::Router) -> String {
    let (status, json) = post_json(
        app,
        "/api/auth/register",
        r#"{"username":"bob","password":"TestPass123!"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "register: {json}");
    let (status, json) = post_json(
        app,
        "/api/auth/login",
        r#"{"username":"bob","password":"TestPass123!"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "login: {json}");
    json["token"].as_str().expect("token").to_string()
}

// ── Rescan schedule ──────────────────────────────────────────────────────────

#[tokio::test]
async fn schedule_rescan_endpoints() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;

    // Python default when the file is missing.
    let (status, json) = get_auth(app, "/api/admin/schedule-rescan", &token).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["interval_hours"], 0);
    assert!(json["last_scan"].is_null());

    // Negative interval → 400 with the Python message.
    let (status, json) = post_auth(
        app,
        "/api/admin/schedule-rescan",
        &token,
        Some(r#"{"interval_hours":-1}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["detail"], "Interval must be >= 0");

    // Set + read back (float, like pydantic).
    let (status, json) = post_auth(
        app,
        "/api/admin/schedule-rescan",
        &token,
        Some(r#"{"interval_hours":8}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["ok"], true);
    assert_eq!(json["interval_hours"], 8.0);

    let (status, json) = get_auth(app, "/api/admin/schedule-rescan", &token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["interval_hours"], 8.0);

    // Persisted next to the DB (Python: data/rescan_schedule.json).
    let schedule = t
        .state
        .config
        .db_path
        .parent()
        .unwrap()
        .join("rescan_schedule.json");
    assert!(schedule.is_file(), "schedule file missing");
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&schedule).unwrap()).unwrap();
    assert_eq!(saved["interval_hours"], 8.0);

    // Auth gates.
    let (status, _) = get_auth(app, "/api/admin/schedule-rescan", "bogus").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = post_auth(
        app,
        "/api/admin/schedule-rescan",
        "bogus",
        Some(r#"{"interval_hours":1}"#),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ── Watcher (notify) ─────────────────────────────────────────────────────────

#[tokio::test]
async fn watcher_imports_created_file_and_removes_deleted() {
    let t = test_app().await;
    moidify::scanner::spawn_watcher(&t.state).expect("start watcher");

    let dest = t.music_dir.join("watched.mp3");
    std::fs::copy(fixture(), &dest).unwrap();

    // Create → 2s settle + process; poll for the row.
    let mut imported = false;
    for _ in 0..60 {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE file_path = ?")
            .bind(dest.to_string_lossy().to_string())
            .fetch_one(&t.pool)
            .await
            .unwrap();
        if count.0 == 1 {
            imported = true;
            break;
        }
    }
    assert!(imported, "watcher did not import the created file");
    let title: (Option<String>,) = sqlx::query_as("SELECT title FROM tracks WHERE file_path = ?")
        .bind(dest.to_string_lossy().to_string())
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(title.0.as_deref(), Some("Fixture Song"));

    // Delete → row removed.
    std::fs::remove_file(&dest).unwrap();
    let mut removed = false;
    for _ in 0..40 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE file_path = ?")
            .bind(dest.to_string_lossy().to_string())
            .fetch_one(&t.pool)
            .await
            .unwrap();
        if count.0 == 0 {
            removed = true;
            break;
        }
    }
    assert!(removed, "watcher did not remove the deleted file");
}
