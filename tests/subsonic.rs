mod common;

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use common::{test_app, TestApp};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

const PW: &str = "TestPass123!";
/// Fixed client-side salt for `t=md5(password + s)` (the Python suite uses
/// the same nonce — any `s` works, it is not the per-user DB salt).
const SALT: &str = "c19b2d";

/// Golden ids locked by the `ids` unit tests.
const AL_FIRST: &str = "al_7f97e0a6bf2d";
const AR_ALPHA: &str = "ar_899894a0846e";

fn md5hex(s: &str) -> String {
    moidify::routes::subsonic::ids::md5_hex(s)
}

// ── HTTP helpers ────────────────────────────────────────────────────────────

async fn raw(app: &axum::Router, method: &str, uri: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
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

async fn get_raw(app: &axum::Router, uri: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
    raw(app, "GET", uri).await
}

async fn post_raw(app: &axum::Router, uri: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
    raw(app, "POST", uri).await
}

fn s(body: &[u8]) -> String {
    String::from_utf8_lossy(body).into_owned()
}

fn json_of(body: &[u8]) -> Value {
    serde_json::from_slice(body)
        .unwrap_or_else(|e| panic!("invalid json ({e}): {}", String::from_utf8_lossy(body)))
}

/// Children of a JSON element: repeated tags are arrays, a single child is a
/// bare object, an omitted (empty) list is `null`. Panics on `null` so a
/// missing key reads as the real error, not an empty list.
fn elems(v: &Value) -> Vec<&Value> {
    match v {
        Value::Array(a) => a.iter().collect(),
        Value::Null => panic!("expected child elements, got null"),
        other => vec![other],
    }
}

// ── Auth / fixture helpers ──────────────────────────────────────────────────

async fn setup(t: &TestApp) -> String {
    common::setup_admin(t.router()).await
}

/// `u`/`t`/`s` token auth for the admin user (matches `test_subsonic_token_auth`).
fn token_auth() -> String {
    format!(
        "u=admin&t={}&s={SALT}&v=1.16.1&c=rust",
        md5hex(&format!("{PW}{SALT}"))
    )
}

/// Same shape with a wrong password (matches `..._wrong_password`).
fn bad_token_auth() -> String {
    format!(
        "u=admin&t={}&s={SALT}&v=1.16.1&c=rust",
        md5hex(&format!("WrongPass999!{SALT}"))
    )
}

const PLAIN: &str = "u=admin&p=TestPass123!&v=1.16.1&c=rust";

#[allow(clippy::too_many_arguments)]
async fn insert_track(
    t: &TestApp,
    path: &str,
    title: &str,
    artist: &str,
    album: &str,
    album_artist: Option<&str>,
    duration: f64,
    track: i64,
    disc: i64,
    year: Option<i64>,
    genre: Option<&str>,
) -> i64 {
    let full = t.music_dir.join(path);
    std::fs::write(&full, format!("ID3{path}")).expect("write fixture");
    let res = sqlx::query(
        "INSERT INTO tracks (file_path, title, artist, album_artist, album, \
         duration, track_number, disc_number, year, genre) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(full.to_str().unwrap())
    .bind(title)
    .bind(artist)
    .bind(album_artist)
    .bind(album)
    .bind(duration)
    .bind(track)
    .bind(disc)
    .bind(year)
    .bind(genre)
    .execute(&t.pool)
    .await
    .expect("insert track");
    res.last_insert_rowid()
}

struct Catalog {
    t1: i64,
    t2: i64,
    t3: i64,
    t4: i64,
}

/// Four tracks, three albums (`First Album`, `Second Album` via the
/// `album_artist` COALESCE, `Esc Album`), two distinct artists
/// (`Alpha Band`, `Zed Artist`), one track with XML specials.
async fn seed(t: &TestApp) -> Catalog {
    Catalog {
        t1: insert_track(
            t,
            "one.mp3",
            "Song One",
            "Alpha Band",
            "First Album",
            None,
            120.467,
            1,
            1,
            Some(2020),
            Some("Rock"),
        )
        .await,
        t2: insert_track(
            t,
            "two.mp3",
            "Song Two",
            "Alpha Band",
            "First Album",
            None,
            60.0,
            2,
            1,
            Some(2021),
            Some("Rock"),
        )
        .await,
        t3: insert_track(
            t,
            "three.mp3",
            "Song Three",
            "beta",
            "Second Album",
            Some("Zed Artist"),
            90.5,
            1,
            1,
            None,
            None,
        )
        .await,
        t4: insert_track(
            t,
            "spec.mp3",
            "Q & <A>",
            "Alpha Band",
            "Esc Album",
            None,
            10.0,
            1,
            1,
            None,
            Some(r#"R&B <"Rock">"#),
        )
        .await,
    }
}

// ── Format / protocol ───────────────────────────────────────────────────────

#[tokio::test]
async fn ping_xml_is_byte_exact() {
    let t = test_app().await;
    let (status, headers, body) = get_raw(t.router(), "/rest/ping").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get("content-type").unwrap(), "application/xml");
    assert_eq!(
        s(&body),
        "<?xml version='1.0' encoding='UTF-8'?>\n\
         <subsonic-response status=\"ok\" version=\"1.16.1\" \
         xmlns=\"http://subsonic.org/restapi\" />"
    );
}

#[tokio::test]
async fn ping_json_and_jsonp_formats() {
    let t = test_app().await;

    let (status, headers, body) = get_raw(t.router(), "/rest/ping?f=json").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get("content-type").unwrap(), "application/json");
    let json = json_of(&body);
    assert_eq!(
        json,
        serde_json::json!({"subsonic-response": {"status": "ok", "version": "1.16.1"}})
    );
    assert!(json["subsonic-response"].get("xmlns").is_none(), "{json:?}");

    let (status, headers, body) = get_raw(t.router(), "/rest/ping?f=jsonp&callback=cb1").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers.get("content-type").unwrap(),
        "application/javascript"
    );
    let text = s(&body);
    assert!(text.starts_with("cb1({"), "{text}");
    assert!(text.ends_with(')'), "{text}");
    let inner = text
        .strip_prefix("cb1(")
        .and_then(|rest| rest.strip_suffix(')'))
        .expect("cb wrapper");
    let json: Value = serde_json::from_str(inner).expect("jsonp is JSON");
    assert_eq!(json["subsonic-response"]["status"], "ok");

    // f=jsonp without callback falls back to XML (Python behavior).
    let (_, headers, body) = get_raw(t.router(), "/rest/ping?f=jsonp").await;
    assert_eq!(headers.get("content-type").unwrap(), "application/xml");
    assert!(s(&body).starts_with("<?xml"));
}

