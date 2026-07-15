//! Bearer-token sources for providers that need upstream auth. Each source owns its own
//! cache + mutex so concurrent chats don't double-refresh.
//!
//! ponytail: last-writer-wins on the Keychain / grok auth.json; tolerable at single-user
//! scale (same ceiling the Python gateway accepted).

pub mod keychain;
pub mod oidc;

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;

pub fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}

#[derive(Default)]
pub struct Cache {
    pub tok: Option<String>,
    pub exp: f64, // unix seconds
}

/// How a provider obtains its upstream bearer token (or that it needs none).
#[derive(Clone)]
pub enum TokenSource {
    /// Claude Code OAuth token from the macOS Keychain (refreshed like Claude Code does).
    Keychain(Arc<Mutex<Cache>>),
    /// grok CLI OIDC token in ~/.grok/auth.json (refreshed via its OIDC issuer).
    Oidc {
        lock: Arc<Mutex<()>>,
        auth_file: PathBuf,
    },
    /// Local backends (llama.cpp/ollama) need no bearer.
    None,
}

impl TokenSource {
    pub fn keychain() -> Self {
        TokenSource::Keychain(Arc::new(Mutex::new(Cache::default())))
    }

    pub fn oidc(auth_file: PathBuf) -> Self {
        TokenSource::Oidc {
            lock: Arc::new(Mutex::new(())),
            auth_file,
        }
    }

    /// A valid bearer, refreshing + persisting if within skew of expiry. `None` = no auth.
    pub async fn bearer(&self, http: &reqwest::Client) -> anyhow::Result<Option<String>> {
        match self {
            TokenSource::Keychain(cache) => Ok(Some(keychain::claude_token(http, cache).await?)),
            TokenSource::Oidc { lock, auth_file } => {
                Ok(Some(oidc::grok_token(http, lock, auth_file).await?))
            }
            TokenSource::None => Ok(None),
        }
    }
}
