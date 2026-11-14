//! Phase 4: playlists, folders, favorites, player state, shares, export/import.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use http_body_util::BodyExt;
use tower::ServiceExt;

/// Insert a track row and return its id.
async fn insert_track(pool: &sqlx::SqlitePool, file_path: &str, title: &str) -> i64 {
    let result =
        sqlx::query("INSERT INTO tracks (file_path, title, artist, album) VALUES (?, ?, ?, ?)")
            .bind(file_path)
            .bind(title)
            .bind("Phase Four")
            .bind("Phase Album")
            .execute(pool)
            .await
            .expect("insert track");
    result.last_insert_rowid()
}

/// Register a second user (after setup) and return a login token.
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

/// POST JSON with token header.
async fn post_auth_json(
    app: &axum::Router,
    uri: &str,
    token: &str,
    body: &str,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .header("token", token)
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

/// PUT JSON with token header.
async fn put_auth_json(
    app: &axum::Router,
    uri: &str,
    token: &str,
    body: &str,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(uri)
                .header("content-type", "application/json")
                .header("token", token)
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

/// DELETE with token header (no body).
async fn del_auth(app: &axum::Router, uri: &str, token: &str) -> (StatusCode, serde_json::Value) {
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
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

/// DELETE JSON with token header (body-accepting, e.g. album unshare).
async fn del_auth_json(
    app: &axum::Router,
    uri: &str,
    token: &str,
    body: &str,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(uri)
                .header("content-type", "application/json")
                .header("token", token)
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

/// Create a playlist and return its id.
async fn create_playlist(app: &axum::Router, token: &str, name: &str) -> i64 {
    let (status, json) = post_auth_json(
        app,
        "/api/playlists",
        token,
        &format!(r#"{{"name":"{name}"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create playlist: {json}");
    json["id"].as_i64().expect("playlist id")
}

// ── Playlists CRUD ───────────────────────────────────────────────────────────

#[tokio::test]
async fn playlists_empty_without_token() {
    let t = test_app().await;
    let (status, json) = get(t.router(), "/api/playlists").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json, serde_json::json!([]));
}

#[tokio::test]
async fn create_and_list_playlist() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;

    let pid = create_playlist(app, &token, "My Mix").await;
    let (status, json) = get_auth(app, "/api/playlists", &token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.as_array().unwrap().len(), 1);
    assert_eq!(json[0]["id"], pid);
    assert_eq!(json[0]["name"], "My Mix");
    assert_eq!(json[0]["track_count"], 0);
    assert!(json[0]["folder_id"].is_null());
    assert!(json[0]["created_at"].is_string());
}

#[tokio::test]
async fn create_playlist_requires_auth() {
    let t = test_app().await;
    let (status, json) = post_json(t.router(), "/api/playlists", r#"{"name":"x"}"#).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(json["detail"].is_string());
}

#[tokio::test]
async fn delete_playlist_unknown_404() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;
    let (status, json) = del_auth(app, "/api/playlists/999", &token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["detail"], "Playlist not found");
}

#[tokio::test]
async fn playlist_ownership_never_leaks_existence() {
    let t = test_app().await;
    let app = t.router();
    let alice = setup_admin(app).await;
    let bob = second_user(app).await;

    let pid = create_playlist(app, &alice, "Alice Secret").await;

    // Bob gets 404, not 403 — same as Python.
    let (status, json) = del_auth(app, &format!("/api/playlists/{pid}"), &bob).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["detail"], "Playlist not found");

    let (status, _) = post_auth_json(
        app,
        &format!("/api/playlists/{pid}/tracks"),
        &bob,
        r#"{"track_id":1}"#,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Playlist still exists for Alice.
    let (status, _) = get_auth(app, "/api/playlists", &alice).await;
    assert_eq!(status, StatusCode::OK);
}

// ── Playlist tracks ──────────────────────────────────────────────────────────

#[tokio::test]
async fn playlist_tracks_public_read_and_positions() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;
    let t1 = insert_track(&t.pool, "/m/a.mp3", "Song A").await;
    let t2 = insert_track(&t.pool, "/m/b.mp3", "Song B").await;
    let pid = create_playlist(app, &token, "Public?").await;

    let (status, _) = post_auth_json(
        app,
        &format!("/api/playlists/{pid}/tracks"),
        &token,
        &format!(r#"{{"track_id":{t1}}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = post_auth_json(
        app,
        &format!("/api/playlists/{pid}/tracks"),
        &token,
        &format!(r#"{{"track_id":{t2}}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // No auth on read (Python parity).
    let (status, json) = get(app, &format!("/api/playlists/{pid}/tracks")).await;
    assert_eq!(status, StatusCode::OK);
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["id"], t1);
    assert_eq!(arr[0]["position"], 1);
    assert_eq!(arr[1]["id"], t2);
    assert_eq!(arr[1]["position"], 2);
    assert!(arr[0]["added_at"].is_string() || arr[0]["added_at"].is_null());

    // Reorder → positions flip.
    let (status, _) = put_auth_json(
        app,
        &format!("/api/playlists/{pid}/tracks/reorder"),
        &token,
        &format!(r#"{{"order":[{t2},{t1}]}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, json) = get(app, &format!("/api/playlists/{pid}/tracks")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json[0]["id"], t2);
    assert_eq!(json[0]["position"], 1);
    assert_eq!(json[1]["position"], 2);

    // Remove one.
    let (status, _) = del_auth(app, &format!("/api/playlists/{pid}/tracks/{t2}"), &token).await;
    assert_eq!(status, StatusCode::OK);
    let (status, json) = get(app, &format!("/api/playlists/{pid}/tracks")).await;
    assert_eq!(json.as_array().unwrap().len(), 1);
    assert_eq!(status, StatusCode::OK);
}

// ── Favorites ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn favorites_require_auth_for_list() {
    let t = test_app().await;
    let (status, json) = get(t.router(), "/api/favorites").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(json["detail"].is_string());
}

#[tokio::test]
async fn favorites_check_anonymous_is_false() {
    let t = test_app().await;
    let tid = insert_track(&t.pool, "/m/c.mp3", "Song C").await;
    let (status, json) = get(t.router(), &format!("/api/favorites/check/{tid}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["favorite"], false);
}

#[tokio::test]
async fn favorites_roundtrip() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;
    let tid = insert_track(&t.pool, "/m/d.mp3", "Song D").await;

    let (status, json) = post_auth_json(app, &format!("/api/favorites/{tid}"), &token, "{}").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["ok"], true);

    let (status, json) = get_auth(app, &format!("/api/favorites/check/{tid}"), &token).await;
    assert_eq!(json["favorite"], true);
    assert_eq!(status, StatusCode::OK);

    let (status, json) = get_auth(app, "/api/favorites", &token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.as_array().unwrap().len(), 1);
    assert_eq!(json[0]["id"], tid);

    let (status, _) = del_auth(app, &format!("/api/favorites/{tid}"), &token).await;
    assert_eq!(status, StatusCode::OK);
    let (status, json) = get_auth(app, &format!("/api/favorites/check/{tid}"), &token).await;
    assert_eq!(json["favorite"], false);
    assert_eq!(status, StatusCode::OK);
}

// ── Playlist sharing ─────────────────────────────────────────────────────────

#[tokio::test]
async fn share_status_requires_auth() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;
    let pid = create_playlist(app, &token, "S").await;
    let (status, _) = get(app, &format!("/api/playlists/{pid}/share")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn playlist_share_roundtrip() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;
    let tid = insert_track(&t.pool, "/m/e.mp3", "Song E").await;
    let pid = create_playlist(app, &token, "Shared Mix").await;
    post_auth_json(
        app,
        &format!("/api/playlists/{pid}/tracks"),
        &token,
        &format!(r#"{{"track_id":{tid}}}"#),
    )
    .await;

    let (status, json) =
        post_auth_json(app, &format!("/api/playlists/{pid}/share"), &token, "{}").await;
    assert_eq!(status, StatusCode::OK);
    let share = json["token"].as_str().expect("share token").to_string();
    assert!(!share.is_empty());

    // Sharing again returns the same token.
    let (status, json) =
        post_auth_json(app, &format!("/api/playlists/{pid}/share"), &token, "{}").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["token"], share);

    // Status reflects it.
    let (status, json) = get_auth(app, &format!("/api/playlists/{pid}/share"), &token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["shared"], true);
    assert_eq!(json["token"], share);

    // Public read via token.
    let (status, json) = get(app, &format!("/api/shared/{share}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["name"], "Shared Mix");
    assert_eq!(json["username"], "admin");
    assert_eq!(json["tracks"].as_array().unwrap().len(), 1);
    assert_eq!(json["tracks"][0]["id"], tid);

    // Unshare → status false, public read 404.
    let (status, _) = del_auth(app, &format!("/api/playlists/{pid}/share"), &token).await;
    assert_eq!(status, StatusCode::OK);
    let (status, json) = get_auth(app, &format!("/api/playlists/{pid}/share"), &token).await;
    assert_eq!(json["shared"], false);
    assert!(json["token"].is_null());
    assert_eq!(status, StatusCode::OK);
    let (status, json) = get(app, &format!("/api/shared/{share}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["detail"], "Shared playlist not found");
}

#[tokio::test]
async fn shared_playlist_unknown_token_404() {
    let t = test_app().await;
    let (status, json) = get(t.router(), "/api/shared/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["detail"], "Shared playlist not found");
}

// ── Album sharing ────────────────────────────────────────────────────────────

#[tokio::test]
async fn album_share_status_requires_auth() {
    let t = test_app().await;
    let (status, _) = get(t.router(), "/api/albums/share-status?album=Thing").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn album_share_roundtrip() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;
    let result = sqlx::query(
        "INSERT INTO tracks (file_path, title, artist, album, disc_number, track_number) \
         VALUES (?, ?, ?, ?, 1, 1)",
    )
    .bind("/m/f.mp3")
    .bind("Song F")
    .bind("The Amps")
    .bind("Big Record")
    .execute(&t.pool)
    .await
    .unwrap();
    let tid = result.last_insert_rowid();

    let (status, json) = post_auth_json(
        app,
        "/api/albums/share",
        &token,
        r#"{"album":"Big Record","artist":"The Amps"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let share = json["token"].as_str().expect("token").to_string();

    let (status, json) = get_auth(
        app,
        "/api/albums/share-status?album=Big+Record&artist=The+Amps",
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["shared"], true);
    assert_eq!(json["token"], share);

    let (status, json) = get(app, &format!("/api/album-shared/{share}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["album"], "Big Record");
    assert_eq!(json["artist"], "The Amps");
    assert_eq!(json["tracks"].as_array().unwrap().len(), 1);
    assert_eq!(json["tracks"][0]["id"], tid);

    // Unshare → gone.
    let (status, _) = del_auth_json(
        app,
        "/api/albums/share",
        &token,
        r#"{"album":"Big Record","artist":"The Amps"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, json) = get(app, &format!("/api/album-shared/{share}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["detail"], "Shared album not found");
}

#[tokio::test]
async fn album_share_requires_auth() {
    let t = test_app().await;
    let (status, _) = post_json(t.router(), "/api/albums/share", r#"{"album":"X"}"#).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ── Folders ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn folders_empty_without_token() {
    let t = test_app().await;
    let (status, json) = get(t.router(), "/api/playlist-folders").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json, serde_json::json!([]));
}

#[tokio::test]
async fn folder_roundtrip_with_playlist() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;
    let pid = create_playlist(app, &token, "Filed").await;

    let (status, json) = post_auth_json(
        app,
        "/api/playlist-folders",
        &token,
        r#"{"name":"Road Trip"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let fid = json["id"].as_i64().expect("folder id");
    assert_eq!(json["name"], "Road Trip");

    let (status, json) = get_auth(app, "/api/playlist-folders", &token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.as_array().unwrap().len(), 1);
    assert_eq!(json[0]["playlist_count"], 0);

    // Assign playlist to folder.
    let (status, _) = put_auth_json(
        app,
        &format!("/api/playlists/{pid}/folder"),
        &token,
        &format!(r#"{{"folder_id":{fid}}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, json) = get_auth(app, "/api/playlists", &token).await;
    assert_eq!(json[0]["folder_id"], fid);
    assert_eq!(status, StatusCode::OK);
    let (status, json) = get_auth(app, "/api/playlist-folders", &token).await;
    assert_eq!(json[0]["playlist_count"], 1);
    assert_eq!(status, StatusCode::OK);

    // Rename.
    let (status, _) = put_auth_json(
        app,
        &format!("/api/playlist-folders/{fid}"),
        &token,
        r#"{"name":"Renamed"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Delete folder → playlist's folder_id nulled.
    let (status, _) = del_auth(app, &format!("/api/playlist-folders/{fid}"), &token).await;
    assert_eq!(status, StatusCode::OK);
    let (status, json) = get_auth(app, "/api/playlists", &token).await;
    assert!(json[0]["folder_id"].is_null());
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn folder_ownership_404() {
    let t = test_app().await;
    let app = t.router();
    let alice = setup_admin(app).await;
    let bob = second_user(app).await;

    let (status, json) =
        post_auth_json(app, "/api/playlist-folders", &alice, r#"{"name":"A"}"#).await;
    assert_eq!(status, StatusCode::OK);
    let fid = json["id"].as_i64().unwrap();

    let (status, json) = put_auth_json(
        app,
        &format!("/api/playlist-folders/{fid}"),
        &bob,
        r#"{"name":"stolen"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["detail"], "Folder not found");

    let (status, json) = del_auth(app, &format!("/api/playlist-folders/{fid}"), &bob).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["detail"], "Folder not found");
}

// ── Player state ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn player_state_requires_auth() {
    let t = test_app().await;
    let (status, _) = get(t.router(), "/api/player/state").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = put_auth_json(t.router(), "/api/player/state", "bogus", "{}").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn player_state_defaults_then_roundtrip() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;

    // Defaults when no row (Python parity).
    let (status, json) = get_auth(app, "/api/player/state", &token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["queue"], serde_json::json!([]));
    assert_eq!(json["current_index"], -1);
    assert_eq!(json["current_time"], 0);
    assert_eq!(json["shuffle"], false);
    assert_eq!(json["repeat_mode"], "off");
    assert_eq!(json["playback_speed"], 1.0);
    assert_eq!(json["volume"], 0.7);
    assert_eq!(json["shuffle_order"], serde_json::json!([]));
    assert_eq!(json["shuffle_index"], 0);

    // Save partial body → defaults fill the rest.
    let (status, _) = put_auth_json(
        app,
        "/api/player/state",
        &token,
        r#"{"queue":[1,2,3],"current_index":1,"current_time":42.5,"repeat_mode":"all"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, json) = get_auth(app, "/api/player/state", &token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["queue"], serde_json::json!([1, 2, 3]));
    assert_eq!(json["current_index"], 1);
    assert_eq!(json["current_time"], 42.5);
    assert_eq!(json["repeat_mode"], "all");
    assert_eq!(json["volume"], 0.7);
    assert_eq!(json["shuffle"], false);

    // Upsert overwrites.
    let (status, _) = put_auth_json(
        app,
        "/api/player/state",
        &token,
        r#"{"queue":[9],"current_index":-1,"shuffle":true,"volume":0.25}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, json) = get_auth(app, "/api/player/state", &token).await;
    assert_eq!(json["queue"], serde_json::json!([9]));
    assert_eq!(json["shuffle"], true);
    assert_eq!(json["volume"], 0.25);
    assert_eq!(status, StatusCode::OK);

    // Delete → back to defaults.
    let (status, _) = del_auth(app, "/api/player/state", &token).await;
    assert_eq!(status, StatusCode::OK);
    let (status, json) = get_auth(app, "/api/player/state", &token).await;
    assert_eq!(json["queue"], serde_json::json!([]));
    assert_eq!(json["volume"], 0.7);
    assert_eq!(status, StatusCode::OK);
}

// ── Play count (anonymous) ───────────────────────────────────────────────────

#[tokio::test]
async fn play_count_increments_anonymously() {
    let t = test_app().await;
    let app = t.router();
    let tid = insert_track(&t.pool, "/m/g.mp3", "Song G").await;

    let (status, json) = post_json(app, &format!("/api/play/{tid}"), "{}").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["ok"], true);

    let (status, _json) = post_json(app, &format!("/api/play/{tid}"), "{}").await;
    assert_eq!(status, StatusCode::OK);

    let row: (Option<i64>,) = sqlx::query_as("SELECT play_count FROM tracks WHERE id = ?")
        .bind(tid)
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(row.0, Some(2));

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM play_history WHERE track_id = ?")
        .bind(tid)
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(count.0, 2);
}

// ── Export / import ──────────────────────────────────────────────────────────

#[tokio::test]
async fn export_requires_auth_but_accepts_query_token() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;
    let pid = create_playlist(app, &token, "Export Me").await;

    // No token at all → 401.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/playlists/{pid}/export"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // `?token=` works (UI uses <a href> without headers).
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/playlists/{pid}/export?token={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let ct = response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert_eq!(ct, "audio/x-mpegurl");
    let cd = response
        .headers()
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert_eq!(cd, r#"attachment; filename="Export Me.m3u""#);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&body);
    assert!(text.starts_with("#EXTM3U"));
    assert!(text.contains("#PLAYLIST: Export Me"));
}

#[tokio::test]
async fn export_json_and_import_roundtrip() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;
    let tid = insert_track(&t.pool, "/m/Song H.mp3", "Song H").await;
    let pid = create_playlist(app, &token, "Round Trip").await;
    post_auth_json(
        app,
        &format!("/api/playlists/{pid}/tracks"),
        &token,
        &format!(r#"{{"track_id":{tid}}}"#),
    )
    .await;

    // Export JSON via header.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/playlists/{pid}/export?format=json"))
                .header("token", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let cd = response
        .headers()
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert_eq!(cd, r#"attachment; filename="Round Trip.json""#);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["playlist_name"], "Round Trip");
    assert_eq!(payload["format_version"], 1);
    assert_eq!(payload["tracks"].as_array().unwrap().len(), 1);
    assert_eq!(payload["tracks"][0]["file_path"], "/m/Song H.mp3");

    // Import it back as a new playlist via multipart.
    let boundary = "----moidifybnd";
    let mut multipart = Vec::new();
    multipart.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; \
             filename=\"roundtrip.json\"\r\nContent-Type: application/json\r\n\r\n"
        )
        .as_bytes(),
    );
    multipart.extend_from_slice(&body);
    multipart.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/playlists/import")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .header("token", &token)
                .body(Body::from(multipart))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["name"], "Round Trip");
    assert_eq!(json["matched"], 1);
    assert_eq!(json["total"], 1);
    let new_pid = json["playlist_id"].as_i64().unwrap();
    assert_ne!(new_pid, pid);

    let (status, json) = get(app, &format!("/api/playlists/{new_pid}/tracks")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.as_array().unwrap().len(), 1);
    assert_eq!(json[0]["id"], tid);
}

#[tokio::test]
async fn import_requires_auth() {
    let t = test_app().await;
    let boundary = "----b";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; \
         filename=\"x.m3u\"\r\n\r\n#EXTM3U\r\n--{boundary}--\r\n"
    );
    let response = t
        .router()
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/playlists/import")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn import_m3u_matches_existing_track() {
    let t = test_app().await;
    let app = t.router();
    let token = setup_admin(app).await;
    let tid = insert_track(&t.pool, "/m/exact match.mp3", "Exact").await;

    let m3u = "#EXTM3U\n#PLAYLIST: From Disk\n#EXTINF:120,Phase Four - Exact\n/m/exact match.mp3\n";
    let boundary = "----bnd2";
    let mut multipart = Vec::new();
    multipart.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; \
             filename=\"from_disk.m3u\"\r\n\r\n"
        )
        .as_bytes(),
    );
    multipart.extend_from_slice(m3u.as_bytes());
    multipart.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/playlists/import")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .header("token", &token)
                .body(Body::from(multipart))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["matched"], 1);
    assert_eq!(json["name"], "from_disk");
    let pid = json["playlist_id"].as_i64().unwrap();
    let (status, json) = get(app, &format!("/api/playlists/{pid}/tracks")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.as_array().unwrap().len(), 1);
    assert_eq!(json[0]["id"], tid);
}
