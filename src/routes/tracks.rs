//! Track / album / artist / genre listings + home feed + lyrics.
//! Byte-compatible with Python `routes/tracks.py`.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;

use crate::auth::extractors::AuthUser;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::util::{escape_like, normalize};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/tracks", get(list_tracks))
        .route("/api/tracks/{track_id}", get(get_track))
        .route("/api/tracks/{track_id}/similar", get(get_similar))
        .route("/api/tracks/{track_id}/gain", get(get_gain))
        .route(
            "/api/tracks/{track_id}/lyrics",
            get(get_lyrics).put(update_lyrics),
        )
        .route("/api/albums", get(list_albums))
        .route("/api/albums/tracks", get(list_album_tracks))
        .route("/api/artist-image/{*artist_name}", get(artist_image))
        .route("/api/artists", get(list_artists))
        .route("/api/artists/tracks", get(list_artist_tracks))
        .route("/api/genres", get(list_genres))
        .route("/api/genres/tracks", get(list_genre_tracks))
        .route("/api/home", get(home_feed))
        .route("/api/duplicates", get(list_duplicates))
}

// ── Query params ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ListTracksQuery {
    search: Option<String>,
    #[serde(default)]
    limit: i64,
    #[serde(default)]
    offset: i64,
}

#[derive(Deserialize)]
struct AlbumTracksQuery {
    album: String,
    artist: Option<String>,
}

#[derive(Deserialize)]
struct ArtistTracksQuery {
    artist: String,
}

#[derive(Deserialize)]
struct GenreTracksQuery {
    genre: String,
}

#[derive(Deserialize)]
struct GroupedQuery {
    #[serde(default)]
    grouped: bool,
}

#[derive(Deserialize)]
struct SimilarQuery {
    #[serde(default = "default_similar_limit")]
    limit: i64,
}
fn default_similar_limit() -> i64 {
    20
}

#[derive(Deserialize)]
struct UpdateLyricsBody {
    #[serde(default)]
    lyrics: String,
}

// ── Tracks ──────────────────────────────────────────────────────────────────

