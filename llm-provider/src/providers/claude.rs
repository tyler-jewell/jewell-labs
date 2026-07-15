//! Claude provider: OpenAI chat format <-> Anthropic Messages API, both directions,
//! streaming and not, including tool calls. Hand-rolled against api.anthropic.com since
//! there is no official Rust SDK.
//!
//! ponytail: translation works on serde_json::Value for parity with the tested reference;
//! typed serde structs are the upgrade path if this layer grows.

use async_trait::async_trait;
use axum::body::Body;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{json, Map, Value};
use tokio_stream::wrappers::ReceiverStream;

use crate::config::ClaudeConfig;
use crate::openai::error_response;
use crate::registry::Provider;
use crate::token::TokenSource;
use crate::CLAUDE_CODE_SYSTEM;

const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1/messages";

pub struct ClaudeProvider {
    prefix: String,
    models: Vec<String>,
    token: TokenSource,
}

impl ClaudeProvider {
    pub fn new(c: &ClaudeConfig) -> Self {
        ClaudeProvider {
            prefix: c.prefix.clone(),
            models: c.models.clone(),
            token: TokenSource::keychain(),
        }
    }
}

#[async_trait]
impl Provider for ClaudeProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn matches(&self, model: &str) -> bool {
        model.starts_with(&self.prefix)
    }

    async fn models(&self, _http: &reqwest::Client) -> anyhow::Result<Vec<Value>> {
        Ok(self
            .models
            .iter()
            .map(|m| {
                let ctx = if m.contains("haiku") { 200_000 } else { 1_000_000 };
                json!({"id": m, "object": "model", "owned_by": "anthropic", "context_length": ctx})
            })
            .collect())
    }

    async fn chat(&self, http: &reqwest::Client, body: Value) -> anyhow::Result<Response> {
        let tok = self
            .token
            .bearer(http)
            .await?
            .ok_or_else(|| anyhow::anyhow!("no claude token"))?;
        claude_chat(http, &tok, body).await
    }
}

fn text_of(content: &Value) -> String {
    match content {
        Value::Array(parts) => parts
            .iter()
            .filter(|p| p["type"].as_str() == Some("text"))
            .filter_map(|p| p["text"].as_str())
            .collect(),
        Value::String(s) => s.clone(),
        _ => String::new(),
    }
}

