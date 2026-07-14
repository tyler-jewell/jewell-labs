//! Streaming chat endpoint (SSE).

use super::state::AppState;
use crate::{
    certify_agent_markdown, complete_chat, extract_tool_call, invoke_tool_call, load_agent,
    system_with_tools, ChatEndpoint, ChatMessage, ChatRequest, ModelRegistry, MAX_TOOL_ROUNDS,
};
use axum::body::Body;
use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures_util::stream::{self, StreamExt};
use serde_json::{json, Value};
use std::convert::Infallible;
use std::sync::Arc;

pub async fn chat_stream(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChatRequest>,
) -> Response {
    let agent = match load_agent(&state.agents_dir, &req.agent_stem) {
        Ok(a) => a,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    let cert = certify_agent_markdown(&std::fs::read_to_string(&agent.path).unwrap_or_default());
    if !cert.ok {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "agent failed schema certification",
                "certification": cert,
            })),
        )
            .into_response();
    }

    let registry = match ModelRegistry::load(&state.registry_path) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("registry: {e}") })),
            )
                .into_response();
        }
    };

    let model = match registry.resolve(&agent.frontmatter.default_model) {
        Ok(m) => m,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    // Eval pins: require JEWELL_ALLOW_EVAL_PINS=1 + loopback URL + evals/runs fs root.
    let has_pin = req
        .eval_base_url
        .as_ref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
        || req
            .eval_model
            .as_ref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
        || req
            .eval_fs_root
            .as_ref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
        || req.eval_temperature.is_some();
    if let Err(e) = crate::eval_guards::require_eval_pins_enabled(has_pin) {
        return (StatusCode::FORBIDDEN, Json(json!({ "error": e }))).into_response();
    }
    let eval_base = match crate::eval_guards::sanitize_eval_base_url(req.eval_base_url.as_deref()) {
        Ok(v) => v,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response();
        }
    };
    let eval_model = match crate::eval_guards::sanitize_eval_model(req.eval_model.as_deref()) {
        Ok(v) => v,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response();
        }
    };
    let sandbox = match crate::eval_guards::sanitize_eval_fs_root(
        req.eval_fs_root.as_deref(),
        &state.repo_root,
    ) {
        Ok(v) => v,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response();
        }
    };
    let eval_temp = match crate::eval_guards::sanitize_eval_temperature(req.eval_temperature) {
        Ok(v) => v,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response();
        }
    };

    let mut endpoint = ChatEndpoint::from_agent(&agent, &model)
        .with_eval_pin(eval_base.as_deref(), eval_model.as_deref());
    if let Some(t) = eval_temp {
        let max_tokens = endpoint.max_tokens;
        endpoint = endpoint.with_sampling(t, max_tokens);
    }
    let allowed = agent.frontmatter.tools.clone();
    let system = system_with_tools(&agent.body, &allowed);
    let history = req.history.clone();
    let user = req.message.clone();
    let tool_ctx = state
        .tool_ctx(Some(agent.id.clone()), allowed)
        .with_sandbox_override(sandbox);
    let presence = state.presence.clone();
    let agent_id = agent.id.clone();
    presence.set_busy(&agent_id, None);

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Result<Value, String>>();
    tokio::spawn(async move {
        let _ = tx.send(Ok(json!({ "status": "busy" })));
        let result =
            chat_with_tools(&endpoint, &system, &history, &user, &tool_ctx, tx.clone()).await;
        if let Err(e) = result {
            let _ = tx.send(Err(e));
        }
        let _ = tx.send(Ok(json!({ "status": "idle" })));
        presence.set_idle(&agent_id);
    });

    let stream = stream::unfold(rx, |mut rx| async move {
        match rx.recv().await {
            Some(Ok(payload)) => Some((Ok::<_, Infallible>(format!("data: {payload}\n\n")), rx)),
            Some(Err(err)) => {
                let payload = json!({ "error": err });
                Some((Ok::<_, Infallible>(format!("data: {payload}\n\n")), rx))
            }
            None => None,
        }
    });
    let done = stream::once(async { Ok::<_, Infallible>("data: [DONE]\n\n".to_string()) });
    let body = Body::from_stream(stream.chain(done));

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(body)
        .unwrap()
}