async fn list_tracks(
    State(app): State<AppState>,
    Query(q): Query<ListTracksQuery>,
) -> AppResult<Json<Value>> {
    let limit = q.limit.max(0);
    let offset = q.offset.max(0);
    let limit_clause = if limit > 0 { " LIMIT ? OFFSET ?" } else { "" };
    let order =
        "ORDER BY COALESCE(NULLIF(album_artist,''), artist), album, disc_number, track_number";

    let rows: Vec<sqlx::sqlite::SqliteRow> = if let Some(search) =
        q.search.filter(|s| !s.is_empty())
    {
        let safe = escape_like(&search);
        let normalized = normalize(&search);
        let like_pattern = format!("%{safe}%");
        let norm_pattern = format!("%{}%", escape_like(&normalized));

        if normalized.to_lowercase() != search.to_lowercase() {
            let sql = format!(
                "SELECT id FROM tracks WHERE title LIKE ? ESCAPE '\\' OR artist LIKE ? ESCAPE '\\' OR album LIKE ? ESCAPE '\\'
                 OR title LIKE ? ESCAPE '\\' OR artist LIKE ? ESCAPE '\\' OR album LIKE ? ESCAPE '\\'
                 {order} {limit_clause}"
            );
            let mut query = sqlx::query(&sql)
                .bind(&like_pattern)
                .bind(&like_pattern)
                .bind(&like_pattern)
                .bind(&norm_pattern)
                .bind(&norm_pattern)
                .bind(&norm_pattern);
            if limit > 0 {
                query = query.bind(limit).bind(offset);
            }
            query
                .fetch_all(&app.pool)
                .await
                .map_err(|e| AppError::Internal(e.into()))?
        } else {
            let sql = format!(
                "SELECT id FROM tracks WHERE title LIKE ? ESCAPE '\\' OR artist LIKE ? ESCAPE '\\' OR album LIKE ? ESCAPE '\\'
                 {order} {limit_clause}"
            );
            let mut query = sqlx::query(&sql)
                .bind(&like_pattern)
                .bind(&like_pattern)
                .bind(&like_pattern);
            if limit > 0 {
                query = query.bind(limit).bind(offset);
            }
            query
                .fetch_all(&app.pool)
                .await
                .map_err(|e| AppError::Internal(e.into()))?
        }
    } else {
        let sql = format!("SELECT id FROM tracks {order} {limit_clause}");
        let mut query = sqlx::query(&sql);
        if limit > 0 {
            query = query.bind(limit).bind(offset);
        }
        query
            .fetch_all(&app.pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?
    };

    // Fetch full rows preserving order
    let mut tracks = Vec::with_capacity(rows.len());
    for row in rows {
        let id: i64 = row.get("id");
        let t = fetch_track(&app, id).await?;
        tracks.push(t);
    }

    let (total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks")
        .fetch_one(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    if limit > 0 {
        Ok(Json(json!({
            "tracks": tracks,
            "total": total,
            "limit": limit,
            "offset": offset,
        })))
    } else {
        Ok(Json(serde_json::Value::Array(tracks)))
    }
}

async fn fetch_track(app: &AppState, id: i64) -> AppResult<Value> {
    let row = sqlx::query("SELECT * FROM tracks WHERE id = ?")
        .bind(id)
        .fetch_optional(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    match row {
        Some(r) => Ok(json!({
            "id": r.get::<i64, _>("id"),
            "file_path": r.get::<String, _>("file_path"),
            "file_hash": r.get::<Option<String>, _>("file_hash"),
            "title": r.get::<Option<String>, _>("title"),
            "artist": r.get::<Option<String>, _>("artist"),
            "album_artist": r.get::<Option<String>, _>("album_artist"),
            "album": r.get::<Option<String>, _>("album"),
            "track_number": r.get::<Option<i64>, _>("track_number"),
            "disc_number": r.get::<Option<i64>, _>("disc_number"),
            "genre": r.get::<Option<String>, _>("genre"),
            "year": r.get::<Option<i64>, _>("year"),
            "duration": r.get::<Option<f64>, _>("duration"),
            "has_cover": r.get::<Option<i64>, _>("has_cover"),
            "play_count": r.get::<Option<i64>, _>("play_count"),
            "lyrics": r.get::<Option<String>, _>("lyrics"),
            "cover_hash": r.get::<Option<String>, _>("cover_hash"),
            "artist_img_hash": r.get::<Option<String>, _>("artist_img_hash"),
            "created_at": r.get::<Option<String>, _>("created_at"),
        })),
        None => Err(AppError::NotFound("Track not found".into())),
    }
}

async fn get_track(
    State(app): State<AppState>,
    Path(track_id): Path<i64>,
) -> AppResult<Json<Value>> {
    Ok(Json(fetch_track(&app, track_id).await?))
}

async fn get_similar(
    State(app): State<AppState>,
    Path(track_id): Path<i64>,
    Query(q): Query<SimilarQuery>,
) -> AppResult<Json<Value>> {
    let track = sqlx::query("SELECT id, genre, artist, album_artist FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_optional(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let Some(track) = track else {
        return Err(AppError::NotFound("Track not found".into()));
    };
    let genre = track.get::<Option<String>, _>("genre");
    let artist = track.get::<Option<String>, _>("artist");
    let album_artist = track.get::<Option<String>, _>("album_artist");
    let match_artist = album_artist.filter(|s| !s.is_empty()).or(artist);

    let ids: Vec<(i64,)> = sqlx::query_as(
        r#"SELECT DISTINCT id FROM tracks
           WHERE id != ? AND (genre IS ? OR genre = ? OR artist = ?)
           ORDER BY CASE WHEN artist = ? THEN 0 ELSE 1 END, RANDOM()
           LIMIT ?"#,
    )
    .bind(track_id)
    .bind(&genre)
    .bind(&genre)
    .bind(&match_artist)
    .bind(&match_artist)
    .bind(q.limit)
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let mut out = Vec::with_capacity(ids.len());
    for (id,) in ids {
        out.push(fetch_track(&app, id).await?);
    }
    Ok(Json(serde_json::Value::Array(out)))
}

/// ReplayGain — reads the tag live from the file (lofty, Phase 5).
/// Until lofty lands, returns gain: null (same shape as Python with no tag).
async fn get_gain(
    State(app): State<AppState>,
    Path(track_id): Path<i64>,
) -> AppResult<Json<Value>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT file_path FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_optional(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    if row.is_none() {
        return Err(AppError::NotFound("Track not found".into()));
    }
    // TODO(phase-5): read REPLAYGAIN_TRACK_GAIN via lofty
    Ok(Json(json!({ "gain": null })))
}

async fn get_lyrics(
    State(app): State<AppState>,
    Path(track_id): Path<i64>,
) -> AppResult<Json<Value>> {
    let row = sqlx::query("SELECT lyrics, artist, title, album FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_optional(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let Some(row) = row else {
        return Err(AppError::NotFound("Track not found".into()));
    };
    let mut lyrics = row.get::<Option<String>, _>("lyrics");
    if lyrics.is_none() {
        let artist = row.get::<Option<String>, _>("artist").unwrap_or_default();
        let title = row.get::<Option<String>, _>("title").unwrap_or_default();
        let album = row.get::<Option<String>, _>("album").unwrap_or_default();
        // LRCLIB fetch — Phase 8; returns null until then.
        let fetched = crate::services::lyrics::fetch_from_lrclib(&artist, &title, &album).await;
        if let Some(text) = fetched {
            sqlx::query("UPDATE tracks SET lyrics = ? WHERE id = ?")
                .bind(&text)
                .bind(track_id)
                .execute(&app.pool)
                .await
                .map_err(|e| AppError::Internal(e.into()))?;
            lyrics = Some(text);
        }
    }
    Ok(Json(json!({ "lyrics": lyrics })))
}

async fn update_lyrics(
    State(app): State<AppState>,
    AuthUser(_user): AuthUser,
    Path(track_id): Path<i64>,
    Json(body): Json<UpdateLyricsBody>,
) -> AppResult<Json<Value>> {
    sqlx::query("UPDATE tracks SET lyrics = ? WHERE id = ?")
        .bind(&body.lyrics)
        .bind(track_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({ "ok": true })))
}

// ── Albums ──────────────────────────────────────────────────────────────────

async fn list_albums(
    State(app): State<AppState>,
    Query(q): Query<GroupedQuery>,
) -> AppResult<Json<Value>> {
    let rows = sqlx::query(
        r#"SELECT album, artist, album_artist, COUNT(*) as track_count,
                  MAX(has_cover) as has_cover, MIN(id) as cover_track_id
           FROM tracks
           WHERE album IS NOT NULL
           GROUP BY album, COALESCE(NULLIF(album_artist,''), artist)
           ORDER BY COALESCE(NULLIF(album_artist,''), artist), album"#,
    )
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let albums: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "album": r.get::<Option<String>, _>("album"),
                "artist": r.get::<Option<String>, _>("artist"),
                "album_artist": r.get::<Option<String>, _>("album_artist"),
                "track_count": r.get::<i64, _>("track_count"),
                "has_cover": r.get::<Option<i64>, _>("has_cover"),
                "cover_track_id": r.get::<Option<i64>, _>("cover_track_id"),
            })
        })
        .collect();

    if !q.grouped {
        return Ok(Json(serde_json::Value::Array(albums)));
    }

    // Group editions: base name + artist → versions[]
    use std::collections::BTreeMap;
    let mut groups: BTreeMap<(String, String), Value> = BTreeMap::new();
    for a in &albums {
        let name = a["album"].as_str().unwrap_or("");
        let base = base_album_name(name);
        let artist = a["album_artist"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or_else(|| a["artist"].as_str())
            .unwrap_or("")
            .to_string();
        let key = (base.clone(), artist.clone());
        groups
            .entry(key)
            .or_insert_with(|| json!({ "base_album": base, "artist": artist, "versions": [] }))
            .get_mut("versions")
            .and_then(|v| v.as_array_mut())
            .expect("versions array")
            .push(a.clone());
    }

    let mut out: Vec<Value> = groups.into_values().collect();
    out.sort_by(|a, b| {
        let aa = a["artist"].as_str().unwrap_or("").to_lowercase();
        let bb = b["artist"].as_str().unwrap_or("").to_lowercase();
        aa.cmp(&bb).then_with(|| {
            let ab = a["base_album"].as_str().unwrap_or("").to_lowercase();
            let bb2 = b["base_album"].as_str().unwrap_or("").to_lowercase();
            ab.cmp(&bb2)
        })
    });
    Ok(Json(serde_json::Value::Array(out)))
}

