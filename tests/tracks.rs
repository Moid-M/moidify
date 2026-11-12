mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{get, setup_admin, test_app};
use http_body_util::BodyExt;
use tower::ServiceExt;

/// Raw GET returning status + headers + body bytes (for HTML/binary checks).
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

/// Raw PUT JSON without auth helper.
async fn put_json(app: &axum::Router, uri: &str, body: &str) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
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

// ── Empty-library listings (Python test_tracks_empty et al.) ────────────────

#[tokio::test]
async fn tracks_empty_returns_bare_array() {
    let t = test_app().await;
    let (status, json) = get(t.router(), "/api/tracks").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json, serde_json::json!([]));
}

#[tokio::test]
async fn tracks_with_limit_returns_envelope() {
    let t = test_app().await;
    let (status, json) = get(t.router(), "/api/tracks?limit=10").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["tracks"], serde_json::json!([]));
    assert_eq!(json["total"], 0);
    assert_eq!(json["limit"], 10);
    assert_eq!(json["offset"], 0);
}

#[tokio::test]
async fn albums_empty_returns_bare_array() {
    let t = test_app().await;
    let (status, json) = get(t.router(), "/api/albums").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json, serde_json::json!([]));
}

#[tokio::test]
async fn albums_grouped_empty_returns_bare_array() {
    let t = test_app().await;
    let (status, json) = get(t.router(), "/api/albums?grouped=true").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json, serde_json::json!([]));
}

#[tokio::test]
async fn artists_empty_returns_bare_array() {
    let t = test_app().await;
    let (status, json) = get(t.router(), "/api/artists").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json, serde_json::json!([]));
}

#[tokio::test]
async fn genres_empty_returns_bare_array() {
    let t = test_app().await;
    let (status, json) = get(t.router(), "/api/genres").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json, serde_json::json!([]));
}

#[tokio::test]
async fn duplicates_empty_returns_bare_array() {
    let t = test_app().await;
    let (status, json) = get(t.router(), "/api/duplicates").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json, serde_json::json!([]));
}

#[tokio::test]
async fn home_feed_returns_expected_keys() {
    let t = test_app().await;
    let (status, json) = get(t.router(), "/api/home").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["recently_played"].is_array());
    assert!(json["recommended_albums"].is_array());
    assert!(json["recommended_tracks"].is_array());
    assert!(json["recommended_artists"].is_array());
    assert!(json["playlists"].is_array());
}

#[tokio::test]
async fn search_empty_query_returns_bare_array() {
    let t = test_app().await;
    let (status, json) = get(t.router(), "/api/tracks?search=").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json, serde_json::json!([]));
}

#[tokio::test]
async fn album_tracks_missing_album_returns_empty() {
    let t = test_app().await;
    let (status, json) = get(t.router(), "/api/albums/tracks?album=Nope").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json, serde_json::json!([]));
}

#[tokio::test]
async fn artist_tracks_missing_artist_returns_empty() {
    let t = test_app().await;
    let (status, json) = get(t.router(), "/api/artists/tracks?artist=Nope").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json, serde_json::json!([]));
}

#[tokio::test]
async fn genre_tracks_missing_genre_returns_empty() {
    let t = test_app().await;
    let (status, json) = get(t.router(), "/api/genres/tracks?genre=Nope").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json, serde_json::json!([]));
}

// ── Track detail / similar / gain 404s ───────────────────────────────────────

#[tokio::test]
async fn track_missing_returns_404_detail() {
    let t = test_app().await;
    let (status, json) = get(t.router(), "/api/tracks/999").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["detail"], "Track not found");
}

#[tokio::test]
async fn similar_missing_track_returns_404() {
    let t = test_app().await;
    let (status, _) = get(t.router(), "/api/tracks/999/similar").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn gain_missing_track_returns_404() {
    let t = test_app().await;
    let (status, _) = get(t.router(), "/api/tracks/999/gain").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn lyrics_missing_track_returns_404() {
    let t = test_app().await;
    let (status, _) = get(t.router(), "/api/tracks/999/lyrics").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ── Lyrics auth regression (Python test_lyrics_put_requires_auth) ────────────

#[tokio::test]
async fn lyrics_put_requires_auth() {
    let t = test_app().await;
    let (status, json) =
        put_json(t.router(), "/api/tracks/1/lyrics", r#"{"lyrics":"hacked"}"#).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let _ = json;
}

// ── Pages (Python test_index_page / setup / admin) ───────────────────────────

#[tokio::test]
async fn index_page_is_html() {
    let t = test_app().await;
    let (status, headers, body) = get_raw(t.router(), "/").await;
    assert_eq!(status, StatusCode::OK);
    let ct = headers.get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("text/html"), "content-type was {ct}");
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("<html"), "index.html body looks wrong");
}

#[tokio::test]
async fn setup_page_is_html_when_no_admin() {
    let t = test_app().await;
    let (status, headers, _) = get_raw(t.router(), "/setup").await;
    assert_eq!(status, StatusCode::OK);
    let ct = headers.get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("text/html"), "content-type was {ct}");
}

#[tokio::test]
async fn setup_page_redirects_when_admin_exists() {
    let t = test_app().await;
    let _token = setup_admin(t.router()).await;
    let (status, _, body) = get_raw(t.router(), "/setup").await;
    assert_eq!(status, StatusCode::OK);
    let html = String::from_utf8_lossy(&body);
    assert!(
        html.contains("window.location.href='/'"),
        "expected redirect snippet, got: {html}"
    );
}

#[tokio::test]
async fn admin_page_is_html() {
    let t = test_app().await;
    let (status, headers, _) = get_raw(t.router(), "/admin").await;
    assert_eq!(status, StatusCode::OK);
    let ct = headers.get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("text/html"), "content-type was {ct}");
}

#[tokio::test]
async fn shared_pages_are_html() {
    let t = test_app().await;
    for uri in ["/s/some-token", "/a/some-token"] {
        let (status, headers, _) = get_raw(t.router(), uri).await;
        assert_eq!(status, StatusCode::OK, "uri {uri}");
        let ct = headers.get("content-type").unwrap().to_str().unwrap();
        assert!(ct.contains("text/html"), "uri {uri} content-type was {ct}");
    }
}

#[tokio::test]
async fn shared_track_page_404_when_missing() {
    let t = test_app().await;
    let (status, headers, body) = get_raw(t.router(), "/track/999").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let ct = headers.get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("text/html"));
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("Track not found"));
}

#[tokio::test]
async fn now_playing_returns_null_fields_when_empty() {
    let t = test_app().await;
    let (status, json) = get(t.router(), "/api/player/now-playing").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["id"].is_null());
    assert!(json["title"].is_null());
}

// ── Static files (Python test_static_files) ──────────────────────────────────

#[tokio::test]
async fn static_logo_served() {
    let t = test_app().await;
    let (status, headers, body) = get_raw(t.router(), "/static/logo.png").await;
    assert_eq!(status, StatusCode::OK);
    let ct = headers.get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("image/png"), "content-type was {ct}");
    assert!(!body.is_empty());
}

// ── Security headers layer ───────────────────────────────────────────────────

#[tokio::test]
async fn security_headers_present() {
    let t = test_app().await;
    let (status, headers, _) = get_raw(t.router(), "/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
    assert_eq!(headers.get("x-frame-options").unwrap(), "DENY");
    assert!(headers.contains_key("content-security-policy"));
    assert!(headers.contains_key("referrer-policy"));
    assert!(headers.contains_key("permissions-policy"));
}
