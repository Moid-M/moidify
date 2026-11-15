//! Subsonic authentication — same branch order and messages as Python
//! `_authenticate` / `_require_auth` in `subsonic.py`:
//!
//! 1. `u` required; per-username rate limit (`subsonic:{u}`, 30 / 15 min).
//! 2. `p=enc:<hex>` → decoded to the plaintext password first.
//! 3. `t` + `s` → `md5(password + salt)` against the Fernet-decrypted copy.
//! 4. `token` + `s` → same token form under a legacy parameter name.
//! 5. `p` plaintext → PBKDF2 verify, backfilling the encrypted copy when
//!    missing so subsequent token auth works.
//!
//! Divergences from Python (documented in `mod.rs`): the rate limiter counts
//! **failed** attempts only — Python's `check_rate_limit` records every
//! request, so any real client hits error 40 after 30 calls per 15 minutes.

use super::response::{error, render};
use super::Params;
use crate::auth::password::verify_password_async;
use crate::auth::store_subsonic_password;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use std::time::Duration;

const RATE_MAX: usize = 30;
const RATE_WINDOW: Duration = Duration::from_secs(900);

/// Authenticated Subsonic caller (only what handlers need).
#[derive(Debug, Clone)]
pub struct SubUser {
    pub id: i64,
    pub username: String,
}

/// `_require_auth`: authenticate or render `<error code="40">`.
pub async fn require(app: &AppState, q: &Params) -> Result<SubUser, axum::response::Response> {
    match authenticate(app, q).await {
        Ok(Some(user)) => Ok(user),
        Ok(None) => Err(render(
            &q.format(),
            &error(40, "Wrong username or password"),
        )),
        Err(e) => Err(e.into()),
    }
}

/// `_authenticate`: `None` = auth failed (rate limit, bad credentials, …);
/// `Err` only for infrastructure failures (surfaced as HTTP 500 like Python).
async fn authenticate(app: &AppState, q: &Params) -> AppResult<Option<SubUser>> {
    let u = q.u.as_deref().unwrap_or("");
    if u.is_empty() {
        return Ok(None);
    }

    let key = format!("subsonic:{u}");
    if app.rate.is_limited(&key, RATE_MAX, RATE_WINDOW) {
        return Ok(None);
    }

    let result = try_auth(app, q, u).await;
    if !matches!(&result, Ok(Some(_))) {
        app.rate.record(&key, RATE_WINDOW);
    }
    result
}

async fn try_auth(app: &AppState, q: &Params, u: &str) -> AppResult<Option<SubUser>> {
    // Legacy hex-encoded password: enc:<hex(password)>.
    let plaintext = match q.p.as_deref() {
        Some(p) if p.starts_with("enc:") => {
            let bytes = match hex::decode(&p[4..]) {
                Ok(b) => b,
                Err(_) => return Ok(None),
            };
            match String::from_utf8(bytes) {
                Ok(s) => s,
                Err(_) => return Ok(None),
            }
        }
        Some(p) => p.to_string(),
        None => String::new(),
    };

    let row = sqlx::query_as::<_, (i64, String, String, String, Option<String>)>(
        "SELECT id, username, password_hash, salt, password_encrypted \
         FROM users WHERE username = ?",
    )
    .bind(u)
    .fetch_optional(&app.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let Some((id, username, password_hash, salt, password_encrypted)) = row else {
        return Ok(None);
    };

    // Token auth (Subsonic >= 1.13): t = md5(password + salt), verified
    // against the encrypted copy kept specifically for Subsonic clients.
    let t = q.t.as_deref().unwrap_or("");
    let s = q.s.as_deref().unwrap_or("");
    if !t.is_empty() && !s.is_empty() {
        return token_match(app, t, s, &password_encrypted, id, username).await;
    }

    // Legacy parameter name for the same token form.
    let token = q.token.as_deref().unwrap_or("");
    if !token.is_empty() && !s.is_empty() {
        return token_match(app, token, s, &password_encrypted, id, username).await;
    }

    // Plaintext / hex password auth — also backfills the encrypted copy so
    // subsequent token auth works for this user.
    if !plaintext.is_empty() {
        let ok = verify_password_async(plaintext.clone(), salt, password_hash).await;
        if ok {
            if password_encrypted.is_none() {
                store_subsonic_password(&app.pool, &app.fernet, id, &plaintext).await?;
            }
            return Ok(Some(SubUser { id, username }));
        }
    }

    Ok(None)
}

#[allow(clippy::too_many_arguments)]
async fn token_match(
    app: &AppState,
    presented: &str,
    salt: &str,
    password_encrypted: &Option<String>,
    id: i64,
    username: String,
) -> AppResult<Option<SubUser>> {
    let Some(enc) = password_encrypted else {
        return Ok(None);
    };
    let Ok(raw) = app.fernet.decrypt(enc.as_bytes()) else {
        return Ok(None);
    };
    let Ok(plaintext) = String::from_utf8(raw) else {
        return Ok(None);
    };
    let expected = super::ids::md5_hex(&format!("{plaintext}{salt}"));
    // hmac.compare_digest(presented.lower(), expected)
    if constant_time_eq(&presented.to_lowercase(), &expected) {
        return Ok(Some(SubUser { id, username }));
    }
    Ok(None)
}

/// Timing-safe string comparison (Python `hmac.compare_digest`).
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_cases() {
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abd"));
        assert!(!constant_time_eq("abc", "abcd"));
        assert!(constant_time_eq("", ""));
    }
}