/// Strip edition suffixes: "Album (Deluxe Edition)" → "Album".
fn base_album_name(name: &str) -> String {
    if name.is_empty() {
        return name.to_string();
    }
    // Mirror the Python _EDITION_PATTERN — bracketed edition tokens.
    lazy_edition_re().replace_all(name, "").trim().to_string()
}

fn lazy_edition_re() -> &'static regex::Regex {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r#"(?i)\s*[\(\[]\s*\d*\s*(?:Deluxe\s*(?:Edition|Version)?|Remaster(?:ed)?|Expanded\s*(?:Edition|Version)?|Bonus\s*Track\s*(?:Edition|Version)?|Special\s*(?:Edition|Version)?|Collector'?s?\s*(?:Edition|Version)?|Limited\s*(?:Edition|Version)?|Super\s*Deluxe|Disk\s*\d+|Disc\s*\d+|Version\s*\d+|Anniversary\s*(?:Edition)?|Reissue|Original|Single|Explicit|Clean)\s*[\)\]]"#,
        )
        .expect("edition regex")
    })
}

async fn list_album_tracks(
    State(app): State<AppState>,
    Query(q): Query<AlbumTracksQuery>,
) -> AppResult<Json<Value>> {
    let rows = if let Some(artist) = q.artist.filter(|s| !s.is_empty()) {
        sqlx::query(
            r#"SELECT * FROM tracks WHERE album = ?
               AND COALESCE(NULLIF(album_artist,''), artist) = ?
               ORDER BY disc_number, track_number"#,
        )
        .bind(&q.album)
        .bind(&artist)
        .fetch_all(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    } else {
        sqlx::query("SELECT * FROM tracks WHERE album = ? ORDER BY disc_number, track_number")
            .bind(&q.album)
            .fetch_all(&app.pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?
    };
    Ok(Json(Value::Array(rows_to_tracks(&rows))))
}

// ── Artists ─────────────────────────────────────────────────────────────────

async fn list_artists(State(app): State<AppState>) -> AppResult<Json<Value>> {
    let rows = sqlx::query(
        r#"SELECT COALESCE(NULLIF(album_artist,''), artist) as artist,
                  COUNT(*) as track_count,
                  COUNT(DISTINCT album) as album_count,
                  MIN(id) as cover_track_id
           FROM tracks
           WHERE artist IS NOT NULL
           GROUP BY COALESCE(NULLIF(album_artist,''), artist)
           ORDER BY artist"#,
    )
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "artist": r.get::<Option<String>, _>("artist"),
                "track_count": r.get::<i64, _>("track_count"),
                "album_count": r.get::<i64, _>("album_count"),
                "cover_track_id": r.get::<Option<i64>, _>("cover_track_id"),
            })
        })
        .collect();
    Ok(Json(serde_json::Value::Array(out)))
}

