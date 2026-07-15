//! One provider for every OpenAI-native upstream: grok (bearer + prefix routing) and local
//! llama.cpp/ollama (no bearer, discovered-set routing). They differ only in (base URL,
//! token source, how ownership is decided).

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::config::{self, PassthroughConfig};
use crate::registry::Provider;
use crate::token::TokenSource;

pub struct PassthroughProvider {
    name: String,           // owned_by tag
    base: String,           // e.g. https://api.x.ai/v1 or http://127.0.0.1:8080/v1
    prefix: Option<String>, // Some => prefix routing (grok); None => discovered-set (local)
    token: TokenSource,
    seen: Mutex<HashSet<String>>, // model ids from the last models() call (local only)
}

impl PassthroughProvider {
    pub fn grok(c: &PassthroughConfig) -> Self {
        PassthroughProvider {
            name: if c.name.is_empty() { "xai".into() } else { c.name.clone() },
            base: c.base_url.clone(),
            prefix: Some(if c.prefix.is_empty() { "grok".into() } else { c.prefix.clone() }),
            token: TokenSource::oidc(config::grok_auth_file()),
            seen: Mutex::new(HashSet::new()),
        }
    }

    pub fn local(c: &PassthroughConfig) -> Self {
        PassthroughProvider {
            name: c.name.clone(),
            base: c.base_url.clone(),
            prefix: None,
            token: TokenSource::None,
            seen: Mutex::new(HashSet::new()),
        }
    }
}

#[async_trait]
impl Provider for PassthroughProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn matches(&self, model: &str) -> bool {
        match &self.prefix {
            Some(p) => model.starts_with(p.as_str()),
            None => self.seen.lock().unwrap().contains(model),
        }
    }

    async fn models(&self, http: &reqwest::Client) -> anyhow::Result<Vec<Value>> {
        // Always bound the models call so one slow/hung backend can't stall the aggregate:
        // local backends may be down (short), remote grok gets a longer ceiling.
        let timeout = if self.prefix.is_none() { 3 } else { 10 };
        let mut req = http
            .get(format!("{}/models", self.base))
            .timeout(Duration::from_secs(timeout));
        if let Some(tok) = self.token.bearer(http).await? {
            req = req.bearer_auth(tok);
        }
        let v: Value = req.send().await?.json().await?;
        let mut out = Vec::new();
        let mut ids = HashSet::new();
        for m in v["data"].as_array().unwrap_or(&vec![]) {
            let id = m["id"].as_str().unwrap_or("").to_string();
            let mut e = json!({"id": id, "object": "model", "owned_by": self.name});
            if m["context_length"].is_number() {
                e["context_length"] = m["context_length"].clone();
            }
            ids.insert(id);
            out.push(e);
        }
        if self.prefix.is_none() {
            *self.seen.lock().unwrap() = ids;
        }
        Ok(out)
    }

    async fn chat(&self, http: &reqwest::Client, body: Value) -> anyhow::Result<axum::response::Response> {
        let bearer = self.token.bearer(http).await?;
        super::passthrough(http, &self.base, body, bearer).await
    }
}