#[tokio::test]
async fn view_suffix_is_stripped_for_get_and_post() {
    let t = test_app().await;
    setup(&t).await;
    let a = token_auth();

    let (status, _, body) = get_raw(t.router(), "/rest/ping.view?f=json").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json_of(&body)["subsonic-response"]["status"], "ok");

    let uri = format!("/rest/getMusicFolders.view?{a}&f=json");
    let (status, _, body) = get_raw(t.router(), &uri).await;
    assert_eq!(status, StatusCode::OK);
    let json = json_of(&body);
    assert_eq!(json["subsonic-response"]["status"], "ok");
    assert_eq!(
        json["subsonic-response"]["musicFolders"]["musicFolder"]["id"],
        "1"
    );

    let (status, _, body) = post_raw(t.router(), &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json_of(&body)["subsonic-response"]["status"], "ok");
}

#[tokio::test]
async fn post_method_works_like_get() {
    let t = test_app().await;
    setup(&t).await;
    seed(&t).await;
    let a = token_auth();

    let (status, _, body) =
        post_raw(t.router(), &format!("/rest/search3?{a}&query=Song&f=json")).await;
    assert_eq!(status, StatusCode::OK);
    let json = json_of(&body);
    assert_eq!(json["subsonic-response"]["status"], "ok");
    assert_eq!(json["subsonic-response"]["searchResult3"]["totalHits"], "3");
}