async fn list_artist_tracks(
    State(app): State<AppState>,
    Query(q): Query<ArtistTracksQuery>,
) -> AppResult<Json<Value>> {
    let rows = sqlx::query(
        r#"SELECT * FROM tracks
           WHERE COALESCE(NULLIF(album_artist,''), artist) = ?
           ORDER BY album, disc_number, track_number"#,
    )
    .bind(&q.artist)
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(Value::Array(rows_to_tracks(&rows))))
}

// ── Genres ──────────────────────────────────────────────────────────────────

async fn list_genres(State(app): State<AppState>) -> AppResult<Json<Value>> {
    let rows = sqlx::query(
        r#"SELECT genre, COUNT(*) as track_count,
                  COUNT(DISTINCT album) as album_count
           FROM tracks
           WHERE genre IS NOT NULL AND genre != ''
           GROUP BY genre
           ORDER BY genre"#,
    )
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "genre": r.get::<Option<String>, _>("genre"),
                "track_count": r.get::<i64, _>("track_count"),
                "album_count": r.get::<i64, _>("album_count"),
            })
        })
        .collect();
    Ok(Json(serde_json::Value::Array(out)))
}

async fn list_genre_tracks(
    State(app): State<AppState>,
    Query(q): Query<GenreTracksQuery>,
) -> AppResult<Json<Value>> {
    let rows = sqlx::query(
        "SELECT * FROM tracks WHERE genre = ? ORDER BY album, disc_number, track_number",
    )
    .bind(&q.genre)
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(Value::Array(rows_to_tracks(&rows))))
}

