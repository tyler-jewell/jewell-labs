//! Shared application state passed to every handler.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::Mutex;

use crate::config::{self, Config};
use crate::identity::KeyStore;
use crate::registry::Registry;

#[derive(Clone)]
pub struct AppState {
    pub cfg: Arc<Config>,
    pub http: reqwest::Client,
    pub registry: Arc<Registry>,
    pub keys: Arc<KeyStore>,
    /// CSRF states for the browser Google flow (state -> issued-at).
    pub oauth_states: Arc<Mutex<HashMap<String, Instant>>>,
}

impl AppState {
    pub fn new(cfg: Config) -> Self {
        let registry = Registry::from_config(&cfg);
        let keys = KeyStore::new(
            config::keys_file(),
            cfg.auth.access_ttl_secs,
            cfg.auth.refresh_ttl_secs,
        );
        AppState {
            cfg: Arc::new(cfg),
            http: reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap(),
            registry: Arc::new(registry),
            keys: Arc::new(keys),
            oauth_states: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}
