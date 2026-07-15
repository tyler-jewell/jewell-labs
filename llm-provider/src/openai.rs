//! Small helpers for emitting OpenAI-shaped responses.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// Generic OpenAI-shaped error body: `{"error":{"message","type"}}`.
pub fn error_response(status: StatusCode, msg: String) -> Response {
    (status, axum::Json(json!({"error": {"message": msg, "type": "api_error"}}))).into_response()
}

/// OpenAI `invalid_request_error` with a `code`, used for 4xx client errors.
pub fn client_error(status: StatusCode, code: &str, msg: String) -> Response {
    (
        status,
        axum::Json(json!({"error": {"message": msg, "type": "invalid_request_error", "code": code}})),
    )
        .into_response()
}