// ── Home feed ───────────────────────────────────────────────────────────────

async fn home_feed(State(app): State<AppState>, headers: HeaderMap) -> AppResult<Json<Value>> {
    let token = headers
        .get("token")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let user = match token {
        Some(t) => crate::auth::session::get_user_from_token(&app.pool, &t)
            .await
            .map_err(|e| AppError::Internal(e.into()))?,
        None => None,
    };

    // Recently played (only for logged-in users, per Python)
    let recently_played = if user.is_some() {
        let rows = sqlx::query(
            r#"SELECT DISTINCT t.* FROM play_history ph
               JOIN tracks t ON ph.track_id = t.id
               GROUP BY t.id
               ORDER BY MAX(ph.played_at) DESC
               LIMIT 20"#,
        )
        .fetch_all(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
        rows_to_tracks(&rows)
    } else {
        vec![]
    };

    // Recommended albums (12 random)
    let rec_album_rows = sqlx::query(
        r#"SELECT t.album, t.artist, t.album_artist,
                  COUNT(*) as track_count, MAX(t.has_cover) as has_cover, MIN(t.id) as cover_track_id
           FROM tracks t JOIN (
               SELECT DISTINCT album, COALESCE(NULLIF(album_artist,''), artist) as artist_key
               FROM tracks WHERE album IS NOT NULL
               ORDER BY RANDOM() LIMIT 12
           ) r ON t.album = r.album AND COALESCE(NULLIF(t.album_artist,''), t.artist) = r.artist_key
           GROUP BY t.album, t.artist"#,
    )
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    let recommended_albums: Vec<Value> = rec_album_rows
        .iter()
        .map(|r| {
            json!({
                "album": r.get::<Option<String>, _>("album"),
                "artist": r.get::<Option<String>, _>("artist"),
                "album_artist": r.get::<Option<String>, _>("album_artist"),
                "track_count": r.get::<i64, _>("track_count"),
                "has_cover": r.get::<Option<i64>, _>("has_cover"),
                "cover_track_id": r.get::<Option<i64>, _>("cover_track_id"),
            })
        })
        .collect();

    // Recommended tracks (20 random)
    let rec_track_rows = sqlx::query("SELECT * FROM tracks ORDER BY RANDOM() LIMIT 20")
        .fetch_all(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let recommended_tracks = rows_to_tracks(&rec_track_rows);

    // Recommended artists (12 random)
    let rec_artist_rows = sqlx::query(
        r#"SELECT COALESCE(NULLIF(t.album_artist,''), t.artist) as artist,
                  COUNT(*) as track_count, COUNT(DISTINCT t.album) as album_count,
                  MIN(t.id) as cover_track_id
           FROM tracks t JOIN (
               SELECT DISTINCT COALESCE(NULLIF(album_artist,''), artist) as artist_key
               FROM tracks WHERE artist IS NOT NULL
               ORDER BY RANDOM() LIMIT 12
           ) r ON COALESCE(NULLIF(t.album_artist,''), t.artist) = r.artist_key
           GROUP BY COALESCE(NULLIF(t.album_artist,''), t.artist)"#,
    )
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    let recommended_artists: Vec<Value> = rec_artist_rows
        .iter()
        .map(|r| {
            json!({
                "artist": r.get::<Option<String>, _>("artist"),
                "track_count": r.get::<i64, _>("track_count"),
                "album_count": r.get::<i64, _>("album_count"),
                "cover_track_id": r.get::<Option<i64>, _>("cover_track_id"),
            })
        })
        .collect();

    // Playlists (only for logged-in users)
    let playlists = if let Some(u) = &user {
        let rows = sqlx::query(
            r#"SELECT p.*, COUNT(pt.id) as track_count
               FROM playlists p
               LEFT JOIN playlist_tracks pt ON pt.playlist_id = p.id
               WHERE p.user_id = ?
               GROUP BY p.id
               ORDER BY RANDOM() LIMIT 6"#,
        )
        .bind(u.id)
        .fetch_all(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
        rows.iter()
            .map(|r| {
                json!({
                    "id": r.get::<i64, _>("id"),
                    "name": r.get::<String, _>("name"),
                    "user_id": r.get::<i64, _>("user_id"),
                    "folder_id": r.get::<Option<i64>, _>("folder_id"),
                    "created_at": r.get::<Option<String>, _>("created_at"),
                    "track_count": r.get::<i64, _>("track_count"),
                })
            })
            .collect::<Vec<_>>()
    } else {
        vec![]
    };

    Ok(Json(json!({
        "recently_played": recently_played,
        "recommended_albums": recommended_albums,
        "recommended_tracks": recommended_tracks,
        "recommended_artists": recommended_artists,
        "playlists": playlists,
    })))
}

// ── Duplicates ──────────────────────────────────────────────────────────────

async fn list_duplicates(State(app): State<AppState>) -> AppResult<Json<Value>> {
    let rows = sqlx::query(
        r#"SELECT t1.id as id1, t2.id as id2,
                  t1.title, t1.artist, t1.album,
                  t1.duration, t1.file_path as file1, t2.file_path as file2
           FROM tracks t1
           JOIN tracks t2 ON LOWER(t1.title) = LOWER(t2.title)
                         AND LOWER(COALESCE(t1.artist,'')) = LOWER(COALESCE(t2.artist,''))
                         AND ABS(COALESCE(t1.duration,0) - COALESCE(t2.duration,0)) < 2
                         AND t1.id < t2.id
           ORDER BY t1.title"#,
    )
    .fetch_all(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id1": r.get::<i64, _>("id1"),
                "id2": r.get::<i64, _>("id2"),
                "title": r.get::<Option<String>, _>("title"),
                "artist": r.get::<Option<String>, _>("artist"),
                "album": r.get::<Option<String>, _>("album"),
                "duration": r.get::<Option<f64>, _>("duration"),
                "file1": r.get::<String, _>("file1"),
                "file2": r.get::<String, _>("file2"),
            })
        })
        .collect();
    Ok(Json(serde_json::Value::Array(out)))
}

