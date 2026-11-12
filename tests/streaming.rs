mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::test_app;
use http_body_util::BodyExt;
use tower::ServiceExt;

/// Raw GET returning status + headers + body bytes.
async fn get_raw(app: &axum::Router, uri: &str) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let response = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, headers, bytes.to_vec())
}

/// Raw GET with a Range header.
async fn get_range(
    app: &axum::Router,
    uri: &str,
    range: &str,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri)
                .header("range", range)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, headers, bytes.to_vec())
}

/// Insert a track row pointing at `file_path` and return its id.
async fn insert_track(pool: &sqlx::SqlitePool, file_path: &str) -> i64 {
    // last_insert_rowid() is per-connection — use the execute result instead.
    let result =
        sqlx::query("INSERT INTO tracks (file_path, title, artist, album) VALUES (?, ?, ?, ?)")
            .bind(file_path)
            .bind("Test Title")
            .bind("Test Artist")
            .bind("Test Album")
            .execute(pool)
            .await
            .expect("insert track");
    result.last_insert_rowid()
}

// ── Stream: missing rows / files / path safety ──────────────────────────────

#[tokio::test]
async fn stream_missing_track_404() {
    let t = test_app().await;
    let (status, _, body) = get_raw(t.router(), "/api/stream/999?quality=original").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["detail"], "Track not found");
}

#[tokio::test]
async fn stream_missing_file_404() {
    let t = test_app().await;
    let ghost = t.music_dir.join("does-not-exist.mp3");
    let id = insert_track(&t.pool, ghost.to_str().unwrap()).await;
    let (status, _, body) =
        get_raw(t.router(), &format!("/api/stream/{id}?quality=original")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["detail"], "File not found on disk");
}

#[tokio::test]
async fn stream_outside_music_dir_403() {
    let t = test_app().await;
    // Real file, but outside the temp music dir (path traversal / escape).
    let outside = std::env::temp_dir().join("moidify-escape-test.mp3");
    std::fs::write(&outside, b"ID3 outside").unwrap();
    let id = insert_track(&t.pool, outside.to_str().unwrap()).await;

    let (status, _, body) =
        get_raw(t.router(), &format!("/api/stream/{id}?quality=original")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["detail"], "Forbidden");

    let _ = std::fs::remove_file(&outside);
}

#[tokio::test]
async fn stream_traversal_dotdot_403() {
    let t = test_app().await;
    // file_path that canonicalizes outside music dir via ..
    let sneaky = t.music_dir.join("../escape.mp3");
    std::fs::write(
        sneaky.canonicalize().unwrap_or_else(|_| {
            let p = t.music_dir.parent().unwrap().join("escape.mp3");
            std::fs::write(&p, b"x").unwrap();
            p
        }),
        b"x",
    )
    .ok();
    // Ensure the file exists at the canonical location
    let outside = t.music_dir.parent().unwrap().join("escape.mp3");
    if !outside.exists() {
        std::fs::write(&outside, b"x").unwrap();
    }
    let id = insert_track(&t.pool, sneaky.to_str().unwrap()).await;

    let (status, _, _) = get_raw(t.router(), &format!("/api/stream/{id}?quality=original")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let _ = std::fs::remove_file(&outside);
}

// ── Stream: happy paths ─────────────────────────────────────────────────────

#[tokio::test]
async fn stream_original_serves_file_without_auth() {
    let t = test_app().await;
    let file = t.music_dir.join("song.mp3");
    // Minimal bytes — content doesn't matter for ServeFile.
    std::fs::write(&file, b"ID3\x03\x00\x00\x00\x00\x00\x00fake-mp3-bytes").unwrap();
    let id = insert_track(&t.pool, file.to_str().unwrap()).await;

    // No token header — anonymous by design.
    let (status, headers, body) =
        get_raw(t.router(), &format!("/api/stream/{id}?quality=original")).await;
    assert_eq!(status, StatusCode::OK);
    let ct = headers.get("content-type").unwrap().to_str().unwrap();
    assert_eq!(ct, "audio/mpeg");
    assert_eq!(
        headers.get("accept-ranges").unwrap(),
        "bytes",
        "ServeFile must advertise Range support"
    );
    assert_eq!(body, b"ID3\x03\x00\x00\x00\x00\x00\x00fake-mp3-bytes");
}

#[tokio::test]
async fn stream_original_supports_range_requests() {
    let t = test_app().await;
    let file = t.music_dir.join("ranged.flac");
    std::fs::write(&file, b"0123456789ABCDEF").unwrap();
    let id = insert_track(&t.pool, file.to_str().unwrap()).await;

    let (status, headers, body) = get_range(
        t.router(),
        &format!("/api/stream/{id}?quality=original"),
        "bytes=0-3",
    )
    .await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(headers.get("content-range").unwrap(), "bytes 0-3/16");
    assert_eq!(body, b"0123");
}

#[tokio::test]
async fn stream_high_quality_transcodes_with_header_when_ffmpeg_present() {
    let t = test_app().await;
    let file = t.music_dir.join("t.ogg");
    // Tiny valid-ish file; ffmpeg may fail → fallback still returns 200.
    std::fs::write(&file, b"OggS\x00\x02").unwrap();
    let id = insert_track(&t.pool, file.to_str().unwrap()).await;

    let (status, headers, _) = get_raw(t.router(), &format!("/api/stream/{id}?quality=high")).await;
    assert_eq!(status, StatusCode::OK);

    let ffmpeg = std::process::Command::new("which")
        .arg("ffmpeg")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if ffmpeg {
        let xt = headers.get("x-transcoded").unwrap().to_str().unwrap();
        assert_eq!(xt, "high");
        let ct = headers.get("content-type").unwrap().to_str().unwrap();
        assert_eq!(ct, "audio/ogg");
        assert_eq!(headers.get("cache-control").unwrap(), "no-cache");
        assert_eq!(headers.get("content-disposition").unwrap(), "inline");
    } else {
        // No ffmpeg: plain file fallback, no transcode marker (Python parity).
        assert!(headers.get("x-transcoded").is_none());
    }
}

#[tokio::test]
async fn stream_quality_original_never_sets_transcoded_header() {
    let t = test_app().await;
    let file = t.music_dir.join("orig.mp3");
    std::fs::write(&file, b"ID3orig").unwrap();
    let id = insert_track(&t.pool, file.to_str().unwrap()).await;

    let (status, headers, _) =
        get_raw(t.router(), &format!("/api/stream/{id}?quality=original")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers.get("x-transcoded").is_none());
}

#[tokio::test]
async fn stream_default_quality_is_high() {
    let t = test_app().await;
    let file = t.music_dir.join("default.mp3");
    std::fs::write(&file, b"ID3def").unwrap();
    let id = insert_track(&t.pool, file.to_str().unwrap()).await;

    // No ?quality= → Python defaults to "high".
    let (status, headers, _) = get_raw(t.router(), &format!("/api/stream/{id}")).await;
    assert_eq!(status, StatusCode::OK);
    let ffmpeg = std::process::Command::new("which")
        .arg("ffmpeg")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if ffmpeg {
        assert_eq!(headers.get("x-transcoded").unwrap(), "high");
    }
}

// ── Cover ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn cover_missing_track_falls_back_to_placeholder() {
    let t = test_app().await;
    let (status, headers, body) = get_raw(t.router(), "/api/cover/999").await;
    assert_eq!(status, StatusCode::OK);
    let ct = headers.get("content-type").unwrap().to_str().unwrap();
    assert_eq!(ct, "image/svg+xml");
    assert_eq!(
        headers.get("cache-control").unwrap(),
        "public, max-age=3600"
    );
    let svg = String::from_utf8_lossy(&body);
    assert!(svg.contains("<svg"), "expected placeholder SVG");
}

