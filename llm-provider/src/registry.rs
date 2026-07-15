//! The `Provider` trait and the registry that routes model ids to providers.
//!
//! Adding a provider is mechanical: write `providers/<x>.rs` implementing `Provider`, add a
//! `ProviderConfig::<X>` variant in config.rs, and add one arm to `from_config` below.

use std::sync::Mutex;
use std::time::Instant;

use async_trait::async_trait;
use axum::response::Response;
use serde_json::Value;

use crate::config::{Config, ProviderConfig};
use crate::providers::{ClaudeProvider, PassthroughProvider};

#[async_trait]
pub trait Provider: Send + Sync {
    /// Stable id; also the `owned_by` value in model objects.
    fn name(&self) -> &str;

    /// Does this provider own `model`? Cheap/pure for prefix providers; local providers
    /// consult a set warmed by `models()`.
    fn matches(&self, model: &str) -> bool;

    /// OpenAI model objects. An unreachable backend returns `Ok(vec![])` (skip), never a
    /// hard error that would sink the whole aggregate.
    async fn models(&self, http: &reqwest::Client) -> anyhow::Result<Vec<Value>>;

    /// Handle a chat/completions request end to end (SSE stream or buffered JSON).
    async fn chat(&self, http: &reqwest::Client, body: Value) -> anyhow::Result<Response>;
}

struct ModelCache {
    at: Option<Instant>,
    data: Vec<Value>,
}

pub struct Registry {
    providers: Vec<Box<dyn Provider>>,
    cache: Mutex<ModelCache>,
}

impl Registry {
    /// THE registration point. Order matters: prefix providers (claude, grok) come before
    /// the local fallback so a prefixed id never gets shadowed by a discovered local model.
    pub fn from_config(cfg: &Config) -> Self {
        let mut providers: Vec<Box<dyn Provider>> = Vec::new();
        for pc in &cfg.providers {
            providers.push(match pc {
                ProviderConfig::Claude(c) => Box::new(ClaudeProvider::new(c)) as Box<dyn Provider>,
                ProviderConfig::Grok(c) => Box::new(PassthroughProvider::grok(c)),
                ProviderConfig::Local(c) => Box::new(PassthroughProvider::local(c)),
            });
        }
        Registry {
            providers,
            cache: Mutex::new(ModelCache { at: None, data: vec![] }),
        }
    }

    /// First matching provider wins. If nothing matches on the first pass, warm the model
    /// caches (populates local providers' discovered sets) and retry once.
    // ponytail: local match depends on a warmed model cache; the retry pays for one
    // discovery round-trip only when a prefix provider didn't already claim the id.
    pub async fn route(&self, http: &reqwest::Client, model: &str) -> Option<&dyn Provider> {
        if let Some(p) = self.providers.iter().find(|p| p.matches(model)) {
            return Some(p.as_ref());
        }
        let _ = self.all_models(http).await;
        self.providers
            .iter()
            .find(|p| p.matches(model))
            .map(|b| b.as_ref())
    }

    /// Concurrent fan-out across providers, cached 30s. Per-provider errors are skipped
    /// (matches the old per-backend tolerance). Side effect on a cache miss: warms local
    /// providers' discovered sets. Shared by `/v1/models` and `route`, so unknown-model
    /// traffic doesn't re-fan-out to every backend on each request.
    pub async fn all_models(&self, http: &reqwest::Client) -> Vec<Value> {
        {
            let cache = self.cache.lock().unwrap();
            if let Some(at) = cache.at {
                if at.elapsed().as_secs() < 30 {
                    return cache.data.clone();
                }
            }
        }
        let futs = self.providers.iter().map(|p| p.models(http));
        let data: Vec<Value> = futures_util::future::join_all(futs)
            .await
            .into_iter()
            .flatten() // drop Err(_)
            .flatten() // flatten Vec<Value>
            .collect();
        let mut cache = self.cache.lock().unwrap();
        cache.at = Some(Instant::now());
        cache.data = data.clone();
        data
    }
}