// ── Artist image (Deezer + SVG fallback) ────────────────────────────────────

async fn artist_image(State(app): State<AppState>, Path(artist_name): Path<String>) -> Response {
    let safe_name = {
        let t = artist_name.trim();
        if t.is_empty() {
            "?".to_string()
        } else {
            t.to_string()
        }
    };

    let cache_key = md5_hex(&safe_name.to_lowercase())[..16].to_string();
    for (ext, mt) in [
        ("jpg", "image/jpeg"),
        ("png", "image/png"),
        ("webp", "image/webp"),
    ] {
        let p = app
            .config
            .covers_dir
            .join(format!("artist_{cache_key}.{ext}"));
        if p.exists() {
            return file_response(p, mt, 86400);
        }
    }

    // Deezer fetch — Phase 8 (services::artist_image). Until then, SVG fallback.
    if let Some(resp) =
        crate::services::artist_image::fetch_deezer(&safe_name, &cache_key, &app.config.covers_dir)
            .await
    {
        return resp;
    }

    // Gradient SVG fallback
    let h = md5_hex(&safe_name);
    let hue1 = i64::from_str_radix(&h[..2], 16).unwrap_or(0) % 360;
    let hue2 = (hue1 + 40) % 360;
    let initial = safe_name
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".into());
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200" viewBox="0 0 200 200">
      <defs><linearGradient id="g" x1="0%" y1="0%" x2="100%" y2="100%">
        <stop offset="0%" style="stop-color:hsl({hue1},60%,35%)"/>
        <stop offset="100%" style="stop-color:hsl({hue2},60%,45%)"/>
      </linearGradient></defs>
      <rect width="200" height="200" rx="40" fill="url(#g)"/>
      <text x="100" y="120" text-anchor="middle" fill="rgba(255,255,255,0.85)" font-size="80" font-family="system-ui,sans-serif" font-weight="600">{initial}</text>
    </svg>"#
    );
    (
        StatusCode::OK,
        [
            ("content-type", "image/svg+xml"),
            ("cache-control", "public, max-age=3600"),
        ],
        svg,
    )
        .into_response()
}

