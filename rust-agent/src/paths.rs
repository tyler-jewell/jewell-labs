//! Resolve crate + monorepo paths.

use std::path::{Path, PathBuf};

/// Absolute path to the `rust-agent` crate root (compile-time).
pub fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Monorepo root containing `models/` (parent of rust-agent, or RUST_AGENT_ROOT).
pub fn repo_root() -> PathBuf {
    if let Ok(root) = std::env::var("RUST_AGENT_ROOT") {
        return PathBuf::from(root);
    }
    let crate_dir = crate_root();
    if let Some(parent) = crate_dir.parent() {
        if parent.join("models").is_dir() {
            return parent.to_path_buf();
        }
    }
    // walk from CWD
    let mut dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for _ in 0..6 {
        if dir.join("models").is_dir() {
            return dir;
        }
        if !dir.pop() {
            break;
        }
    }
    crate_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or(crate_dir)
}

/// Agents live at `rust-agent/agents/{category}/{name}.md`.
pub fn agents_dir() -> PathBuf {
    crate_root().join("agents")
}

/// Tools live at `rust-agent/tools/{category}/{name}.rs`.
pub fn tools_dir() -> PathBuf {
    crate_root().join("tools")
}

pub fn registry_path() -> PathBuf {
    repo_root().join("models").join("registry.yaml")
}

/// Server-side chat sessions under the crate.
pub fn sessions_dir() -> PathBuf {
    crate::tools::sessions_dir()
}

pub fn ensure_under_agents(path: &Path) -> bool {
    crate::agents::path_is_under_agents(agents_dir(), path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_has_agents_layout() {
        let agents = agents_dir();
        assert!(
            agents.ends_with("rust-agent/agents") || agents.ends_with("agents"),
            "{}",
            agents.display()
        );
    }

    #[test]
    fn ensure_under_agents_nested() {
        let inside = agents_dir().join("system").join("orchestrator.md");
        if inside.is_file() {
            assert!(ensure_under_agents(&inside));
        }
        let sibling = repo_root().join("AGENTS.md");
        if sibling.is_file() {
            assert!(!ensure_under_agents(&sibling));
        }
    }
}
