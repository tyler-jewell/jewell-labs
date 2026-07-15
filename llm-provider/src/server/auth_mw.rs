//! The single auth boundary for `/v1/*`: loopback is trusted, remote needs a valid key.
//! The decision itself lives in `identity::is_authorized` (pure, unit-tested).

use std::net::SocketAddr;

use axum::extract::{ConnectInfo, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::app::AppState;
use crate::identity;

pub async fn gate(
    State(app): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Response {
    if identity::is_loopback(peer.ip()) || app.keys.verify(req.headers()) {
        return next.run(req).await;
    }
    (
        StatusCode::UNAUTHORIZED,
        axum::Json(json!({"error": {
            "message": format!("Invalid or missing API key. Sign in at {}/login to get one.", app.cfg.public_url),
            "type": "invalid_request_error", "code": "invalid_api_key"}})),
    )
        .into_response()
}
