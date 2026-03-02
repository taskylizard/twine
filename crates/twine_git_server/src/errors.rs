use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct ErrorEnvelope {
    error: &'static str,
    message: String,
}

pub(crate) fn api_error(
    status: StatusCode,
    error: &'static str,
    message: impl Into<String>,
) -> Response {
    (
        status,
        Json(ErrorEnvelope {
            error,
            message: message.into(),
        }),
    )
        .into_response()
}
