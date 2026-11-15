//! Subsonic REST API (`/rest/*`) — port of Python `subsonic.py` (23
//! endpoints, `f=xml|json|jsonp`), with the `.view` suffix stripping that
//! Starlette does in `server.py`'s middleware.
//!
//! Two deliberate divergences from Python (approved for the rewrite):
//!
//! 1. **Nested lists are real elements/arrays.** Python's `_ok` passes list
//!    values to `ElementTree.SubElement` as *attributes*, which ElementTree
//!    stringifies as Python reprs — so every listing endpoint serves
//!    `index="[{'_tag': ...}]"` and Subsonic clients show nothing. Here each
//!    item becomes a proper child element (XML) / array entry (JSON).
//! 2. **The rate limiter counts failed attempts only.** Python's
//!    `check_rate_limit` records every request, so a working client exhausts
//!    its 30 requests / 15 minutes on *successful* calls and gets error 40.
//!
//! Everything else — auth branch order, error codes, id scheme, SQL quirks,
//! value formatting — is a faithful port; `/rest` is not used by the web
//! frontend, which keeps its exact `/api` contract.

// `SubResult`/`require` carry an already-rendered `Response` in the `Err`
// variant (Subsonic failures are responses too) — boxing it would only add
// indirection without shrinking anything meaningful.
#![allow(clippy::result_large_err)]

pub mod auth;
pub mod browse;
pub mod child;
pub mod ids;
pub mod media;
pub mod response;
pub mod session;

use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde::Deserialize;

use crate::error::AppError;
use crate::state::AppState;
use response::Format;

/// Query parameters shared by every `/rest` endpoint. Unknown parameters are
/// ignored (like FastAPI's per-endpoint `Query` models).
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Params {
    pub u: Option<String>,
    pub p: Option<String>,
    pub t: Option<String>,
    pub s: Option<String>,
    pub token: Option<String>,
    pub f: Option<String>,
    pub callback: Option<String>,
    pub id: Option<String>,
    pub query: Option<String>,
    pub size: Option<String>,
    pub offset: Option<String>,
    #[serde(rename = "trackId")]
    pub track_id: Option<String>,
    #[serde(rename = "type")]
    pub list_type: Option<String>,
    #[serde(rename = "artistCount")]
    pub artist_count: Option<String>,
    #[serde(rename = "albumCount")]
    pub album_count: Option<String>,
    #[serde(rename = "songCount")]
    pub song_count: Option<String>,
}

impl Params {
    /// Requested response format (`f` + `callback`).
    pub fn format(&self) -> Format {
        Format::new(self.f.as_deref(), self.callback.as_deref())
    }

    /// Required `id` query parameter (FastAPI returns 422 before auth runs).
    pub fn require_id(&self) -> Result<&str, Response> {
        match self.id.as_deref() {
            Some(v) => Ok(v),
            None => Err(response::missing_param("id")),
        }
    }

    /// `int(param)` with a default — Python raises 500 on garbage here;
    /// the port falls back to the default instead.
    pub fn int_param(&self, value: Option<&str>, default: i64) -> i64 {
        value.and_then(|v| v.parse().ok()).unwrap_or(default)
    }
}

/// Render a success/error body in the caller's requested format.
pub fn respond(q: &Params, root: response::Elem) -> Response {
    response::render(&q.format(), &root)
}

/// Handler result. `Ok` is the response to send (Subsonic is always HTTP 200
/// with status in the body); `Err` is an already-rendered response — FastAPI
/// 422 validation, an XML auth error, or an infrastructure 500.
pub type SubResult = Result<Response, Response>;

/// Infrastructure failures (DB, Fernet, …) surface as the same HTTP 500 the
/// Python server produces; subsonic handlers bubble them via `?`.
impl From<AppError> for Response {
    fn from(e: AppError) -> Self {
        e.into_response()
    }
}

/// GET + POST on every endpoint, like FastAPI's `api_route(methods=[...])`.
/// Each path is also registered with a `.view` twin — Subsonic clients append
/// `.view` (`/rest/ping.view`), which Python strips in a pre-routing
/// middleware. Axum's `Router::layer` runs *after* routing and cannot rewrite
/// URIs, so the twin routes are equivalent and cheaper.
macro_rules! subsonic_route {
    ($router:expr, $path:literal, $handler:path) => {{
        let r = $router.route($path, get($handler).post($handler));
        r.route(concat!($path, ".view"), get($handler).post($handler))
    }};
}

pub fn router() -> Router<AppState> {
    use browse::*;
    use media::*;
    use session::*;

    let r = Router::new();
    let r = subsonic_route!(r, "/rest/ping", ping);
    let r = subsonic_route!(r, "/rest/getLicense", get_license);
    let r = subsonic_route!(r, "/rest/getMusicFolders", get_music_folders);
    let r = subsonic_route!(r, "/rest/getIndexes", get_indexes);
    let r = subsonic_route!(r, "/rest/getArtists", get_artists);
    let r = subsonic_route!(r, "/rest/getArtist", get_artist);
    let r = subsonic_route!(r, "/rest/getAlbum", get_album);
    let r = subsonic_route!(r, "/rest/getSong", get_song);
    let r = subsonic_route!(r, "/rest/getMusicDirectory", get_music_directory);
    let r = subsonic_route!(r, "/rest/stream", stream);
    let r = subsonic_route!(r, "/rest/download", download);
    let r = subsonic_route!(r, "/rest/getCoverArt", get_cover_art);
    let r = subsonic_route!(r, "/rest/getAvatar", get_avatar);
    let r = subsonic_route!(r, "/rest/search3", search3);
    let r = subsonic_route!(r, "/rest/getAlbumList2", get_album_list2);
    let r = subsonic_route!(r, "/rest/getPlaylists", get_playlists);
    let r = subsonic_route!(r, "/rest/getPlaylist", get_playlist);
    let r = subsonic_route!(r, "/rest/scrobble", scrobble);
    let r = subsonic_route!(r, "/rest/star", star);
    let r = subsonic_route!(r, "/rest/unstar", unstar);
    let r = subsonic_route!(r, "/rest/getStarred", get_starred);
    let r = subsonic_route!(r, "/rest/getRandomSongs", get_random_songs);
    subsonic_route!(r, "/rest/getNowPlaying", get_now_playing)
}
