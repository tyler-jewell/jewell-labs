//! Shared tool execution context.

use crate::sessions::SessionStore;
use std::path::PathBuf;

/// Shared context for tool execution (used by src and tools/*).
#[derive(Debug, Clone)]
pub struct ToolContext {
    pub agents_dir: PathBuf,
    pub registry_path: PathBuf,
    pub sessions: SessionStore,
    pub crate_root: PathBuf,
    pub repo_root: PathBuf,
    /// Agent ref currently chatting, e.g. `core/orchestrator`.
    pub caller_agent: Option<String>,
    /// Allowlist from frontmatter `tools:` (empty = none; `*` = all).
    pub allowed_tools: Vec<String>,
    /// When set (catalog eval), fs tools use this root instead of `agents/{id}/fs/`.
    pub sandbox_override: Option<PathBuf>,
}

impl ToolContext {
    pub fn from_paths(
        agents_dir: PathBuf,
        registry_path: PathBuf,
        sessions_root: PathBuf,
        crate_root: PathBuf,
        repo_root: PathBuf,
        caller_agent: Option<String>,
        allowed_tools: Vec<String>,
    ) -> Self {
        Self {
            agents_dir,
            registry_path,
            sessions: SessionStore::new(sessions_root),
            crate_root,
            repo_root,
            caller_agent,
            allowed_tools,
            sandbox_override: None,
        }
    }

    pub fn with_allowlist(mut self, allowed: Vec<String>) -> Self {
        self.allowed_tools = allowed;
        self
    }

    pub fn allow_all(mut self) -> Self {
        self.allowed_tools = vec!["*".into()];
        self
    }

    pub fn with_sandbox_override(mut self, root: Option<PathBuf>) -> Self {
        self.sandbox_override = root;
        self
    }
}

pub fn sessions_dir() -> PathBuf {
    if let Ok(p) = std::env::var("RUST_AGENT_SESSIONS") {
        return PathBuf::from(p);
    }
    crate::paths::crate_root().join("data").join("sessions")
}
