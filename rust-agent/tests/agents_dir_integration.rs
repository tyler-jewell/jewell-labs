//! Integration tests against rust-agent/agents/ — core agent is required; specialty is optional.

use rust_agent::{
    agents_dir, all_tool_names, builtin_tools, certify_agent_markdown, invoke_tool, list_agents,
    load_agent, registry_path, repo_root, sessions_dir, ToolContext, CORE_AGENT_ID,
    CORE_AGENT_TOOLS, LATEST_SCHEMA_VERSION,
};
use serde_json::json;
use std::collections::BTreeSet;
use std::path::PathBuf;

#[test]
fn core_orchestrator_is_loadable_alone() {
    let dir = agents_dir();
    assert!(dir.is_dir(), "agents dir missing at {}", dir.display());
    let orch = load_agent(&dir, CORE_AGENT_ID).expect("load core orchestrator");
    assert_eq!(orch.id, CORE_AGENT_ID);
    assert_eq!(orch.frontmatter.role, "orchestrator");
    assert_eq!(orch.category, "core");
    assert!(!orch.frontmatter.default_model.is_empty());
    let claimed: BTreeSet<_> = CORE_AGENT_TOOLS.iter().map(|s| (*s).to_string()).collect();
    let fm: BTreeSet<_> = orch.frontmatter.tools.iter().cloned().collect();
    assert_eq!(
        fm, claimed,
        "frontmatter tools must match CORE_AGENT_TOOLS"
    );
    let reg: BTreeSet<_> = all_tool_names().into_iter().collect();
    assert_eq!(reg, claimed, "registry must equal CORE_AGENT_TOOLS (lean)");
    let text = std::fs::read_to_string(&orch.path).unwrap();
    let cert = certify_agent_markdown(&text);
    assert!(cert.ok, "core agent cert errors: {:?}", cert.errors);
}

#[test]
fn specialty_agents_are_optional() {
    // Core gate must not require specialty files to exist for identity.
    let dir = agents_dir();
    let orch = load_agent(&dir, CORE_AGENT_ID).unwrap();
    assert_eq!(orch.frontmatter.role, "orchestrator");
    // Specialty may exist in this repo but must not share core id/role.
    if let Ok(tutor) = load_agent(&dir, "tutoring/math-tutor") {
        assert_ne!(tutor.id, CORE_AGENT_ID);
        assert_ne!(tutor.frontmatter.role, "orchestrator");
    }
}

#[test]
fn list_includes_core_agent() {
    let items = list_agents(agents_dir()).unwrap();
    assert!(
        items.iter().any(|i| i.id == CORE_AGENT_ID && i.role == "orchestrator"),
        "core missing: {:?}",
        items.iter().map(|i| i.id.clone()).collect::<Vec<_>>()
    );
}

#[test]
fn core_agent_can_invoke_every_registered_tool() {
    let dir = agents_dir();
    let orch = load_agent(&dir, CORE_AGENT_ID).unwrap();
    let ctx = ToolContext::from_paths(
        dir,
        registry_path(),
        sessions_dir(),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        repo_root(),
        Some(CORE_AGENT_ID.into()),
        orch.frontmatter.tools.clone(),
    );

    let names = all_tool_names();
    assert_eq!(names.len(), builtin_tools().len());
    assert_eq!(names.len(), CORE_AGENT_TOOLS.len());

    let seed = invoke_tool(
        &ctx,
        "upsert_session",
        &json!({
            "agent_id": CORE_AGENT_ID,
            "session_id": "orch-smoke",
            "messages": [
                {"role": "user", "content": "core smoke pi"},
                {"role": "assistant", "content": "ready"}
            ]
        }),
    );
    assert!(seed.ok, "upsert_session: {:?}", seed.result);

    for name in &names {
        let args = match name.as_str() {
            "list_tools" | "app_status" | "list_agents" | "get_schema" | "list_models" => {
                json!({})
            }
            "get_agent" | "certify_agent" => json!({"id": CORE_AGENT_ID}),
            "list_sessions" => json!({"agent_id": CORE_AGENT_ID}),
            "get_session" => json!({"agent_id": CORE_AGENT_ID, "session_id": "orch-smoke"}),
            "upsert_session" => json!({
                "agent_id": CORE_AGENT_ID,
                "session_id": "orch-smoke-2",
                "messages": [{"role": "user", "content": "hi"}]
            }),
            other => panic!("no smoke args for tool {other}"),
        };
        let result = invoke_tool(&ctx, name, &args);
        assert!(
            result.ok,
            "core agent failed tool {name}: {:?}",
            result.result
        );
    }

    // merged search
    let search = invoke_tool(
        &ctx,
        "list_sessions",
        &json!({"query": "pi", "agent_id": CORE_AGENT_ID}),
    );
    assert!(search.ok, "{:?}", search.result);
    assert_eq!(search.result["mode"], "search");
    assert!(search.result["count"].as_u64().unwrap_or(0) >= 1);
}

#[test]
fn specialty_math_tutor_if_present_is_restricted() {
    let dir = agents_dir();
    let Ok(tutor) = load_agent(&dir, "tutoring/math-tutor") else {
        return; // optional
    };
    assert_eq!(tutor.frontmatter.schema_version, LATEST_SCHEMA_VERSION);
    let ctx = ToolContext::from_paths(
        dir,
        registry_path(),
        sessions_dir(),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        repo_root(),
        Some("tutoring/math-tutor".into()),
        tutor.frontmatter.tools.clone(),
    );
    let denied = invoke_tool(&ctx, "list_agents", &json!({}));
    assert!(!denied.ok);
    let allowed = invoke_tool(&ctx, "app_status", &json!({}));
    assert!(allowed.ok, "{:?}", allowed.result);
}
