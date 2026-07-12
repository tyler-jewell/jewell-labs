//! Streaming chat client for llama-server OpenAI-compatible API.

use crate::registry::ResolvedModel;
use crate::schema::{AgentDocument, SamplingConfig, ServerConfig};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChatError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("server error: {0}")]
    Server(String),
    #[error("empty response")]
    Empty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub agent_stem: String,
    pub message: String,
    #[serde(default)]
    pub history: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct ChatEndpoint {
    pub base_url: String,
    pub model_alias: String,
    pub temperature: f64,
    pub max_tokens: u32,
}

impl ChatEndpoint {
    /// Eval-friendly sampling overrides.
    pub fn with_sampling(mut self, temperature: f64, max_tokens: u32) -> Self {
        self.temperature = temperature;
        self.max_tokens = max_tokens;
        self
    }
}

impl ChatEndpoint {
    pub fn from_agent(agent: &AgentDocument, model: &ResolvedModel) -> Self {
        let host = agent
            .frontmatter
            .server
            .as_ref()
            .and_then(|s: &ServerConfig| s.host.clone())
            .unwrap_or_else(|| "127.0.0.1".into());
        let port = agent
            .frontmatter
            .server
            .as_ref()
            .and_then(|s| s.port)
            .unwrap_or(8080);
        let temperature = agent
            .frontmatter
            .sampling
            .as_ref()
            .and_then(|s: &SamplingConfig| s.temperature)
            .unwrap_or(0.2);
        let max_tokens = agent
            .frontmatter
            .sampling
            .as_ref()
            .and_then(|s| s.max_tokens)
            .unwrap_or(1024);
        Self {
            base_url: format!("http://{host}:{port}"),
            model_alias: model.alias.clone(),
            temperature,
            max_tokens,
        }
    }
}

/// Non-streaming complete (for tests / fallback).
pub async fn complete_chat(
    endpoint: &ChatEndpoint,
    system: &str,
    history: &[ChatMessage],
    user: &str,
) -> Result<String, ChatError> {
    let mut messages = vec![serde_json::json!({
        "role": "system",
        "content": system,
    })];
    for h in history {
        messages.push(serde_json::json!({
            "role": h.role,
            "content": h.content,
        }));
    }
    messages.push(serde_json::json!({
        "role": "user",
        "content": user,
    }));

    let body = serde_json::json!({
        "model": endpoint.model_alias,
        "messages": messages,
        "temperature": endpoint.temperature,
        "max_tokens": endpoint.max_tokens,
        "stream": false,
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/v1/chat/completions", endpoint.base_url))
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let t = resp.text().await.unwrap_or_default();
        return Err(ChatError::Server(t));
    }
    let v: serde_json::Value = resp.json().await?;
    let content = v["choices"][0]["message"]["content"]
        .as_str()
        .or_else(|| v["choices"][0]["message"]["reasoning_content"].as_str())
        .unwrap_or("")
        .to_string();
    if content.is_empty() {
        return Err(ChatError::Empty);
    }
    Ok(content)
}

/// Stream chat completions; yields text deltas.
pub async fn stream_chat<F>(
    endpoint: &ChatEndpoint,
    system: &str,
    history: &[ChatMessage],
    user: &str,
    mut on_delta: F,
) -> Result<String, ChatError>
where
    F: FnMut(&str),
{
    let mut messages = vec![serde_json::json!({
        "role": "system",
        "content": system,
    })];
    for h in history {
        messages.push(serde_json::json!({
            "role": h.role,
            "content": h.content,
        }));
    }
    messages.push(serde_json::json!({
        "role": "user",
        "content": user,
    }));

    let body = serde_json::json!({
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
        return Err(ChatError::Server(t));
    }

    let mut full = String::new();
    let mut stream = resp.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
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
                return Ok(full);
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
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
        // fallback non-stream
        return complete_chat(endpoint, system, history, user).await;
    }
    Ok(full)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{AgentFrontmatter, LATEST_SCHEMA_VERSION};

    #[test]
    fn endpoint_from_agent_defaults() {
        let agent = AgentDocument {
            path: "x".into(),
            id: "cat/x".into(),
            category: "cat".into(),
            stem: "x".into(),
            frontmatter: AgentFrontmatter {
                schema_version: LATEST_SCHEMA_VERSION,
                name: "x".into(),
                description: String::new(),
                default_model: "m".into(),
                role: "agent".into(),
                tools: vec!["*".into()],
                server: None,
                sampling: Some(SamplingConfig {
                    temperature: Some(0.0),
                    max_tokens: Some(64),
                    seed: None,
                }),
            },
            body: "sys".into(),
        };
        let model = ResolvedModel {
            key: "m".into(),
            path: "/tmp/m.gguf".into(),
            alias: "m-alias".into(),
            defaults: Default::default(),
        };
        let ep = ChatEndpoint::from_agent(&agent, &model);
        assert_eq!(ep.model_alias, "m-alias");
        assert_eq!(ep.max_tokens, 64);
        assert!((ep.temperature - 0.0).abs() < f64::EPSILON);
        assert!(ep.base_url.contains("8080"));
    }
}
