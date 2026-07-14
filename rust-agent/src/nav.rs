//! UI navigation that stays aligned with the on-disk / `src/` structure.
//!
//! - Sidebar **Agents** ← `agents/{category}/{name}.md`
//! - Sidebar **Tools**  ← `tools/{category}/{name}.rs`
//! - Agent top tabs     ← domain modules under `src/` (not infrastructure)

use serde::Serialize;

/// One agent workspace tab, mirrored from a `src/` domain module.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct AgentTabDef {
    /// Query value: `?tab=chat`
    pub id: &'static str,
    /// Human label in the tab bar
    pub label: &'static str,
    /// Source module this tab is implemented by
    pub src_module: &'static str,
    /// Short help under the tab content
    pub description: &'static str,
}

/// Per-agent tabs — keep in sync with domain modules in `src/`.
///
/// Intentionally **not** listed: `main`, `lib`, `app`, `paths`, `agents`
/// (shell / loading infrastructure, not agent workspace surfaces).
pub const AGENT_TABS: &[AgentTabDef] = &[
    AgentTabDef {
        id: "chat",
        label: "Chat",
        src_module: "src/chat.rs",
        description: "Streaming chat via default_model (OpenAI-compatible llama-server).",
    },
    AgentTabDef {
        id: "sessions",
        label: "Sessions",
        src_module: "src/sessions.rs",
        description: "Server-side chat logs under data/sessions/{category}/{name}/.",
    },
    AgentTabDef {
        id: "schema",
        label: "Schema",
        src_module: "src/schema.rs",
        description: "Frontmatter, certification, system prompt body.",
    },
    AgentTabDef {
        id: "registry",
        label: "Registry",
        src_module: "src/registry.rs",
        description: "Resolved default_model from models/registry.yaml.",
    },
];

pub fn default_agent_tab() -> &'static str {
    "chat"
}

pub fn is_valid_agent_tab(tab: &str) -> bool {
    AGENT_TABS.iter().any(|t| t.id == tab)
}

pub fn normalize_agent_tab(tab: &str) -> &'static str {
    let t = tab.trim().to_ascii_lowercase();
    // legacy alias
    let t = if t == "settings" {
        "schema".to_string()
    } else {
        t
    };
    AGENT_TABS
        .iter()
        .find(|tab| tab.id == t)
        .map(|tab| tab.id)
        .unwrap_or_else(|| default_agent_tab())
}

pub fn agent_tab_def(id: &str) -> Option<&'static AgentTabDef> {
    let id = normalize_agent_tab(id);
    AGENT_TABS.iter().find(|t| t.id == id)
}

/// Group items by category folder name (preserves category sort, item sort).
pub fn group_by_category<T, F>(items: Vec<T>, category_of: F) -> Vec<(String, Vec<T>)>
where
    F: Fn(&T) -> String,
{
    use std::collections::BTreeMap;
    let mut map: BTreeMap<String, Vec<T>> = BTreeMap::new();
    for item in items {
        let cat = category_of(&item);
        map.entry(cat).or_default().push(item);
    }
    map.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_match_src_domain_modules() {
        let ids: Vec<_> = AGENT_TABS.iter().map(|t| t.id).collect();
        assert_eq!(ids, vec!["chat", "sessions", "schema", "registry"]);
        for t in AGENT_TABS {
            assert!(t.src_module.starts_with("src/"));
            assert!(t.src_module.ends_with(".rs"));
        }
    }

    #[test]
    fn normalize_legacy_settings() {
        assert_eq!(normalize_agent_tab("settings"), "schema");
        assert_eq!(normalize_agent_tab("CHAT"), "chat");
        assert_eq!(normalize_agent_tab("nope"), "chat");
    }
}
