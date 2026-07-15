//! Provider implementations. `local` + `grok` share `PassthroughProvider`; `claude` is the
//! only translator.

mod claude;
mod passthrough;

pub use claude::ClaudeProvider;
pub use passthrough::PassthroughProvider;

use axum::body::Body;
use axum::http::StatusCode;
use axum::response::Response;
use futures_util::TryStreamExt;
use serde_json::Value;

/// Forward a chat request to an OpenAI-native upstream, attaching a bearer if present.
/// Streams SSE straight through on success + `stream:true`; otherwise buffers the body.
pub async fn passthrough(
    http: &reqwest::Client,
    base: &str,
    body: Value,
    bearer: Option<String>,
) -> anyhow::Result<Response> {
    let stream_requested = body["stream"].as_bool().unwrap_or(false);
    let mut req = http.post(format!("{base}/chat/completions")).json(&body);
    if let Some(tok) = bearer {
        req = req.bearer_auth(tok);
    }
    let resp = req.send().await?;
    let status = StatusCode::from_u16(resp.status().as_u16())?;

    if !status.is_success() || !stream_requested {
        let bytes = resp.bytes().await?;
        return Ok(Response::builder()
            .status(status)
            .header("content-type", "application/json")
            .body(Body::from(bytes))?);
    }
    let stream = resp.bytes_stream().map_err(std::io::Error::other);
    Ok(Response::builder()
        .status(status)
        .header("content-type", "text/event-stream")
        .body(Body::from_stream(stream))?)
}