fn file_response(path: std::path::PathBuf, mime: &str, max_age: u64) -> Response {
    match std::fs::read(&path) {
        Ok(bytes) => (
            StatusCode::OK,
            [
                ("content-type", mime.to_string()),
                ("cache-control", format!("public, max-age={max_age}")),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

fn md5_hex(s: &str) -> String {
    use md5::Digest;
    format!("{:x}", md5::Md5::digest(s.as_bytes()))
}

// ── Shared helpers ──────────────────────────────────────────────────────────

/// Shared track-row → JSON shape (also used by playlists/favorites joins).
pub(crate) fn row_to_track(r: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": r.get::<i64, _>("id"),
        "file_path": r.get::<String, _>("file_path"),
        "file_hash": r.get::<Option<String>, _>("file_hash"),
        "title": r.get::<Option<String>, _>("title"),
        "artist": r.get::<Option<String>, _>("artist"),
        "album_artist": r.get::<Option<String>, _>("album_artist"),
        "album": r.get::<Option<String>, _>("album"),
        "track_number": r.get::<Option<i64>, _>("track_number"),
        "disc_number": r.get::<Option<i64>, _>("disc_number"),
        "genre": r.get::<Option<String>, _>("genre"),
        "year": r.get::<Option<i64>, _>("year"),
        "duration": r.get::<Option<f64>, _>("duration"),
        "has_cover": r.get::<Option<i64>, _>("has_cover"),
        "play_count": r.get::<Option<i64>, _>("play_count"),
        "lyrics": r.get::<Option<String>, _>("lyrics"),
        "cover_hash": r.get::<Option<String>, _>("cover_hash"),
        "artist_img_hash": r.get::<Option<String>, _>("artist_img_hash"),
        "created_at": r.get::<Option<String>, _>("created_at"),
    })
}

fn rows_to_tracks(rows: &[sqlx::sqlite::SqliteRow]) -> Vec<Value> {
    rows.iter().map(row_to_track).collect()
}