async fn chat_with_tools(
    endpoint: &ChatEndpoint,
    system: &str,
    history: &[ChatMessage],
    user: &str,
    tool_ctx: &crate::ToolContext,
    tx: tokio::sync::mpsc::UnboundedSender<Result<Value, String>>,
) -> Result<(), String> {
    let mut working_history: Vec<ChatMessage> = history.to_vec();
    let mut next_user = user.to_string();

    for round in 0..MAX_TOOL_ROUNDS {
        let mut full = String::new();
        stream_to_channel(endpoint, system, &working_history, &next_user, |delta| {
            full.push_str(delta);
            let _ = tx.send(Ok(json!({ "delta": delta, "round": round })));
        })
        .await
        .map_err(|e| e.to_string())?;

        if let Some(call) = extract_tool_call(&full) {
            let _ = tx.send(Ok(json!({
                "tool_call": { "name": call.name, "arguments": call.arguments, "round": round }
            })));
            let result = invoke_tool_call(tool_ctx, &call);
            let _ = tx.send(Ok(json!({
                "tool_result": {
                    "name": result.name, "ok": result.ok, "result": result.result, "round": round
                }
            })));
            working_history.push(ChatMessage {
                role: "user".into(),
                content: next_user.clone(),
            });
            working_history.push(ChatMessage {
                role: "assistant".into(),
                content: full,
            });
            // Mechanical payload only — multi-step policy lives in the agent system prompt.
            next_user = format!(
                "tool_result for {}:\n{}",
                result.name,
                serde_json::to_string_pretty(&result.result).unwrap_or_else(|_| "{}".into())
            );
            continue;
        }

        if full.is_empty() {
            let _ = tx.send(Ok(json!({ "delta": "(empty response)" })));
        }
        return Ok(());
    }
    let _ = tx.send(Ok(json!({
        "error": format!("tool loop exceeded {MAX_TOOL_ROUNDS} rounds")
    })));
    Ok(())
}

async fn stream_to_channel<F>(
    endpoint: &ChatEndpoint,
    system: &str,
    history: &[ChatMessage],
    user: &str,
    mut on_delta: F,
) -> Result<(), crate::ChatError>
where
    F: FnMut(&str),
{
    let mut messages = vec![json!({ "role": "system", "content": system })];
    for h in history {
        messages.push(json!({ "role": h.role, "content": h.content }));
    }
    messages.push(json!({ "role": "user", "content": user }));

    let body = json!({
        "model": endpoint.model_alias,
        "messages": messages,
        "temperature": endpoint.temperature,
        "max_tokens": endpoint.max_tokens,
        "stream": true,
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/v1/chat/completions", endpoint.base_url))
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let t = resp.text().await.unwrap_or_default();
        match complete_chat(endpoint, system, history, user).await {
            Ok(text) => {
                on_delta(&text);
                return Ok(());
            }
            Err(_) => return Err(crate::ChatError::Server(t)),
        }
    }

    let mut full = String::new();
    let mut byte_stream = resp.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = byte_stream.next().await {
        let chunk = chunk?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim_end_matches('\r').to_string();
            buffer = buffer[pos + 1..].to_string();
            if line.is_empty() || line.starts_with(':') {
                continue;
            }
            let data = line.strip_prefix("data: ").unwrap_or(&line);
            if data.trim() == "[DONE]" {
                if full.is_empty() {
                    if let Ok(text) = complete_chat(endpoint, system, history, user).await {
                        on_delta(&text);
                    }
                }
                return Ok(());
            }
            if let Ok(v) = serde_json::from_str::<Value>(data) {
                let delta = v["choices"][0]["delta"]["content"]
                    .as_str()
                    .or_else(|| v["choices"][0]["delta"]["reasoning_content"].as_str())
                    .unwrap_or("");
                if !delta.is_empty() {
                    full.push_str(delta);
                    on_delta(delta);
                }
            }
        }
    }

    if full.is_empty() {
        let text = complete_chat(endpoint, system, history, user).await?;
        on_delta(&text);
    }
    Ok(())
}