fn finish_reason(stop: &str) -> &'static str {
    match stop {
        "max_tokens" => "length",
        "tool_use" => "tool_calls",
        "refusal" => "content_filter",
        _ => "stop", // end_turn, stop_sequence
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// OpenAI chat request -> Anthropic Messages request.
pub fn to_anthropic(body: &Value) -> Value {
    let mut system = vec![json!({"type": "text", "text": CLAUDE_CODE_SYSTEM})]; // required first block for Claude Code OAuth tokens
    let mut msgs: Vec<Value> = vec![];

    let push = |msgs: &mut Vec<Value>, role: &str, blocks: Vec<Value>| {
        if let Some(last) = msgs.last_mut() {
            if last["role"].as_str() == Some(role) {
                last["content"].as_array_mut().unwrap().extend(blocks);
                return;
            }
        }
        msgs.push(json!({"role": role, "content": blocks}));
    };

    for m in body["messages"].as_array().unwrap_or(&vec![]) {
        let content = &m["content"];
        match m["role"].as_str().unwrap_or("") {
            "system" | "developer" => {
                let text = text_of(content);
                if !text.is_empty() {
                    system.push(json!({"type": "text", "text": text}));
                }
            }
            "tool" => push(
                &mut msgs,
                "user",
                vec![json!({"type": "tool_result",
                            "tool_use_id": m["tool_call_id"].as_str().unwrap_or(""),
                            "content": text_of(content)})],
            ),
            "assistant" => {
                let mut blocks = vec![];
                let text = text_of(content);
                if !text.is_empty() {
                    blocks.push(json!({"type": "text", "text": text}));
                }
                for tc in m["tool_calls"].as_array().unwrap_or(&vec![]) {
                    let args = tc["function"]["arguments"].as_str().unwrap_or("{}");
                    let input: Value = serde_json::from_str(args).unwrap_or_else(|_| json!({}));
                    blocks.push(json!({"type": "tool_use", "id": tc["id"],
                                       "name": tc["function"]["name"], "input": input}));
                }
                if !blocks.is_empty() {
                    push(&mut msgs, "assistant", blocks);
                }
            }
            _ => {
                // user (ponytail: text parts only; image parts dropped, same as reference)
                let text = text_of(content);
                let text = if text.is_empty() { " ".to_string() } else { text };
                push(&mut msgs, "user", vec![json!({"type": "text", "text": text})]);
            }
        }
    }

    // clamp to the model's output ceiling: clients (e.g. hermes) send blanket 65536
    // defaults that Anthropic rejects on models with lower caps
    let model_name = body["model"].as_str().unwrap_or("");
    let output_cap = if model_name.contains("haiku") { 64_000 } else { 128_000 };
    let max_tokens = body["max_tokens"]
        .as_u64()
        .or(body["max_completion_tokens"].as_u64())
        .unwrap_or(8192)
        .min(output_cap);
    let mut req = Map::new();
    req.insert("model".into(), body["model"].clone());
    req.insert("system".into(), json!(system));
    req.insert("messages".into(), json!(msgs));
    req.insert("max_tokens".into(), json!(max_tokens));
    // sampling params intentionally dropped: current Claude models reject temperature/top_p/top_k

    match &body["stop"] {
        Value::String(s) => {
            req.insert("stop_sequences".into(), json!([s]));
        }
        Value::Array(a) => {
            req.insert("stop_sequences".into(), json!(a));
        }
        _ => {}
    }
    let tools: Vec<Value> = body["tools"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter(|t| t["type"].as_str() == Some("function"))
        .map(|t| {
            let f = &t["function"];
            let schema = if f["parameters"].is_object() {
                f["parameters"].clone()
            } else {
                json!({"type": "object", "properties": {}})
            };
            json!({"name": f["name"], "description": f["description"].as_str().unwrap_or(""),
                   "input_schema": schema})
        })
        .collect();
    if !tools.is_empty() {
        req.insert("tools".into(), json!(tools));
    }
    match &body["tool_choice"] {
        Value::String(s) if s == "none" => {
            req.insert("tool_choice".into(), json!({"type": "none"}));
        }
        Value::String(s) if s == "required" => {
            req.insert("tool_choice".into(), json!({"type": "any"}));
        }
        Value::Object(o) if o.get("type").and_then(Value::as_str) == Some("function") => {
            req.insert("tool_choice".into(), json!({"type": "tool", "name": o["function"]["name"]}));
        }
        _ => {}
    }
    Value::Object(req)
}

/// Anthropic non-streaming response -> OpenAI chat completion.
pub fn from_anthropic(msg: &Value, model: &str) -> Value {
    let empty = vec![];
    let content = msg["content"].as_array().unwrap_or(&empty);
    let text: String = content
        .iter()
        .filter(|b| b["type"].as_str() == Some("text"))
        .filter_map(|b| b["text"].as_str())
        .collect();
    let tool_calls: Vec<Value> = content
        .iter()
        .filter(|b| b["type"].as_str() == Some("tool_use"))
        .map(|b| {
            json!({"id": b["id"], "type": "function",
                   "function": {"name": b["name"], "arguments": b["input"].to_string()}})
        })
        .collect();
    let mut message =
        json!({"role": "assistant", "content": if text.is_empty() { Value::Null } else { json!(text) }});
    if !tool_calls.is_empty() {
        message["tool_calls"] = json!(tool_calls);
    }
    let (input_tokens, output_tokens) = (
        msg["usage"]["input_tokens"].as_u64().unwrap_or(0),
        msg["usage"]["output_tokens"].as_u64().unwrap_or(0),
    );
    json!({
        "id": msg["id"], "object": "chat.completion", "created": unix_now(), "model": model,
        "choices": [{"index": 0, "message": message,
                     "finish_reason": finish_reason(msg["stop_reason"].as_str().unwrap_or(""))}],
        "usage": {"prompt_tokens": input_tokens, "completion_tokens": output_tokens,
                  "total_tokens": input_tokens + output_tokens}
    })
}

async fn claude_chat(http: &reqwest::Client, tok: &str, body: Value) -> anyhow::Result<Response> {
    let model = body["model"].as_str().unwrap_or("").to_string();
    let stream_requested = body["stream"].as_bool().unwrap_or(false);
    let mut areq = to_anthropic(&body);
    if stream_requested {
        areq["stream"] = json!(true);
    }
    let resp = http
        .post(ANTHROPIC_URL)
        .bearer_auth(tok)
        .header("anthropic-version", "2023-06-01")
        .header("anthropic-beta", "oauth-2025-04-20")
        .json(&areq)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = StatusCode::from_u16(resp.status().as_u16())?;
        let text = resp.text().await.unwrap_or_default();
        let msg = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| v["error"]["message"].as_str().map(String::from))
            .unwrap_or(text);
        return Ok(error_response(status, msg));
    }
    if !stream_requested {
        let msg: Value = resp.json().await?;
        return Ok(axum::Json(from_anthropic(&msg, &model)).into_response());
    }

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<String, std::io::Error>>(64);
    tokio::spawn(claude_sse(resp, model, tx));
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/event-stream")
        .body(Body::from_stream(ReceiverStream::new(rx)))?)
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|w| w == needle)
}

