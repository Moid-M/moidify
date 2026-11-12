//! Session tokens — same contract as Python:
//! - raw token: `secrets.token_urlsafe(48)` (48 random bytes, base64url no pad)
//! - stored: SHA-256 hex in BOTH `sessions.token` and `sessions.token_hash`
//! - expiry: 30 days (`2592000` seconds)
//! - clients send the raw token in the HTTP header `token`

use crate::db::models::User;
use sqlx::SqlitePool;

pub const SESSION_TTL_SECS: i64 = 2_592_000; // 30 days

/// SHA-256 hex of a token (`_token_hash`).
pub fn token_hash(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

/// Generate a new raw session token (48 bytes → 64-char base64url).
pub fn generate_token() -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use rand::RngCore;
    let mut bytes = [0u8; 48];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Create a session row for `user_id` and return the raw token.
pub async fn create_session(pool: &SqlitePool, user_id: i64) -> Result<String, sqlx::Error> {
    let token = generate_token();
    let hash = token_hash(&token);
    let expires = chrono::Utc::now().timestamp() + SESSION_TTL_SECS;
    sqlx::query(
        "INSERT INTO sessions (token, user_id, expires_at, token_hash) VALUES (?, ?, ?, ?)",
    )
    .bind(&hash)
    .bind(user_id)
    .bind(expires)
    .bind(&hash)
    .execute(pool)
    .await?;
    Ok(token)
}

/// Resolve a raw token to a user. Prunes expired sessions first (as Python did).
/// Returns `None` for missing/invalid/expired tokens.
pub async fn get_user_from_token(
    pool: &SqlitePool,
    token: &str,
) -> Result<Option<User>, sqlx::Error> {
    let now = chrono::Utc::now().timestamp();
    sqlx::query("DELETE FROM sessions WHERE expires_at IS NOT NULL AND expires_at < ?")
        .bind(now)
        .execute(pool)
        .await?;

    let hash = token_hash(token);
    let row = sqlx::query_as::<_, (i64, String, Option<String>, i64)>(
        r#"SELECT u.id, u.username, u.email, u.is_admin
           FROM sessions s JOIN users u ON s.user_id = u.id
           WHERE s.token_hash = ? AND (s.expires_at IS NULL OR s.expires_at > ?)"#,
    )
    .bind(&hash)
    .bind(now)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|(id, username, email, is_admin)| User {
        id,
        username,
        email,
        is_admin,
    }))
}

/// Delete one session (logout).
pub async fn delete_session(pool: &SqlitePool, token: &str) -> Result<(), sqlx::Error> {
    let hash = token_hash(token);
    sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
        .bind(&hash)
        .execute(pool)
        .await?;
    Ok(())
}

/// Delete all sessions for a user (logout-all).
pub async fn delete_all_sessions(pool: &SqlitePool, user_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Delete every session except `keep_token` (used after password change).
pub async fn delete_other_sessions(
    pool: &SqlitePool,
    user_id: i64,
    keep_token: &str,
) -> Result<(), sqlx::Error> {
    let keep = token_hash(keep_token);
    sqlx::query("DELETE FROM sessions WHERE user_id = ? AND token_hash != ?")
        .bind(user_id)
        .bind(&keep)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_hash_is_sha256_hex() {
        let h = token_hash("abc");
        assert_eq!(h.len(), 64);
        // Known SHA-256("abc")
        assert_eq!(
            h,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn generated_tokens_are_64_chars() {
        let t = generate_token();
        assert_eq!(t.len(), 64);
        assert_ne!(generate_token(), generate_token());
    }
}
