//! Load and list agents from `agents/{category}/{agent-name}.md`.
//!
//! **Core agent** (required): `core/orchestrator` — see [`CORE_AGENT_ID`].
//! Specialty agents (e.g. tutoring/*) are optional and never required for core gates.

mod list;
mod path;

pub use list::{
    certify_agent_file, list_agents, load_agent, write_agent_file, AgentListItem, AgentsError,
};
pub use path::{
    agent_id, parse_agent_ref, path_is_under_agents, resolve_agent_path, validate_segment,
    validate_stem,
};

/// Stable id of the product core agent (orchestrator).
pub const CORE_AGENT_ID: &str = "core/orchestrator";

/// Explicit lean tool allowlist claimed by the core orchestrator (must match frontmatter).
pub const CORE_AGENT_TOOLS: &[&str] = &[
    "list_tools",
    "app_status",
    "list_agents",
    "get_agent",
    "certify_agent",
    "get_schema",
    "list_models",
    "list_sessions",
    "get_session",
    "upsert_session",
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn sample(name: &str, role: &str, tools: &str) -> String {
        format!(
            r#"---
schema_version: 1
name: {name}
description: d
default_model: qwen3-0.6b
role: {role}
tools: {tools}
---

body
"#
        )
    }

    #[test]
    fn list_nested_agents() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("core")).unwrap();
        fs::create_dir_all(dir.path().join("tutoring")).unwrap();
        fs::write(
            dir.path().join("core/orchestrator.md"),
            sample(
                "orchestrator",
                "orchestrator",
                r#"["list_tools","app_status","list_agents","get_agent","certify_agent","get_schema","list_models","list_sessions","get_session","upsert_session"]"#,
            ),
        )
        .unwrap();
        fs::write(
            dir.path().join("tutoring/math-tutor.md"),
            sample("math-tutor", "agent", r#"["list_tools"]"#),
        )
        .unwrap();

        let items = list_agents(dir.path()).unwrap();
        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|i| i.id == "core/orchestrator"));
        assert!(items.iter().any(|i| i.id == "tutoring/math-tutor"));
        let orch = items.iter().find(|i| i.id == "core/orchestrator").unwrap();
        assert_eq!(orch.role, "orchestrator");
        assert!(orch.certification.ok);
    }

    #[test]
    fn load_and_write_ref() {
        let dir = tempdir().unwrap();
        let md = sample("new-bot", "agent", r#"["list_agents"]"#);
        write_agent_file(dir.path(), "lab/new-bot", &md).unwrap();
        let doc = load_agent(dir.path(), "lab/new-bot").unwrap();
        assert_eq!(doc.category, "lab");
        assert_eq!(doc.stem, "new-bot");
        assert_eq!(doc.frontmatter.tools, vec!["list_agents"]);
    }

    #[test]
    fn rejects_traversal() {
        let dir = tempdir().unwrap();
        for bad in ["../AGENTS", "a/../../b", "foo", "", "a/b/c", "has space/x"] {
            let err = load_agent(dir.path(), bad).unwrap_err();
            assert!(
                matches!(err, AgentsError::InvalidStem(_) | AgentsError::NotFound(_)),
                "bad={bad:?} err={err:?}"
            );
        }
        let md = sample("evil", "agent", "[]");
        let parent = dir.path().parent().unwrap().join("evil.md");
        let _ = fs::remove_file(&parent);
        let err = write_agent_file(dir.path(), "../evil", &md).unwrap_err();
        assert!(matches!(err, AgentsError::InvalidStem(_)));
        assert!(!parent.exists());
    }

    #[test]
    fn parse_ref_ok() {
        let (c, n) = parse_agent_ref("core/orchestrator").unwrap();
        assert_eq!(c, "core");
        assert_eq!(n, "orchestrator");
        assert!(validate_segment("math-tutor").is_ok());
    }

    #[test]
    fn core_constants_are_stable() {
        assert_eq!(CORE_AGENT_ID, "core/orchestrator");
        assert_eq!(CORE_AGENT_TOOLS.len(), 10);
    }
}
