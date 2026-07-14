//! Shared tools: `tools/{category}/{name}.rs`.
//! Agent-local drafts: `agents/{category}/{name}/tools/*` (metadata only until host-registered).

use crate::agents::validate_segment;
use crate::tools::builtin_tools;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct ToolFileEntry {
    pub id: String,
    pub category: String,
    pub name: String,
    pub path: String,
    /// Present in the compiled tools registry (has run() + spec).
    pub registered: bool,
    pub description: String,
    pub selected: bool,
}

/// Scan `tools_dir/{category}/*.rs` (skips `mod.rs`).
pub fn list_tools_from_fs(tools_dir: impl AsRef<Path>) -> Vec<ToolFileEntry> {
    let root = tools_dir.as_ref();
    let registry = builtin_tools();
    let mut out = Vec::new();
    if !root.is_dir() {
        return out;
    }

    let mut categories: Vec<PathBuf> = fs::read_dir(root)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .filter(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .map(|s| validate_segment(s).is_ok())
                .unwrap_or(false)
        })
        .collect();
    categories.sort();

    for cat_dir in categories {
        let category = cat_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let mut files: Vec<PathBuf> = fs::read_dir(&cat_dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e == "rs")
                    .unwrap_or(false)
            })
            .filter(|p| {
                let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                stem != "mod" && validate_segment(stem).is_ok()
            })
            .collect();
        files.sort();

        for path in files {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            let id = format!("{category}/{name}");
            let reg = registry.iter().find(|t| t.name == name && t.category == category);
            // also match by name only if category matches registry
            let reg = reg.or_else(|| registry.iter().find(|t| t.name == name));
            out.push(ToolFileEntry {
                id,
                category: category.clone(),
                name: name.clone(),
                path: path.display().to_string(),
                registered: reg.is_some(),
                description: reg
                    .map(|t| t.description.clone())
                    .unwrap_or_else(|| "(on disk; not in runtime registry)".into()),
                selected: false,
            });
        }
    }
    out
}

/// Agent-local tool drafts under `agents/{cat}/{name}/tools/` (not invocable unless registered).
pub fn list_agent_local_tools(agents_dir: impl AsRef<Path>, agent_id: &str) -> Vec<String> {
    let parts: Vec<_> = agent_id.splitn(2, '/').collect();
    if parts.len() != 2 {
        return vec![];
    }
    let dir = agents_dir.as_ref().join(parts[0]).join(parts[1]).join("tools");
    if !dir.is_dir() {
        return vec![];
    }
    let mut names = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                if stem != "mod" && validate_segment(stem).is_ok() {
                    names.push(stem.to_string());
                }
            }
        }
    }
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::tools_dir;
    use tempfile::tempdir;

    #[test]
    fn scans_crate_tools_dir() {
        let dir = tools_dir();
        let items = list_tools_from_fs(&dir);
        assert!(
            items.iter().any(|t| t.id == "introspect/list_agents"),
            "got {:?}",
            items.iter().map(|t| t.id.clone()).collect::<Vec<_>>()
        );
        assert!(items.iter().any(|t| t.category == "sessions"));
        assert!(items.iter().all(|t| t.name != "mod"));
        assert!(items.iter().any(|t| t.registered));
    }

    #[test]
    fn agent_local_tools_only_for_that_agent() {
        let d = tempdir().unwrap();
        fs::create_dir_all(d.path().join("system/learner/tools")).unwrap();
        fs::write(d.path().join("system/learner/tools/hint.md"), "x").unwrap();
        let local = list_agent_local_tools(d.path(), "system/learner");
        assert_eq!(local, vec!["hint"]);
        assert!(list_agent_local_tools(d.path(), "core/orchestrator").is_empty());
    }
}
