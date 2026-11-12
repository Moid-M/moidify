use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// Application-wide error type. Maps to the same status codes and
/// `{ "detail": ... }` JSON shape the Python backend (and frontend) expect.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),

    #[error("{0}")]
    Unauthorized(String),

    #[error("{0}")]
    Forbidden(String),

    #[error("{0}")]
    NotFound(String),

    #[error("{0}")]
    Conflict(String),

    #[error("{0}")]
    PayloadTooLarge(String),

    #[error("{0}")]
    UnsupportedMediaType(String),

    #[error("{0}")]
    TooManyRequests(String),

    #[error("Internal server error")]
    Internal(#[from] anyhow::Error),
}

impl AppError {
    pub fn status(&self) -> StatusCode {
        match self {
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            AppError::Forbidden(_) => StatusCode::FORBIDDEN,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::PayloadTooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,
            AppError::UnsupportedMediaType(_) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            AppError::TooManyRequests(_) => StatusCode::TOO_MANY_REQUESTS,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn detail(&self) -> String {
        match self {
            // Include source in debug builds so tests can see the cause.
            AppError::Internal(e) => {
                if cfg!(debug_assertions) {
                    format!("Internal server error: {e:#}")
                } else {
                    "Internal server error".to_string()
                }
            }
            other => other.to_string(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        let detail = self.detail();
        if status == StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!(error = %self, "internal error");
        }
        (status, Json(json!({ "detail": detail }))).into_response()
    }
}

/// Convenience alias used across route handlers.
pub type AppResult<T> = Result<T, AppError>;
