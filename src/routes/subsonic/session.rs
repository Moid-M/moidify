//! Session/activity endpoints: `ping`, `getLicense`, `scrobble`, `star`,
//! `unstar`, `getStarred`, `getRandomSongs`, `getNowPlaying`.
//!
//! Port notes:
//! - `ping` and `getAvatar` are the only unauthenticated endpoints (Python
//!   never calls `_require_auth` there).
//! - `star` is fixed: Python ran `SELECT id FROM favorites`, but the table
//!   has no `id` column, so `/rest/star` returned 500 for every request
//!   (README promises star/unstar). Port uses `SELECT user_id`.
//! - `scrobble`/`star` with a syntactically valid but unknown track id keep
//!   Python's FK-violation 500 (favorites/play_history enforce FKs).

use axum::extract::{Query, State};

use super::auth;
use super::child::{fetch_tracks, TRACK_COLS, TRACK_COLS_T};
use super::response::{error, ok, Elem};
use super::{respond, Params, SubResult};
use crate::error::AppError;
use crate::state::AppState;

/// `GET|POST /rest/ping` — no auth required (matches the Python suite's
/// "ping does not [require auth]" expectations).
pub async fn ping(State(_app): State<AppState>, Query(q): Query<Params>) -> SubResult {
    Ok(respond(&q, ok()))
}

/// `GET|POST /rest/getLicense` — static license element after auth.
pub async fn get_license(State(app): State<AppState>, Query(q): Query<Params>) -> SubResult {
    let _user = auth::require(&app, &q).await?;
    let root = ok().with(
        Elem::new("license")
            .attr("valid", "true")
            .attr("email", "")
            .attr("licenseExpires", "")
            .attr("trialExpires", "")
            .attr("isTrial", "false"),
    );
    Ok(respond(&q, root))
}

/// `GET|POST /rest/scrobble` — bump play_count, append play_history.
/// `id` or `trackId`; non-numeric → error 10 "Missing track id".
pub async fn scrobble(State(app): State<AppState>, Query(q): Query<Params>) -> SubResult {
    let _user = auth::require(&app, &q).await?;

    let raw =
        q.id.as_deref()
            .filter(|v| !v.is_empty())
            .or_else(|| q.track_id.as_deref().filter(|v| !v.is_empty()))
            .unwrap_or("");
    let Ok(track_id) = raw.parse::<i64>() else {
        return Ok(respond(&q, error(10, "Missing track id")));
    };

    sqlx::query("UPDATE tracks SET play_count = play_count + 1 WHERE id = ?")
        .bind(track_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    sqlx::query("INSERT INTO play_history (track_id) VALUES (?)")
        .bind(track_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    Ok(respond(&q, ok()))
}

/// `GET|POST /rest/star` — add to favorites (idempotent). Non-numeric id →
/// error 10 "Invalid id".
pub async fn star(State(app): State<AppState>, Query(q): Query<Params>) -> SubResult {
    let user = auth::require(&app, &q).await?;

    let raw = q.id.as_deref().unwrap_or("");
    let Ok(track_id) = raw.parse::<i64>() else {
        return Ok(respond(&q, error(10, "Invalid id")));
    };

    let existing: Option<(i64,)> =
        sqlx::query_as("SELECT user_id FROM favorites WHERE user_id = ? AND track_id = ?")
            .bind(user.id)
            .bind(track_id)
            .fetch_optional(&app.pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    if existing.is_none() {
        sqlx::query("INSERT INTO favorites (user_id, track_id) VALUES (?, ?)")
            .bind(user.id)
            .bind(track_id)
            .execute(&app.pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    }

    Ok(respond(&q, ok()))
}

/// `GET|POST /rest/unstar` — remove from favorites (no existence check).
/// Non-numeric id → error 10 "Invalid id".
pub async fn unstar(State(app): State<AppState>, Query(q): Query<Params>) -> SubResult {
    let user = auth::require(&app, &q).await?;

    let raw = q.id.as_deref().unwrap_or("");
    let Ok(track_id) = raw.parse::<i64>() else {
        return Ok(respond(&q, error(10, "Invalid id")));
    };

    sqlx::query("DELETE FROM favorites WHERE user_id = ? AND track_id = ?")
        .bind(user.id)
        .bind(track_id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    Ok(respond(&q, ok()))
}

/// `GET|POST /rest/getStarred` — this user's favorites, newest first.
pub async fn get_starred(State(app): State<AppState>, Query(q): Query<Params>) -> SubResult {
    let user = auth::require(&app, &q).await?;

    let sql = format!(
        "SELECT {TRACK_COLS_T} FROM tracks t \
         JOIN favorites f ON f.track_id = t.id \
         WHERE f.user_id = ? ORDER BY f.added_at DESC"
    );
    let songs = fetch_tracks(&app.pool, sqlx::query(&sql).bind(user.id)).await?;

    let mut starred = Elem::new("starred");
    for song in &songs {
        starred = starred.with(song.child("song"));
    }
    Ok(respond(&q, ok().with(starred)))
}

/// `GET|POST /rest/getRandomSongs` — `size` (default 10) random tracks.
pub async fn get_random_songs(State(app): State<AppState>, Query(q): Query<Params>) -> SubResult {
    let _user = auth::require(&app, &q).await?;

    let size = q.int_param(q.size.as_deref(), 10);
    let sql = format!("SELECT {TRACK_COLS} FROM tracks ORDER BY RANDOM() LIMIT ?");
    let songs = fetch_tracks(&app.pool, sqlx::query(&sql).bind(size)).await?;

    // Songs nest *inside* randomSongs (Python's `_ok` key nesting).
    let mut random = Elem::new("randomSongs");
    for song in &songs {
        random = random.with(song.child("song"));
    }
    Ok(respond(&q, ok().with(random)))
}

/// `GET|POST /rest/getNowPlaying` — most recent play_history entry with the
/// requesting username attached.
pub async fn get_now_playing(State(app): State<AppState>, Query(q): Query<Params>) -> SubResult {
    let user = auth::require(&app, &q).await?;

    let sql = format!(
        "SELECT {TRACK_COLS_T} FROM play_history ph \
         JOIN tracks t ON ph.track_id = t.id \
         ORDER BY ph.played_at DESC LIMIT 1"
    );
    let song = fetch_tracks(&app.pool, sqlx::query(&sql))
        .await?
        .into_iter()
        .next();

    // Entries nest *inside* nowPlaying (Python's `_ok` key nesting).
    let mut now = Elem::new("nowPlaying");
    if let Some(song) = song {
        let entry = song
            .child("entry")
            .attr("username", &user.username)
            .attr("minutesAgo", "0")
            .attr("playerId", "0");
        now = now.with(entry);
    }
    Ok(respond(&q, ok().with(now)))
}
