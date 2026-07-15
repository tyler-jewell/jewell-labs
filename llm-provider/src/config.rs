//! Single config file (`config.toml`) — gateway settings + provider list + tuned local
//! runtime profiles. Falls back to sensible defaults when the file is missing, so the
//! binary runs out-of-the-box exactly like the old gateway did on `Config::default()`.

use std::path::PathBuf;

use serde::Deserialize;

/// Base dir holding config.toml + keys.json. Overridable for tests / alternate installs.
pub fn base_dir() -> PathBuf {
    if let Ok(d) = std::env::var("LLM_PROVIDER_DIR") {
        return PathBuf::from(d);
    }
    PathBuf::from(std::env::var("HOME").expect("HOME not set")).join("Apps/jewell-labs/llm-provider")
}

pub fn keys_file() -> PathBuf {
    base_dir().join("keys.json")
}

pub fn grok_auth_file() -> PathBuf {
    PathBuf::from(std::env::var("HOME").expect("HOME not set")).join(".grok/auth.json")
}

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    pub port: u16,
    pub public_url: String,
    pub auth: AuthConfig,
    pub providers: Vec<ProviderConfig>,
    pub local_runtime: LocalRuntime,
}

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct AuthConfig {
    pub allowed_emails: Vec<String>,
    pub admin_contact: String,
    pub google_client_id: String,
    pub google_client_secret: String,
    /// Trust loopback (127.0.0.1/::1) without a key. Keep true for a local-only instance.
    /// Set FALSE when exposing the gateway publicly (e.g. via a Cloudflare Tunnel), because
    /// a local tunnel forwards from localhost and would otherwise let the internet in keyless.
    pub trust_loopback: bool,
}

/// Kind-tagged provider config. Each provider constructor sees only its own variant —
/// no provider can read another's slice. Adding a provider = a new variant + a new module.
#[derive(Deserialize, Clone)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ProviderConfig {
    Claude(ClaudeConfig),
    Grok(PassthroughConfig),
    Local(PassthroughConfig),
}

#[derive(Deserialize, Clone)]
pub struct ClaudeConfig {
    #[serde(default = "default_claude_prefix")]
    pub prefix: String,
    #[serde(default = "default_claude_models")]
    pub models: Vec<String>,
}

#[derive(Deserialize, Clone)]
pub struct PassthroughConfig {
    /// `owned_by` tag in model objects (grok defaults to "xai"; local uses the backend name).
    #[serde(default)]
    pub name: String,
    /// Routing prefix (grok: "grok"). Empty => discovered-set matching (local backends).
    #[serde(default)]
    pub prefix: String,
    pub base_url: String,
}

#[derive(Deserialize, Clone, Default)]
pub struct LocalRuntime {
    #[serde(default)]
    pub profiles: Vec<LocalProfile>,
}

#[derive(Deserialize, Clone)]
pub struct LocalProfile {
    pub name: String,
    pub model: String,
    pub port: u16,
    #[serde(default)]
    pub flags: Vec<String>,
}

fn default_claude_prefix() -> String {
    "claude".into()
}
fn default_claude_models() -> Vec<String> {
    vec![
        "claude-opus-4-8".into(),
        "claude-sonnet-5".into(),
        "claude-haiku-4-5".into(),
    ]
}

impl Default for AuthConfig {
    fn default() -> Self {
        AuthConfig {
            allowed_emails: vec!["tyler.p.jewell@gmail.com".into()],
            admin_contact: String::new(),
            google_client_id: String::new(),
            google_client_secret: String::new(),
            trust_loopback: true,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            port: 4141,
            public_url: String::new(),
            auth: AuthConfig::default(),
            providers: vec![
                ProviderConfig::Claude(ClaudeConfig {
                    prefix: default_claude_prefix(),
                    models: default_claude_models(),
                }),
                ProviderConfig::Grok(PassthroughConfig {
                    name: "xai".into(),
                    prefix: "grok".into(),
                    base_url: "https://api.x.ai/v1".into(),
                }),
                ProviderConfig::Local(PassthroughConfig {
                    name: "llama-cpp".into(),
                    prefix: String::new(),
                    base_url: "http://127.0.0.1:8080/v1".into(),
                }),
                ProviderConfig::Local(PassthroughConfig {
                    name: "ollama".into(),
                    prefix: String::new(),
                    base_url: "http://127.0.0.1:11434/v1".into(),
                }),
            ],
            local_runtime: LocalRuntime::default(),
        }
    }
}

impl Config {
    /// Load from `base_dir()/config.toml`, apply `PORT` env override, derive defaults.
    /// An ABSENT file → defaults. A PRESENT but invalid file → hard error (exit), so a typo
    /// never silently reverts the allowlist/providers/tuning to defaults.
    pub fn load() -> Config {
        let path = base_dir().join("config.toml");
        let mut cfg: Config = match std::fs::read_to_string(&path) {
            Ok(s) => match toml::from_str(&s) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("FATAL: {} is present but invalid — refusing to start on defaults:\n{e}", path.display());
                    std::process::exit(1);
                }
            },
            Err(_) => Config::default(), // absent → defaults
        };
        if let Ok(p) = std::env::var("PORT") {
            cfg.port = p.parse().expect("bad PORT");
        }
        cfg.finalize();
        cfg
    }

    /// Fill derived fields (public_url, admin_contact). Public so tests can build a Config
    /// literal and finalize it.
    pub fn finalize(&mut self) {
        if self.public_url.is_empty() {
            self.public_url = format!("http://localhost:{}", self.port);
        }
        if self.auth.admin_contact.is_empty() {
            let first = self.allowed().next().unwrap_or_default();
            self.auth.admin_contact = first;
        }
    }

    pub fn allowed(&self) -> impl Iterator<Item = String> + '_ {
        self.auth
            .allowed_emails
            .iter()
            .map(|e| e.trim().to_lowercase())
            .filter(|e| !e.is_empty())
    }

    pub fn is_allowed(&self, email: &str) -> bool {
        let email = email.to_lowercase();
        self.allowed().any(|e| e == email)
    }

    pub fn admin_contact(&self) -> &str {
        &self.auth.admin_contact
    }
}
