//! Shared HTTP application state.

use crate::sessions::SessionStore;
use crate::tools::ToolContext;
use std::path::PathBuf;

#[derive(Clone)]
pub struct AppState {
    pub agents_dir: PathBuf,
    pub registry_path: PathBuf,
    pub sessions_dir: PathBuf,
    pub crate_root: PathBuf,
    pub repo_root: PathBuf,
    pub static_dir: PathBuf,
}

impl AppState {
    pub fn tool_ctx(&self, caller: Option<String>, allowed: Vec<String>) -> ToolContext {
        ToolContext::from_paths(
            self.agents_dir.clone(),
            self.registry_path.clone(),
            self.sessions_dir.clone(),
            self.crate_root.clone(),
            self.repo_root.clone(),
            caller,
            allowed,
        )
    }

    pub fn session_store(&self) -> SessionStore {
        SessionStore::new(self.sessions_dir.clone())
    }
}
