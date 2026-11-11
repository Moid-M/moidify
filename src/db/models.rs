//! Row types mirroring the SQLite schema. `Serialize` shapes match the
//! JSON the Python backend emitted (field names are the contract).

use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    pub is_admin: i64,
}

/// Full user row including secrets — never serialized directly.
#[derive(Debug, Clone, FromRow)]
pub struct UserAuth {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub salt: String,
    pub password_encrypted: Option<String>,
    pub is_admin: i64,
    pub failed_attempts: Option<i64>,
    pub locked_until: Option<i64>,
}

impl UserAuth {
    pub fn to_user(&self) -> User {
        User {
            id: self.id,
            username: self.username.clone(),
            email: None,
            is_admin: self.is_admin,
        }
    }
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Track {
    pub id: i64,
    pub file_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album_artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_number: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disc_number: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_cover: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub play_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lyrics: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist_img_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}