async fn claude_sse(
    resp: reqwest::Response,
    model: String,
    tx: tokio::sync::mpsc::Sender<Result<String, std::io::Error>>,
) {
    use futures_util::StreamExt;
    let cid = format!("chatcmpl-{:x}", rand::random::<u64>());
    let chunk = |delta: Value, finish: Option<&str>| -> String {
        format!(
            "data: {}\n\n",
            json!({"id": cid, "object": "chat.completion.chunk", "created": unix_now(),
                   "model": model, "choices": [{"index": 0, "delta": delta, "finish_reason": finish}]})
        )
    };

    let _ = tx.send(Ok(chunk(json!({"role": "assistant"}), None))).await;
    // Buffer BYTES and split on b"\n\n"; decode only complete frames so a multibyte UTF-8
    // codepoint straddling two TCP chunks is never corrupted into replacement chars.
    let mut buf: Vec<u8> = Vec::new();
    let mut tool_idx: i64 = -1;
    let mut finish = "stop";
    let mut stream = resp.bytes_stream();

    while let Some(item) = stream.next().await {
        let bytes = match item {
            Ok(b) => b,
            Err(e) => {
                let _ = tx
                    .send(Ok(format!(
                        "data: {}\n\n",
                        json!({"error": {"message": e.to_string(), "type": "api_error"}})
                    )))
                    .await;
                break;
            }
        };
        buf.extend_from_slice(&bytes);
        while let Some(pos) = find_subsequence(&buf, b"\n\n") {
            let frame: Vec<u8> = buf.drain(..pos + 2).collect();
            let frame = String::from_utf8_lossy(&frame[..frame.len() - 2]);
            for line in frame.lines() {
                let Some(data) = line.strip_prefix("data: ") else {
                    continue;
                };
                let Ok(ev) = serde_json::from_str::<Value>(data) else {
                    continue;
                };
                match ev["type"].as_str().unwrap_or("") {
                    "content_block_start"
                        if ev["content_block"]["type"].as_str() == Some("tool_use") =>
                    {
                        tool_idx += 1;
                        let cb = &ev["content_block"];
                        let _ = tx
                            .send(Ok(chunk(
                                json!({"tool_calls": [{"index": tool_idx, "id": cb["id"], "type": "function",
                                       "function": {"name": cb["name"], "arguments": ""}}]}),
                                None,
                            )))
                            .await;
                    }
                    "content_block_delta" => match ev["delta"]["type"].as_str().unwrap_or("") {
                        "text_delta" => {
                            if let Some(t) = ev["delta"]["text"].as_str() {
                                if !t.is_empty() {
                                    let _ = tx.send(Ok(chunk(json!({"content": t}), None))).await;
                                }
                            }
                        }
                        "input_json_delta" => {
                            if let Some(p) = ev["delta"]["partial_json"].as_str() {
                                if !p.is_empty() {
                                    let _ = tx
                                        .send(Ok(chunk(
                                            json!({"tool_calls": [{"index": tool_idx, "function": {"arguments": p}}]}),
                                            None,
                                        )))
                                        .await;
                                }
                            }
                        }
                        _ => {}
                    },
                    "message_delta" => {
                        if let Some(sr) = ev["delta"]["stop_reason"].as_str() {
                            finish = finish_reason(sr);
                        }
                    }
                    "error" => {
                        let _ = tx
                            .send(Ok(format!("data: {}\n\n", json!({"error": ev["error"]}))))
                            .await;
                    }
                    _ => {} // message_start, ping, content_block_stop, message_stop
                }
            }
        }
        // Client gone (receiver dropped): stop draining the upstream promptly.
        if tx.is_closed() {
            return;
        }
        // Cap the frame buffer: a never-terminating upstream would otherwise grow it
        // unbounded. Surface an error and stop rather than silently dropping a partial frame.
        if buf.len() > 1_000_000 {
            let _ = tx
                .send(Ok(format!(
                    "data: {}\n\n",
                    json!({"error": {"message": "upstream frame exceeded buffer cap", "type": "api_error"}})
                )))
                .await;
            break;
        }
    }
    let _ = tx.send(Ok(chunk(json!({}), Some(finish)))).await;
    let _ = tx.send(Ok("data: [DONE]\n\n".to_string())).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_request() {
        let req = to_anthropic(&json!({
            "model": "claude-opus-4-8",
            "temperature": 0.7,
            "messages": [
                {"role": "system", "content": "Be terse."},
                {"role": "user", "content": "hi"},
                {"role": "assistant", "content": null,
                 "tool_calls": [{"id": "t1", "type": "function",
                                 "function": {"name": "f", "arguments": "{\"x\":1}"}}]},
                {"role": "tool", "tool_call_id": "t1", "content": "42"},
                {"role": "tool", "tool_call_id": "t1b", "content": "43"},
                {"role": "user", "content": [{"type": "text", "text": "and?"}]},
            ],
            "tools": [{"type": "function", "function": {"name": "f", "parameters": {"type": "object", "properties": {}}}}],
            "stop": "END",
        }));
        assert!(req["system"][0]["text"].as_str().unwrap().starts_with("You are Claude Code"));
        assert_eq!(req["system"][1]["text"], "Be terse.");
        assert!(req.get("temperature").is_none());
        assert_eq!(req["stop_sequences"], json!(["END"]));
        let roles: Vec<&str> = req["messages"].as_array().unwrap().iter()
            .map(|m| m["role"].as_str().unwrap()).collect();
        assert_eq!(roles, ["user", "assistant", "user"]); // tool results + next user merged
        let last = req["messages"][2]["content"].as_array().unwrap();
        let kinds: Vec<&str> = last.iter().map(|b| b["type"].as_str().unwrap()).collect();
        assert_eq!(kinds, ["tool_result", "tool_result", "text"]);
        assert_eq!(req["messages"][1]["content"][0],
                   json!({"type": "tool_use", "id": "t1", "name": "f", "input": {"x": 1}}));
    }

    #[test]
    fn translates_response() {
        let out = from_anthropic(
            &json!({"id": "msg_1", "stop_reason": "tool_use",
                    "content": [{"type": "text", "text": "calling"},
                                {"type": "tool_use", "id": "t1", "name": "f", "input": {"x": 1}}],
                    "usage": {"input_tokens": 10, "output_tokens": 5}}),
            "claude-opus-4-8",
        );
        assert_eq!(out["choices"][0]["finish_reason"], "tool_calls");
        assert_eq!(out["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"], "{\"x\":1}");
        assert_eq!(out["usage"]["total_tokens"], 15);
    }

    #[test]
    fn clamps_max_tokens_to_output_cap() {
        let base = json!({"messages": [{"role": "user", "content": "hi"}]});
        let mut haiku = base.clone();
        haiku["model"] = json!("claude-haiku-4-5");
        haiku["max_tokens"] = json!(65536);
        assert_eq!(to_anthropic(&haiku)["max_tokens"], 64_000);
        let mut opus = base.clone();
        opus["model"] = json!("claude-opus-4-8");
        opus["max_tokens"] = json!(65536);
        assert_eq!(to_anthropic(&opus)["max_tokens"], 65_536);
    }

    #[test]
    fn finish_reasons_map() {
        assert_eq!(finish_reason("end_turn"), "stop");
        assert_eq!(finish_reason("max_tokens"), "length");
        assert_eq!(finish_reason("tool_use"), "tool_calls");
        assert_eq!(finish_reason("refusal"), "content_filter");
    }

    #[test]
    fn sse_framing_handles_split_utf8() {
        // A 4-byte emoji split across two byte chunks must survive intact once framed.
        let full = "😀".as_bytes();
        let mut buf: Vec<u8> = Vec::new();
        buf.extend_from_slice(&full[..2]);
        assert!(find_subsequence(&buf, b"\n\n").is_none()); // no complete frame yet
        buf.extend_from_slice(&full[2..]);
        buf.extend_from_slice(b"\n\n");
        let pos = find_subsequence(&buf, b"\n\n").unwrap();
        let frame: Vec<u8> = buf.drain(..pos + 2).collect();
        let decoded = String::from_utf8_lossy(&frame[..frame.len() - 2]);
        assert_eq!(decoded, "😀"); // no replacement chars
    }
}