#[tokio::test]
async fn cover_by_track_id_fallback() {
    let t = test_app().await;
    let id = insert_track(&t.pool, "/nonexistent/cover-owner.mp3").await;
    let png = t.covers_dir.join(format!("{id}.png"));
    std::fs::write(&png, b"\x89PNG-fake").unwrap();

    let (status, headers, body) = get_raw(t.router(), &format!("/api/cover/{id}")).await;
    assert_eq!(status, StatusCode::OK);
    let ct = headers.get("content-type").unwrap().to_str().unwrap();
    assert_eq!(ct, "image/png");
    assert_eq!(
        headers.get("cache-control").unwrap(),
        "public, max-age=86400"
    );
    assert_eq!(body, b"\x89PNG-fake");
}

#[tokio::test]
async fn cover_by_cover_hash_wins_over_track_id() {
    let t = test_app().await;
    let id = insert_track(&t.pool, "/nonexistent/hash-owner.mp3").await;
    sqlx::query("UPDATE tracks SET cover_hash = ? WHERE id = ?")
        .bind("abc123hash")
        .bind(id)
        .execute(&t.pool)
        .await
        .unwrap();

    // Both files exist; hash file must win.
    std::fs::write(t.covers_dir.join("abc123hash.jpg"), b"HASH-JPG").unwrap();
    std::fs::write(t.covers_dir.join(format!("{id}.png")), b"ID-PNG").unwrap();

    let (status, headers, body) = get_raw(t.router(), &format!("/api/cover/{id}")).await;
    assert_eq!(status, StatusCode::OK);
    let ct = headers.get("content-type").unwrap().to_str().unwrap();
    assert_eq!(ct, "image/jpeg");
    assert_eq!(body, b"HASH-JPG");
}

#[tokio::test]
async fn cover_hash_missing_file_falls_back_to_track_id() {
    let t = test_app().await;
    let id = insert_track(&t.pool, "/nonexistent/partial.mp3").await;
    sqlx::query("UPDATE tracks SET cover_hash = ? WHERE id = ?")
        .bind("no-such-hash")
        .bind(id)
        .execute(&t.pool)
        .await
        .unwrap();
    std::fs::write(t.covers_dir.join(format!("{id}.webp")), b"WEBP").unwrap();

    let (status, _, body) = get_raw(t.router(), &format!("/api/cover/{id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, b"WEBP");
}