// ── Auth ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn token_auth_success() {
    let t = test_app().await;
    setup(&t).await;

    // ping never authenticates — mirror of test_subsonic_token_auth.
    let (status, _, body) =
        get_raw(t.router(), &format!("/rest/ping?{}&f=json", token_auth())).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json_of(&body)["subsonic-response"]["status"], "ok");

    // An endpoint that *does* require auth passes with the token.
    let (status, _, body) = get_raw(
        t.router(),
        &format!("/rest/getMusicFolders?{}&f=json", token_auth()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let json = json_of(&body);
    assert_eq!(json["subsonic-response"]["status"], "ok");
    assert!(json["subsonic-response"].get("musicFolders").is_some());
}

#[tokio::test]
async fn token_auth_wrong_password_fails_with_string_code_40() {
    let t = test_app().await;
    setup(&t).await;

    let (status, _, body) = get_raw(
        t.router(),
        &format!("/rest/getMusicFolders?{}&f=json", bad_token_auth()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let json = json_of(&body);
    assert_eq!(json["subsonic-response"]["status"], "failed");
    // Error codes travel as JSON strings, like Python str(code).
    assert_eq!(json["subsonic-response"]["error"]["code"], "40");
    assert_eq!(
        json["subsonic-response"]["error"]["message"],
        "Wrong username or password"
    );
}

#[tokio::test]
async fn plaintext_password_auth_works() {
    let t = test_app().await;
    setup(&t).await;

    let (status, _, body) =
        get_raw(t.router(), &format!("/rest/getMusicFolders?{PLAIN}&f=json")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json_of(&body)["subsonic-response"]["status"], "ok");
}

#[tokio::test]
async fn hex_encoded_password_auth_works() {
    let t = test_app().await;
    setup(&t).await;

    let hex_pw = hex::encode(PW);
    let (status, _, body) = get_raw(
        t.router(),
        &format!("/rest/getMusicFolders?u=admin&p=enc:{hex_pw}&v=1.16.1&c=rust&f=json"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json_of(&body)["subsonic-response"]["status"], "ok");
}

#[tokio::test]
async fn missing_or_unknown_user_fails_40() {
    let t = test_app().await;
    setup(&t).await;

    // Missing u entirely.
    let (status, _, body) = get_raw(t.router(), "/rest/getMusicFolders?f=json").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json_of(&body)["subsonic-response"]["error"]["code"], "40");
    assert_eq!(
        json_of(&body)["subsonic-response"]["error"]["message"],
        "Wrong username or password"
    );

    // Unknown user.
    let (status, _, body) = get_raw(
        t.router(),
        &format!("/rest/getMusicFolders?u=ghost&p={PW}&v=1.16.1&c=rust&f=json"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json_of(&body)["subsonic-response"]["error"]["code"], "40");
}

#[tokio::test]
async fn rate_limit_counts_failed_attempts_only() {
    let t = test_app().await;
    setup(&t).await;

    // 35 *successful* calls must not count toward the 30/15min budget
    // (Python records every request — divergence #2 in mod.rs).
    for i in 0..35 {
        let (_, _, body) = get_raw(
            t.router(),
            &format!("/rest/getMusicFolders?{}&f=json", token_auth()),
        )
        .await;
        let json = json_of(&body);
        assert_eq!(json["subsonic-response"]["status"], "ok", "request {i}");
    }

    // Failures do count: 5 failures, then a valid token still works.
    for _ in 0..5 {
        let (_, _, body) = get_raw(
            t.router(),
            &format!("/rest/getMusicFolders?{}&f=json", bad_token_auth()),
        )
        .await;
        assert_eq!(json_of(&body)["subsonic-response"]["error"]["code"], "40");
    }
    let (_, _, body) = get_raw(
        t.router(),
        &format!("/rest/getMusicFolders?{}&f=json", token_auth()),
    )
    .await;
    assert_eq!(json_of(&body)["subsonic-response"]["status"], "ok");

    // 25 more failures reach the 30 cap → even a valid token is rejected.
    for _ in 0..25 {
        let (_, _, body) = get_raw(
            t.router(),
            &format!("/rest/getMusicFolders?{}&f=json", bad_token_auth()),
        )
        .await;
        assert_eq!(json_of(&body)["subsonic-response"]["error"]["code"], "40");
    }
    let (_, _, body) = get_raw(
        t.router(),
        &format!("/rest/getMusicFolders?{}&f=json", token_auth()),
    )
    .await;
    let json = json_of(&body);
    assert_eq!(json["subsonic-response"]["status"], "failed");
    assert_eq!(json["subsonic-response"]["error"]["code"], "40");
}

#[tokio::test]
async fn missing_id_returns_fastapi_422_before_auth() {
    let t = test_app().await;

    // No credentials at all — FastAPI validates `id` before auth runs.
    let (status, _, body) = get_raw(t.router(), "/rest/getSong").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let json = json_of(&body);
    let err = &json["detail"][0];
    assert_eq!(err["type"], "missing");
    assert_eq!(err["loc"], serde_json::json!(["query", "id"]));
    assert_eq!(err["msg"], "Field required");
    assert!(err["input"].is_null());

    // Same for POST.
    let (status, _, _) = post_raw(t.router(), "/rest/stream").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn every_endpoint_rejects_missing_credentials() {
    let t = test_app().await;
    setup(&t).await;

    for ep in [
        "getArtist",
        "getAlbum",
        "getSong",
        "getMusicDirectory",
        "stream",
        "download",
        "getCoverArt",
        "getPlaylist",
        "scrobble",
        "star",
        "unstar",
    ] {
        let (_, _, body) = get_raw(t.router(), &format!("/rest/{ep}?id=1&f=json")).await;
        let json = json_of(&body);
        assert_eq!(
            json["subsonic-response"]["error"]["code"], "40",
            "{ep}: {json}"
        );
    }

    for ep in [
        "getLicense",
        "getMusicFolders",
        "getIndexes",
        "getArtists",
        "search3",
        "getAlbumList2",
        "getPlaylists",
        "getStarred",
        "getRandomSongs",
        "getNowPlaying",
    ] {
        let (_, _, body) = get_raw(t.router(), &format!("/rest/{ep}?f=json")).await;
        let json = json_of(&body);
        assert_eq!(
            json["subsonic-response"]["error"]["code"], "40",
            "{ep}: {json}"
        );
    }

    // ping and getAvatar are the two unauthenticated endpoints.
    let (_, _, body) = get_raw(t.router(), "/rest/ping?f=json").await;
    assert_eq!(json_of(&body)["subsonic-response"]["status"], "ok");
    let (status, _, _) = get_raw(t.router(), "/rest/getAvatar").await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

// ── Browse ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_music_folders_structure() {
    let t = test_app().await;
    setup(&t).await;
    let a = token_auth();

    let (_, _, body) = get_raw(t.router(), &format!("/rest/getMusicFolders?{a}")).await;
    let xml = s(&body);
    assert!(
        xml.contains("<musicFolders><musicFolder id=\"1\" name=\"Music\" /></musicFolders>"),
        "{xml}"
    );

    let (_, _, body) = get_raw(t.router(), &format!("/rest/getMusicFolders?{a}&f=json")).await;
    let json = json_of(&body);
    let folder = &json["subsonic-response"]["musicFolders"]["musicFolder"];
    assert_eq!(folder["id"], "1");
    assert_eq!(folder["name"], "Music");
}

#[tokio::test]
async fn indexes_and_artists_bucket_by_letter() {
    let t = test_app().await;
    setup(&t).await;
    seed(&t).await;
    let a = token_auth();

    let (_, _, body) = get_raw(t.router(), &format!("/rest/getIndexes?{a}")).await;
    let xml = s(&body);
    assert!(
        xml.contains("<indexes lastModified=\"0\" ignoredArticles=\"\">"),
        "{xml}"
    );
    assert!(
        xml.contains(&format!(
            "<index name=\"A\"><artist name=\"Alpha Band\" id=\"{AR_ALPHA}\" /></index>"
        )),
        "{xml}"
    );
    assert!(
        xml.contains("<index name=\"Z\"><artist name=\"Zed Artist\" id=\"ar_"),
        "{xml}"
    );
    assert!(
        xml.find("name=\"A\"").unwrap() < xml.find("name=\"Z\"").unwrap(),
        "BTreeMap letter order: {xml}"
    );

    let (_, _, body) = get_raw(t.router(), &format!("/rest/getArtists?{a}&f=json")).await;
    let json = json_of(&body);
    let idx = json["subsonic-response"]["artists"]["index"]
        .as_array()
        .expect("index array");
    assert_eq!(idx.len(), 2);
    assert_eq!(idx[0]["name"], "A");
    assert_eq!(idx[0]["artist"]["name"], "Alpha Band");
    assert_eq!(idx[0]["artist"]["id"], AR_ALPHA);
    // Raw GROUP BY quirk: Alpha Band's group covers both of its albums.
    assert_eq!(idx[0]["artist"]["albumCount"], "2");
    assert_eq!(idx[1]["name"], "Z");
    // The 'beta' raw group renders its COALESCE'd name.
    assert_eq!(idx[1]["artist"]["name"], "Zed Artist");
    assert_eq!(idx[1]["artist"]["albumCount"], "1");
}

#[tokio::test]
async fn get_artist_lists_albums_or_errors_70() {
    let t = test_app().await;
    setup(&t).await;
    seed(&t).await;
    let a = token_auth();

    let (_, _, body) = get_raw(
        t.router(),
        &format!("/rest/getArtist?id={AR_ALPHA}&{a}&f=json"),
    )
    .await;
    let json = json_of(&body);
    let artist = &json["subsonic-response"]["artist"];
    assert_eq!(artist["id"], AR_ALPHA);
    assert_eq!(artist["name"], "Alpha Band");
    assert_eq!(artist["albumCount"], "2");
    let albums = artist["album"].as_array().expect("album array");
    // ORDER BY album → Esc Album before First Album.
    assert_eq!(albums[0]["title"], "Esc Album");
    assert_eq!(albums[0]["parent"], AR_ALPHA);
    assert_eq!(albums[0]["isDir"], "true");
    assert_eq!(albums[0]["songCount"], "1");
    assert_eq!(albums[1]["title"], "First Album");
    assert_eq!(albums[1]["id"], AL_FIRST);

    let (_, _, body) = get_raw(
        t.router(),
        &format!("/rest/getArtist?id=ar_000000000000&{a}&f=json"),
    )
    .await;
    let json = json_of(&body);
    assert_eq!(json["subsonic-response"]["error"]["code"], "70");
    assert_eq!(
        json["subsonic-response"]["error"]["message"],
        "Artist not found"
    );
}

#[tokio::test]
async fn get_album_by_id_cover_id_and_404() {
    let t = test_app().await;
    setup(&t).await;
    let c = seed(&t).await;
    let a = token_auth();

    let (_, _, body) = get_raw(
        t.router(),
        &format!("/rest/getAlbum?id={AL_FIRST}&{a}&f=json"),
    )
    .await;
    let json = json_of(&body);
    let album = &json["subsonic-response"]["album"];
    assert_eq!(album["id"], AL_FIRST);
    assert_eq!(album["name"], "First Album");
    assert_eq!(album["artist"], "Alpha Band");
    assert_eq!(album["songCount"], "2");
    // Python str(sum([120.467, 60.0])) — shortest float repr, quirks included.
    assert_eq!(album["duration"], "180.46699999999998");
    assert_eq!(album["year"], "2020");
    assert_eq!(album["genre"], "Rock");
    assert_eq!(album["coverArt"], c.t1.to_string());
    assert_eq!(album["isDir"], "false");
    assert_eq!(album["type"], "album");
    let songs = album["song"].as_array().expect("song array");
    assert_eq!(songs.len(), 2);
    assert_eq!(songs[0]["id"], c.t1.to_string());
    assert_eq!(songs[0]["title"], "Song One");
    assert_eq!(songs[0]["duration"], "120");
    assert_eq!(songs[0]["track"], "1");
    assert_eq!(songs[0]["discNumber"], "1");
    assert_eq!(songs[0]["type"], "music");
    assert_eq!(songs[0]["suffix"], "mp3");
    assert_eq!(songs[0]["contentType"], "audio/mpeg");
    assert_eq!(songs[0]["parent"], songs[0]["id"]);
    assert_eq!(songs[1]["id"], c.t2.to_string());
    assert_eq!(songs[1]["track"], "2");

    // Numeric cover-track-id fallback (Python `str(cover_track_id) == id`).
    let (_, _, body) = get_raw(
        t.router(),
        &format!("/rest/getAlbum?id={}&{a}&f=json", c.t1),
    )
    .await;
    assert_eq!(
        json_of(&body)["subsonic-response"]["album"]["name"],
        "First Album"
    );

    let (_, _, body) = get_raw(
        t.router(),
        &format!("/rest/getAlbum?id=al_deadbeefdead&{a}&f=json"),
    )
    .await;
    let json = json_of(&body);
    assert_eq!(json["subsonic-response"]["error"]["code"], "70");
    assert_eq!(
        json["subsonic-response"]["error"]["message"],
        "Album not found"
    );
}

#[tokio::test]
async fn get_song_and_xml_escaping() {
    let t = test_app().await;
    setup(&t).await;
    let c = seed(&t).await;
    let a = token_auth();

    let (_, _, body) = get_raw(t.router(), &format!("/rest/getSong?id={}&{a}&f=json", c.t1)).await;
    let json = json_of(&body);
    let song = &json["subsonic-response"]["song"];
    assert_eq!(song["id"], c.t1.to_string());
    assert_eq!(song["title"], "Song One");
    assert_eq!(song["artist"], "Alpha Band");
    assert_eq!(song["album"], "First Album");
    assert_eq!(song["year"], "2020");
    assert_eq!(song["genre"], "Rock");
    assert_eq!(song["size"], format!("{}", "ID3one.mp3".len()));
    assert_eq!(song["isDir"], "false");

    // XML special characters are attribute-escaped like ElementTree.
    let (_, _, body) = get_raw(t.router(), &format!("/rest/getSong?id={}&{a}", c.t4)).await;
    let xml = s(&body);
    assert!(xml.contains("title=\"Q &amp; &lt;A&gt;\""), "{xml}");
    assert!(
        xml.contains("genre=\"R&amp;B &lt;&quot;Rock&quot;&gt;\""),
        "{xml}"
    );

    // JSON keeps the raw characters.
    let (_, _, body) = get_raw(t.router(), &format!("/rest/getSong?id={}&{a}&f=json", c.t4)).await;
    let json = json_of(&body);
    assert_eq!(json["subsonic-response"]["song"]["title"], "Q & <A>");
    assert_eq!(
        json["subsonic-response"]["song"]["genre"],
        r#"R&B <"Rock">"#
    );
    assert_eq!(json["subsonic-response"]["song"]["album"], "Esc Album");

    // Unknown / non-numeric ids → error 70.
    for id in ["99999", "abc"] {
        let (_, _, body) = get_raw(t.router(), &format!("/rest/getSong?id={id}&{a}&f=json")).await;
        let json = json_of(&body);
        assert_eq!(json["subsonic-response"]["error"]["code"], "70", "id={id}");
        assert_eq!(
            json["subsonic-response"]["error"]["message"],
            "Song not found"
        );
    }
}

#[tokio::test]
async fn get_music_directory_album_artist_and_404() {
    let t = test_app().await;
    setup(&t).await;
    seed(&t).await;
    let a = token_auth();

    // Album directory → <child> entries (Subsonic spec tag).
    let (_, _, body) = get_raw(
        t.router(),
        &format!("/rest/getMusicDirectory?id={AL_FIRST}&{a}&f=json"),
    )
    .await;
    let json = json_of(&body);
    let dir = &json["subsonic-response"]["directory"];
    assert_eq!(dir["id"], AL_FIRST);
    assert_eq!(dir["name"], "First Album");
    let children = dir["child"].as_array().expect("child array");
    assert_eq!(children.len(), 2);
    assert_eq!(children[0]["title"], "Song One");
    assert_eq!(children[0]["isDir"], "false");

    // Artist directory → album children.
    let (_, _, body) = get_raw(
        t.router(),
        &format!("/rest/getMusicDirectory?id={AR_ALPHA}&{a}&f=json"),
    )
    .await;
    let json = json_of(&body);
    let dir = &json["subsonic-response"]["directory"];
    assert_eq!(dir["name"], "Alpha Band");
    let children = dir["child"].as_array().expect("child array");
    assert_eq!(children.len(), 2);
    assert_eq!(children[0]["title"], "Esc Album");
    assert_eq!(children[0]["isDir"], "true");
    assert_eq!(children[0]["type"], "album");

    let (_, _, body) = get_raw(
        t.router(),
        &format!("/rest/getMusicDirectory?id=al_000000000000&{a}&f=json"),
    )
    .await;
    let json = json_of(&body);
    assert_eq!(json["subsonic-response"]["error"]["code"], "70");
    assert_eq!(
        json["subsonic-response"]["error"]["message"],
        "Directory not found"
    );
}

#[tokio::test]
async fn search3_returns_arrays_and_total_hits() {
    let t = test_app().await;
    setup(&t).await;
    let c = seed(&t).await;
    let a = token_auth();

    let (_, _, body) = get_raw(t.router(), &format!("/rest/search3?{a}&query=Song&f=json")).await;
    let json = json_of(&body);
    let result = &json["subsonic-response"]["searchResult3"];
    assert_eq!(result["totalHits"], "3");
    // Empty result lists are omitted (Subsonic-conformant).
    assert!(result.get("artist").is_none(), "{result:?}");
    assert!(result.get("album").is_none(), "{result:?}");
    let songs = result["song"].as_array().expect("song array");
    assert_eq!(songs.len(), 3);
    assert_eq!(songs[0]["id"], c.t1.to_string());
    assert_eq!(songs[1]["id"], c.t2.to_string());
    assert_eq!(songs[2]["id"], c.t3.to_string());

    // Artist matches come back as an array of artist children.
    let (_, _, body) = get_raw(t.router(), &format!("/rest/search3?{a}&query=Alpha&f=json")).await;
    let json = json_of(&body);
    let result = &json["subsonic-response"]["searchResult3"];
    let artists = elems(&result["artist"]);
    assert_eq!(artists.len(), 1);
    assert_eq!(artists[0]["name"], "Alpha Band");
    assert_eq!(artists[0]["id"], AR_ALPHA);
    // totalHits counts songs: t1 + t2 + t4 all carry artist "Alpha Band".
    assert_eq!(result["totalHits"], "3");

    // Album match.
    let (_, _, body) = get_raw(t.router(), &format!("/rest/search3?{a}&query=First&f=json")).await;
    let json = json_of(&body);
    let albums = elems(&json["subsonic-response"]["searchResult3"]["album"]);
    assert_eq!(albums.len(), 1);
    assert_eq!(albums[0]["title"], "First Album");
    assert_eq!(albums[0]["id"], AL_FIRST);
}

#[tokio::test]
async fn album_list2_ordering_and_garbage_size_defaults() {
    let t = test_app().await;
    setup(&t).await;
    seed(&t).await;
    let a = token_auth();

    let (_, _, body) = get_raw(
        t.router(),
        &format!("/rest/getAlbumList2?{a}&type=alphabeticalByName&f=json"),
    )
    .await;
    let json = json_of(&body);
    let albums = json["subsonic-response"]["albumList2"]["album"]
        .as_array()
        .expect("album array");
    assert_eq!(albums.len(), 3);
    assert_eq!(albums[0]["title"], "Esc Album");
    assert_eq!(albums[1]["title"], "First Album");
    assert_eq!(albums[2]["title"], "Second Album");

    let (_, _, body) = get_raw(
        t.router(),
        &format!("/rest/getAlbumList2?{a}&type=alphabeticalByName&size=1&f=json"),
    )
    .await;
    let json = json_of(&body);
    let albums = elems(&json["subsonic-response"]["albumList2"]["album"]);
    assert_eq!(albums.len(), 1);
    assert_eq!(albums[0]["title"], "Esc Album");

    // Divergence: Python 500s on `int("abc")`; we fall back to the default.
    let (_, _, body) = get_raw(
        t.router(),
        &format!("/rest/getAlbumList2?{a}&type=alphabeticalByName&size=abc&f=json"),
    )
    .await;
    let json = json_of(&body);
    let albums = elems(&json["subsonic-response"]["albumList2"]["album"]);
    assert_eq!(albums.len(), 3);
}

// ── Playlists (plan §3.6 ownership fix) ─────────────────────────────────────

async fn insert_playlist(t: &TestApp, name: &str, user_id: i64) -> i64 {
    let res = sqlx::query("INSERT INTO playlists (name, user_id) VALUES (?, ?)")
        .bind(name)
        .bind(user_id)
        .execute(&t.pool)
        .await
        .expect("insert playlist");
    res.last_insert_rowid()
}

async fn insert_playlist_track(t: &TestApp, playlist: i64, track: i64, position: i64) {
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)")
        .bind(playlist)
        .bind(track)
        .bind(position)
        .execute(&t.pool)
        .await
        .expect("insert playlist track");
}

#[tokio::test]
async fn playlists_are_filtered_by_owner_and_detail_is_existence_only() {
    let t = test_app().await;
    let admin_token = setup(&t).await;
    let c = seed(&t).await;

    // Second user with their own playlist (admin web token from setup).
    let response = t
        .router()
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/users")
                .header("token", &admin_token)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"username":"user2","password":"OtherPass456!","is_admin":false}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let user2: i64 = sqlx::query_scalar("SELECT id FROM users WHERE username = 'user2'")
        .fetch_one(&t.pool)
        .await
        .expect("user2");

    let mine = insert_playlist(&t, "Road Trip", 1).await;
    insert_playlist_track(&t, mine, c.t2, 5).await;
    insert_playlist_track(&t, mine, c.t1, 1).await;
    let theirs = insert_playlist(&t, "Other Mix", user2).await;

    // Ownership filter: admin sees only their own playlist (fix vs Python,
    // which listed every user's playlists).
    let a = token_auth();
    let (_, _, body) = get_raw(t.router(), &format!("/rest/getPlaylists?{a}&f=json")).await;
    let json = json_of(&body);
    let lists = elems(&json["subsonic-response"]["playlists"]["playlist"]);
    assert_eq!(lists.len(), 1, "{json}");
    assert_eq!(lists[0]["id"], mine.to_string());
    assert_eq!(lists[0]["name"], "Road Trip");
    assert_eq!(lists[0]["owner"], "admin");
    assert_eq!(lists[0]["songCount"], "2");
    // Python hardcodes duration "0" on the *list* endpoint.
    assert_eq!(lists[0]["duration"], "0");

    // Detail endpoint is existence-only — fetching user2's playlist works.
    let (_, _, body) = get_raw(
        t.router(),
        &format!("/rest/getPlaylist?id={theirs}&{a}&f=json"),
    )
    .await;
    let json = json_of(&body);
    let playlist = &json["subsonic-response"]["playlist"];
    assert_eq!(playlist["name"], "Other Mix");
    assert_eq!(playlist["songCount"], "0");
    assert!(playlist.get("entry").is_none(), "{playlist:?}");

    // Detail: ordered by position, real duration sum, <entry> children.
    let (_, _, body) = get_raw(
        t.router(),
        &format!("/rest/getPlaylist?id={mine}&{a}&f=json"),
    )
    .await;
    let json = json_of(&body);
    let playlist = &json["subsonic-response"]["playlist"];
    assert_eq!(playlist["name"], "Road Trip");
    assert_eq!(playlist["owner"], "admin");
    assert_eq!(playlist["public"], "false");
    assert_eq!(playlist["songCount"], "2");
    assert_eq!(playlist["duration"], "180.46699999999998");
    let entries = playlist["entry"].as_array().expect("entry array");
    assert_eq!(entries[0]["id"], c.t1.to_string(), "position 1 before 5");
    assert_eq!(entries[1]["id"], c.t2.to_string());

    // XML uses <entry .../> children.
    let (_, _, body) = get_raw(t.router(), &format!("/rest/getPlaylist?id={mine}&{a}")).await;
    let xml = s(&body);
    assert!(xml.contains("<entry id="), "{xml}");

    for id in ["99999", "abc"] {
        let (_, _, body) =
            get_raw(t.router(), &format!("/rest/getPlaylist?id={id}&{a}&f=json")).await;
        let json = json_of(&body);
        assert_eq!(json["subsonic-response"]["error"]["code"], "70");
        assert_eq!(
            json["subsonic-response"]["error"]["message"],
            "Playlist not found"
        );
    }
}

// ── Session / activity ──────────────────────────────────────────────────────

#[tokio::test]
async fn get_license_shape() {
    let t = test_app().await;
    setup(&t).await;
    let a = token_auth();

    let (_, _, body) = get_raw(t.router(), &format!("/rest/getLicense?{a}")).await;
    let xml = s(&body);
    assert!(
        xml.contains(
            "<license valid=\"true\" email=\"\" licenseExpires=\"\" \
             trialExpires=\"\" isTrial=\"false\" />"
        ),
        "{xml}"
    );
}

#[tokio::test]
async fn scrobble_bumps_counts_and_tracks_playing() {
    let t = test_app().await;
    setup(&t).await;
    let c = seed(&t).await;
    let a = token_auth();

    let (_, _, body) = get_raw(
        t.router(),
        &format!("/rest/scrobble?id={}&{a}&f=json", c.t1),
    )
    .await;
    assert_eq!(json_of(&body)["subsonic-response"]["status"], "ok");
    let (play_count,): (i64,) = sqlx::query_as("SELECT play_count FROM tracks WHERE id = ?")
        .bind(c.t1)
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(play_count, 1);
    let (history,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM play_history")
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(history, 1);

    // trackId alias (Python `request.query_params.get("id") or ...("trackId")`).
    let (_, _, body) = get_raw(
        t.router(),
        &format!("/rest/scrobble?trackId={}&{a}&f=json", c.t2),
    )
    .await;
    assert_eq!(json_of(&body)["subsonic-response"]["status"], "ok");
    let (play_count,): (i64,) = sqlx::query_as("SELECT play_count FROM tracks WHERE id = ?")
        .bind(c.t2)
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(play_count, 1);

    // Non-numeric id → error 10 "Missing track id".
    let (_, _, body) = get_raw(t.router(), &format!("/rest/scrobble?id=abc&{a}&f=json")).await;
    let json = json_of(&body);
    assert_eq!(json["subsonic-response"]["error"]["code"], "10");
    assert_eq!(
        json["subsonic-response"]["error"]["message"],
        "Missing track id"
    );

    // Syntactically valid but unknown id → FK violation keeps Python's 500.
    let (status, _, _) = get_raw(t.router(), &format!("/rest/scrobble?id=99999&{a}&f=json")).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn star_unstar_get_starred_cycle_is_fixed() {
    let t = test_app().await;
    setup(&t).await;
    let c = seed(&t).await;
    let a = token_auth();

    // Python's /rest/star always 500s (`SELECT id FROM favorites` — no such
    // column); the port uses user_id and works.
    let (status, _, body) =
        get_raw(t.router(), &format!("/rest/star?id={}&{a}&f=json", c.t1)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json_of(&body)["subsonic-response"]["status"], "ok");

    // Idempotent — still a single favorites row.
    let (_, _, body) = get_raw(t.router(), &format!("/rest/star?id={}&{a}&f=json", c.t1)).await;
    assert_eq!(json_of(&body)["subsonic-response"]["status"], "ok");
    let (rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM favorites")
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(rows, 1);

    let (_, _, body) = get_raw(t.router(), &format!("/rest/getStarred?{a}&f=json")).await;
    let json = json_of(&body);
    let songs = elems(&json["subsonic-response"]["starred"]["song"]);
    assert_eq!(songs.len(), 1);
    assert_eq!(songs[0]["id"], c.t1.to_string());
    assert_eq!(songs[0]["title"], "Song One");

    // Unstar removes it; empty <starred /> self-closes.
    let (_, _, body) = get_raw(t.router(), &format!("/rest/unstar?id={}&{a}&f=json", c.t1)).await;
    assert_eq!(json_of(&body)["subsonic-response"]["status"], "ok");
    let (_, _, body) = get_raw(t.router(), &format!("/rest/getStarred?{a}&f=json")).await;
    let json = json_of(&body);
    assert_eq!(json["subsonic-response"]["starred"], serde_json::json!({}));
    let (_, _, body) = get_raw(t.router(), &format!("/rest/getStarred?{a}")).await;
    assert!(s(&body).contains("<starred />"), "{}", s(&body));

    // Validation + FK parity.
    let (_, _, body) = get_raw(t.router(), &format!("/rest/star?id=abc&{a}&f=json")).await;
    let json = json_of(&body);
    assert_eq!(json["subsonic-response"]["error"]["code"], "10");
    assert_eq!(json["subsonic-response"]["error"]["message"], "Invalid id");
    let (status, _, _) = get_raw(t.router(), &format!("/rest/star?id=99999&{a}&f=json")).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn random_songs_respects_size_and_defaults() {
    let t = test_app().await;
    setup(&t).await;
    seed(&t).await;
    let a = token_auth();

    let (_, _, body) = get_raw(
        t.router(),
        &format!("/rest/getRandomSongs?{a}&size=2&f=json"),
    )
    .await;
    let json = json_of(&body);
    let songs = elems(&json["subsonic-response"]["randomSongs"]["song"]);
    assert_eq!(songs.len(), 2);

    let (_, _, body) = get_raw(t.router(), &format!("/rest/getRandomSongs?{a}&f=json")).await;
    let json = json_of(&body);
    let songs = elems(&json["subsonic-response"]["randomSongs"]["song"]);
    assert_eq!(songs.len(), 4);

    // Divergence: garbage size → default instead of Python's 500.
    let (_, _, body) = get_raw(
        t.router(),
        &format!("/rest/getRandomSongs?{a}&size=zzz&f=json"),
    )
    .await;
    let json = json_of(&body);
    let songs = elems(&json["subsonic-response"]["randomSongs"]["song"]);
    assert_eq!(songs.len(), 4);
}

#[tokio::test]
async fn now_playing_after_scrobble() {
    let t = test_app().await;
    setup(&t).await;
    let c = seed(&t).await;
    let a = token_auth();

    // Empty before any play.
    let (_, _, body) = get_raw(t.router(), &format!("/rest/getNowPlaying?{a}&f=json")).await;
    let json = json_of(&body);
    assert!(json["subsonic-response"]["nowPlaying"]
        .get("entry")
        .is_none());

    let (_, _, body) = get_raw(
        t.router(),
        &format!("/rest/scrobble?id={}&{a}&f=json", c.t3),
    )
    .await;
    assert_eq!(json_of(&body)["subsonic-response"]["status"], "ok");

    let (_, _, body) = get_raw(t.router(), &format!("/rest/getNowPlaying?{a}&f=json")).await;
    let json = json_of(&body);
    let entry = &json["subsonic-response"]["nowPlaying"]["entry"];
    assert_eq!(entry["username"], "admin");
    assert_eq!(entry["minutesAgo"], "0");
    assert_eq!(entry["playerId"], "0");
    assert_eq!(entry["title"], "Song Three");
}

// ── Media ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn stream_serves_original_with_subsonic_headers_and_range() {
    let t = test_app().await;
    setup(&t).await;
    let c = seed(&t).await;
    let a = token_auth();

    let (status, headers, body) =
        get_raw(t.router(), &format!("/rest/stream?id={}&{a}", c.t1)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get("content-type").unwrap(), "audio/mpeg");
    assert_eq!(headers.get("accept-ranges").unwrap(), "bytes");
    assert_eq!(headers.get("content-disposition").unwrap(), "inline");
    assert_eq!(body, b"ID3one.mp3");

    // Range → 206 (native ServeFile, like Starlette's FileResponse).
    let response = t
        .router()
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/rest/stream?id={}&{a}", c.t1))
                .header("range", "bytes=0-2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        response
            .headers()
            .get("content-range")
            .unwrap()
            .to_str()
            .unwrap(),
        format!("bytes 0-2/{}", "ID3one.mp3".len())
    );
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&bytes[..], b"ID3");

    // download is the same handler.
    let (status, headers, body) =
        get_raw(t.router(), &format!("/rest/download?id={}&{a}", c.t1)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get("content-type").unwrap(), "audio/mpeg");
    assert_eq!(body, b"ID3one.mp3");

    // POST works too.
    let (status, _, _) = post_raw(t.router(), &format!("/rest/stream?id={}&{a}", c.t1)).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn stream_media_type_table() {
    let t = test_app().await;
    setup(&t).await;
    let a = token_auth();

    // .flac → audio/flac, .opus → audio/ogg (Python's Subsonic table).
    let flac = insert_track(&t, "song.flac", "F", "A", "B", None, 1.0, 1, 1, None, None).await;
    let opus = insert_track(&t, "song.opus", "O", "A", "B", None, 1.0, 1, 1, None, None).await;
    let unknown = insert_track(&t, "song.xyz", "U", "A", "B", None, 1.0, 1, 1, None, None).await;

    for (id, expect) in [
        (flac, "audio/flac"),
        (opus, "audio/ogg"),
        (unknown, "audio/mpeg"),
    ] {
        let (status, headers, _) = get_raw(t.router(), &format!("/rest/stream?id={id}&{a}")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers.get("content-type").unwrap(), expect, "id {id}");
    }
}

#[tokio::test]
async fn stream_errors_70_for_missing_ids_and_paths() {
    let t = test_app().await;
    setup(&t).await;
    let a = token_auth();

    for (id, msg) in [
        (99999.to_string(), "File not found"),
        ("abc".to_string(), "File not found"),
    ] {
        let (_, _, body) = get_raw(t.router(), &format!("/rest/stream?id={id}&{a}&f=json")).await;
        let json = json_of(&body);
        assert_eq!(json["subsonic-response"]["error"]["code"], "70");
        assert_eq!(json["subsonic-response"]["error"]["message"], msg);
    }

    // Row whose file vanished from disk.
    let ghost = t.music_dir.join("ghost.mp3");
    let ghost_id = insert_track(
        &t,
        ghost.to_str().unwrap(),
        "G",
        "A",
        "B",
        None,
        1.0,
        1,
        1,
        None,
        None,
    )
    .await;
    // insert_track writes the file — remove it to simulate the vanish.
    std::fs::remove_file(&ghost).unwrap();
    let (_, _, body) = get_raw(
        t.router(),
        &format!("/rest/stream?id={ghost_id}&{a}&f=json"),
    )
    .await;
    let json = json_of(&body);
    assert_eq!(json["subsonic-response"]["error"]["code"], "70");
    assert_eq!(
        json["subsonic-response"]["error"]["message"],
        "File not found on disk"
    );

    // Outside MUSIC_DIR — the containment check (Python serves it; we 70).
    let outside = std::env::temp_dir().join("moidify-subsonic-escape.mp3");
    std::fs::write(&outside, b"ID3escape").unwrap();
    let out_id = insert_track(
        &t,
        outside.to_str().unwrap(),
        "X",
        "A",
        "B",
        None,
        1.0,
        1,
        1,
        None,
        None,
    )
    .await;
    let (_, _, body) = get_raw(t.router(), &format!("/rest/stream?id={out_id}&{a}&f=json")).await;
    let json = json_of(&body);
    assert_eq!(json["subsonic-response"]["error"]["code"], "70");
    assert_eq!(
        json["subsonic-response"]["error"]["message"],
        "File not found on disk"
    );
    let _ = std::fs::remove_file(&outside);
}

#[tokio::test]
async fn cover_art_hash_legacy_and_404() {
    let t = test_app().await;
    setup(&t).await;
    let c = seed(&t).await;
    let a = token_auth();

    // Legacy {track_id}.png.
    std::fs::write(t.covers_dir.join(format!("{}.png", c.t1)), b"\x89PNG-t1").unwrap();
    let (status, headers, body) =
        get_raw(t.router(), &format!("/rest/getCoverArt?id={}&{a}", c.t1)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get("content-type").unwrap(), "image/png");
    assert_eq!(
        headers.get("cache-control").unwrap(),
        "public, max-age=86400"
    );
    assert_eq!(body, b"\x89PNG-t1");

    // cover_hash wins over the legacy file.
    sqlx::query("UPDATE tracks SET cover_hash = ? WHERE id = ?")
        .bind("hh123")
        .bind(c.t1)
        .execute(&t.pool)
        .await
        .unwrap();
    std::fs::write(t.covers_dir.join("hh123.jpg"), b"HASH-JPG").unwrap();
    let (status, headers, body) = get_raw(
        t.router(),
        &format!("/rest/getCoverArt?id={}&{a}&size=100", c.t1),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get("content-type").unwrap(), "image/jpeg");
    assert_eq!(body, b"HASH-JPG");

    // Nothing on disk → empty 404 (no placeholder like /api/cover).
    let (status, _, body) =
        get_raw(t.router(), &format!("/rest/getCoverArt?id={}&{a}", c.t2)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body.is_empty());

    // Non-numeric / zero ids resolve to no track → 404.
    for id in ["abc", "0"] {
        let (status, _, _) = get_raw(t.router(), &format!("/rest/getCoverArt?id={id}&{a}")).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "id={id}");
    }

    // Missing id → FastAPI 422 (checked before auth).
    let (status, _, _) = get_raw(t.router(), "/rest/getCoverArt").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}
