//! Player state CRUD. Byte-compatible with Python `routes/player.py`.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;

use crate::auth::extractors::AuthUser;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/api/player/state",
        get(get_state).put(save_state).delete(clear_state),
    )
}

fn default_index() -> i64 {
    -1
}
fn default_time() -> f64 {
    0.0
}
fn default_speed() -> f64 {
    1.0
}
fn default_volume() -> f64 {
    0.7
}
fn default_repeat() -> String {
    "off".to_string()
}

#[derive(Deserialize)]
struct PlayerStateBody {
    #[serde(default = "empty_array")]
    queue: Value,
    #[serde(default = "default_index")]
    current_index: i64,
    #[serde(default = "default_time")]
    current_time: f64,
    #[serde(default)]
    shuffle: bool,
    #[serde(default = "default_repeat")]
    repeat_mode: String,
    #[serde(default = "default_speed")]
    playback_speed: f64,
    #[serde(default = "default_volume")]
    volume: f64,
    #[serde(default = "empty_array")]
    shuffle_order: Value,
    #[serde(default)]
    shuffle_index: i64,
}

fn empty_array() -> Value {
    json!([])
}

async fn get_state(
    State(app): State<AppState>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    let row = sqlx::query(
        r#"SELECT queue, current_index, "current_time", shuffle, repeat_mode,
                  playback_speed, volume, shuffle_order, shuffle_index
           FROM player_state WHERE user_id = ?"#,
    )
    .bind(user.id)
    .fetch_optional(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let Some(row) = row else {
        return Ok(Json(json!({
            "queue": [],
            "current_index": -1,
            "current_time": 0,
            "shuffle": false,
            "repeat_mode": "off",
            "playback_speed": 1.0,
            "volume": 0.7,
            "shuffle_order": [],
            "shuffle_index": 0,
        })));
    };

    let queue: Value = row
        .get::<Option<String>, _>("queue")
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| json!([]));
    let shuffle_order: Value = row
        .get::<Option<String>, _>("shuffle_order")
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| json!([]));

    Ok(Json(json!({
        "queue": queue,
        "current_index": row.get::<Option<i64>, _>("current_index").unwrap_or(-1),
        "current_time": row.get::<Option<f64>, _>("current_time").unwrap_or(0.0),
        "shuffle": row.get::<Option<i64>, _>("shuffle").unwrap_or(0) != 0,
        "repeat_mode": row.get::<Option<String>, _>("repeat_mode").unwrap_or_else(|| "off".into()),
        "playback_speed": row.get::<Option<f64>, _>("playback_speed").unwrap_or(1.0),
        "volume": row.get::<Option<f64>, _>("volume").unwrap_or(0.7),
        "shuffle_order": shuffle_order,
        "shuffle_index": row.get::<Option<i64>, _>("shuffle_index").unwrap_or(0),
    })))
}

async fn save_state(
    State(app): State<AppState>,
    AuthUser(user): AuthUser,
    Json(body): Json<PlayerStateBody>,
) -> AppResult<Json<Value>> {
    let queue = serde_json::to_string(&body.queue).map_err(|e| AppError::Internal(e.into()))?;
    let shuffle_order =
        serde_json::to_string(&body.shuffle_order).map_err(|e| AppError::Internal(e.into()))?;
    let now = chrono::Utc::now().timestamp();

    sqlx::query(
        r#"INSERT INTO player_state (user_id, queue, current_index, current_time, shuffle,
                  repeat_mode, playback_speed, volume, shuffle_order, shuffle_index, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(user_id) DO UPDATE SET
             queue=excluded.queue, current_index=excluded.current_index,
             current_time=excluded.current_time, shuffle=excluded.shuffle,
             repeat_mode=excluded.repeat_mode, playback_speed=excluded.playback_speed,
             volume=excluded.volume, shuffle_order=excluded.shuffle_order,
             shuffle_index=excluded.shuffle_index, updated_at=excluded.updated_at"#,
    )
    .bind(user.id)
    .bind(&queue)
    .bind(body.current_index)
    .bind(body.current_time)
    .bind(if body.shuffle { 1 } else { 0 })
    .bind(&body.repeat_mode)
    .bind(body.playback_speed)
    .bind(body.volume)
    .bind(&shuffle_order)
    .bind(body.shuffle_index)
    .bind(now)
    .execute(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    Ok(Json(json!({ "ok": true })))
}

async fn clear_state(
    State(app): State<AppState>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    sqlx::query("DELETE FROM player_state WHERE user_id = ?")
        .bind(user.id)
        .execute(&app.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(json!({ "ok": true })))
}
