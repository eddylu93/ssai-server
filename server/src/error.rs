use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("bad request: {0}")]
    BadRequest(&'static str),
    #[error("unauthorized: {0}")]
    Unauthorized(&'static str),
    #[error("service unavailable: {0}")]
    ServiceUnavailable(&'static str),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

#[derive(Debug, Serialize)]
struct ErrorBody<'a> {
    error: &'a str,
    message: &'a str,
}

impl AppError {
    pub fn oauth_state_invalid() -> Self {
        Self::Unauthorized("oauth_state_invalid")
    }

    pub fn oauth_state_expired() -> Self {
        Self::Unauthorized("oauth_state_expired")
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error, message) = match &self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, "bad_request", *message),
            Self::Unauthorized(message) => (StatusCode::UNAUTHORIZED, "unauthorized", *message),
            Self::ServiceUnavailable(message) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
                *message,
            ),
            Self::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_server_error",
                "internal_server_error",
            ),
        };

        if let Self::Internal(error) = &self {
            tracing::error!(error = %error, "request failed");
        }

        (status, Json(ErrorBody { error, message })).into_response()
    }
}
