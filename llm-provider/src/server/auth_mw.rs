//! The single auth boundary for `/v1/*`. When `auth.trust_loopback` is true (default),
//! loopback (127.0.0.1/::1) is trusted without a key; everyone else needs a minted key. Set
//! `trust_loopback = false` when the gateway is exposed (e.g. behind a reverse SSH tunnel,
//! which makes remote traffic appear as loopback) so a key is required from everyone.

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
    let loopback_ok = app.cfg.auth.trust_loopback && identity::is_loopback(peer.ip());
    if loopback_ok || app.keys.verify(req.headers()) {
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
