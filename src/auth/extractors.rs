//! Axum extractors for the custom `token` header.
//!
//! Using extractors means a protected handler *cannot compile* without
//! declaring its auth requirement — the Python "forgot `_require_admin`"
//! bug class is impossible.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::HeaderMap;

use crate::db::models::User;
use crate::error::AppError;
use crate::state::AppState;

/// Pull the raw `token` header (any case) if present.
fn extract_raw_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("token")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

/// Any authenticated user, or `None` if missing/invalid.
/// Handlers that work for anonymous *and* logged-in callers use this.
pub struct OptionalUser(pub Option<User>);

impl FromRequestParts<AppState> for OptionalUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let Some(raw) = extract_raw_token(&parts.headers) else {
            return Ok(OptionalUser(None));
        };
        let user = crate::auth::session::get_user_from_token(&state.pool, &raw)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        Ok(OptionalUser(user))
    }
}

/// Required authenticated user. 401 if missing/invalid.
pub struct AuthUser(pub User);

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let OptionalUser(user) = OptionalUser::from_request_parts(parts, state).await?;
        match user {
            Some(u) => Ok(AuthUser(u)),
            None => Err(AppError::Unauthorized("Not logged in".into())),
        }
    }
}

/// Required admin. 401 if unauthenticated, 403 if not admin.
/// Messages match Python `_require_admin` / `_get_user_from_token` callers.
pub struct AdminUser(pub User);

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let raw = extract_raw_token(&parts.headers);
        let user = match raw {
            Some(raw) => crate::auth::session::get_user_from_token(&state.pool, &raw)
                .await
                .map_err(|e| AppError::Internal(e.into()))?,
            None => None,
        };
        // Python `_require_admin`: 401 "Not authenticated" / 403 "Admin access required"
        let user = user.ok_or_else(|| AppError::Unauthorized("Not authenticated".into()))?;
        if user.is_admin == 0 {
            return Err(AppError::Forbidden("Admin access required".into()));
        }
        Ok(AdminUser(user))
    }
}

/// Raw token string if present (used by logout / change-password which need
/// the token itself, not just the user).
pub struct RawToken(pub Option<String>);

impl FromRequestParts<AppState> for RawToken {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(RawToken(extract_raw_token(&parts.headers)))
    }
}
