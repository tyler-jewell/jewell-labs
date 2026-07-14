//! Integration tests against rust-agent/agents/ — fixed four-agent team.

use rust_agent::{
    agents_dir, all_tool_names, builtin_tools, certify_agent_markdown, invoke_tool, list_agents,
    load_agent, registry_path, repo_root, sessions_dir, ToolContext, AGENT_IMPLEMENTOR_ID,
    CORE_AGENT_ID, CORE_AGENT_TOOLS, LEARNER_ID, TEAM_AGENT_IDS, TOOL_IMPLEMENTOR_ID,
};
use serde_json::json;
use std::collections::BTreeSet;
use std::path::PathBuf;

#[test]
fn core_orchestrator_is_loadable() {
    let dir = agents_dir();
    assert!(dir.is_dir(), "agents dir missing at {}", dir.display());
    let orch = load_agent(&dir, CORE_AGENT_ID).expect("load core orchestrator");
    assert_eq!(orch.id, CORE_AGENT_ID);
    assert_eq!(orch.frontmatter.role, "orchestrator");
    assert_eq!(orch.category, "core");
    assert_eq!(orch.frontmatter.default_model, "qwen3-4b");
    let claimed: BTreeSet<_> = CORE_AGENT_TOOLS.iter().map(|s| (*s).to_string()).collect();
    let fm: BTreeSet<_> = orch.frontmatter.tools.iter().cloned().collect();
    assert_eq!(
        fm, claimed,
        "frontmatter tools must match CORE_AGENT_TOOLS"
    );
    let reg: BTreeSet<_> = all_tool_names().into_iter().collect();
    assert!(
        claimed.is_subset(&reg),
        "CORE tools must be ⊆ registry; missing {:?}",
        claimed.difference(&reg)
    );
    assert!(reg.len() > claimed.len(), "registry has specialist tools too");
    let text = std::fs::read_to_string(&orch.path).unwrap();
    let cert = certify_agent_markdown(&text);
    assert!(cert.ok, "core agent cert errors: {:?}", cert.errors);
}

/// Policy for FC/NO_TOOL + multi-step fs lives in the orchestrator body (single SSoT),
/// not as narrative coaching in the tool-loop / protocol appendix.
#[test]
fn orchestrator_body_is_policy_ssot() {
    let orch = load_agent(agents_dir(), CORE_AGENT_ID).expect("load orchestrator");
    let body = orch.body.to_ascii_lowercase();
    for required in [
        "fs_write",
        "tool_result",
        "no_tool",
        "tools:",
        "list_tools",
    ] {
        assert!(
            body.contains(required),
            "orchestrator body must cover {required:?} (policy SSoT); body={}",
            orch.body
        );
    }
    // Minimal: body should stay short (policy, not a second tools catalog).
    assert!(
        orch.body.len() < 2200,
        "orchestrator body too long ({} chars); keep a single minimal prompt",
        orch.body.len()
    );

    let root = repo_root();
    // Product tool-loop paths (not test modules that may quote banned phrases).
    let loop_sources = [
        root.join("rust-agent/src/agent_run.rs"),
        root.join("rust-agent/src/server/api_chat.rs"),
    ];
    let banned = [
        "call the appropriate tool",
        "rather than only describing",
        "If files still need to be created or updated",
    ];
    for path in loop_sources {
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        // Strip cfg(test) modules so test strings cannot false-positive.
        let production = text
            .split("#[cfg(test)]")
            .next()
            .unwrap_or(&text);
        for b in banned {
            assert!(
                !production.contains(b),
                "{} must not contain policy coaching {b:?} — put it in orchestrator.md",
                path.display()
            );
        }
        assert!(
            production.contains("tool_result for {}:\\n{}")
                || production.contains("tool_result for {}:\n{}"),
            "{} should format mechanical tool_result payload only",
            path.display()
        );
    }
    // Protocol appendix policy checked via tools_appendix_is_mechanical unit test.
}

#[test]
fn fixed_team_present() {
    let items = list_agents(agents_dir()).unwrap();
    let ids: BTreeSet<_> = items.iter().map(|i| i.id.clone()).collect();
    for id in TEAM_AGENT_IDS {
        assert!(ids.contains(*id), "missing team agent {id}; have {ids:?}");
    }
    for id in [LEARNER_ID, AGENT_IMPLEMENTOR_ID, TOOL_IMPLEMENTOR_ID] {
        let a = load_agent(agents_dir(), id).unwrap();
        assert_ne!(a.frontmatter.role, "orchestrator");
    }
}

#[test]
fn list_includes_core_agent() {
    let items = list_agents(agents_dir()).unwrap();
    assert!(
        items
            .iter()
            .any(|i| i.id == CORE_AGENT_ID && i.role == "orchestrator"),
        "core missing: {:?}",
        items.iter().map(|i| i.id.clone()).collect::<Vec<_>>()
    );
}

#[test]
fn core_agent_can_invoke_every_core_tool() {
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

    assert_eq!(all_tool_names().len(), builtin_tools().len());
    assert_eq!(CORE_AGENT_TOOLS.len(), 15); // + fs_write, fs_read, fs_list

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

    for name in CORE_AGENT_TOOLS {
        if *name == "run_eval" {
            continue; // nested depth when already evaluating; covered separately
        }
        let args = match *name {
            "list_tools" | "app_status" | "list_agents" | "get_schema" | "list_models"
            | "fs_list" => json!({}),
            "get_agent" | "certify_agent" => json!({"id": CORE_AGENT_ID}),
            "list_sessions" => json!({"agent_id": CORE_AGENT_ID}),
            "get_session" => json!({"agent_id": CORE_AGENT_ID, "session_id": "orch-smoke"}),
            "upsert_session" => json!({
                "agent_id": CORE_AGENT_ID,
                "session_id": "orch-smoke-2",
                "messages": [{"role": "user", "content": "hi"}]
            }),
            "run_agent" => json!({"agent_id": LEARNER_ID, "dry_run": true}),
            "fs_write" => json!({"path": "orch-smoke.txt", "content": "ok"}),
            "fs_read" => json!({"path": "orch-smoke.txt"}),
            other => panic!("no smoke args for tool {other}"),
        };
        let result = invoke_tool(&ctx, name, &args);
        assert!(
            result.ok,
            "core agent failed tool {name}: {:?}",
            result.result
        );
    }

    let denied = invoke_tool(
        &ctx,
        "write_agent",
        &json!({"id": "lab/x", "markdown": "x", "require_eval": false}),
    );
    assert!(!denied.ok, "orch must not call write_agent");

    let search = invoke_tool(
        &ctx,
        "list_sessions",
        &json!({"query": "pi", "agent_id": CORE_AGENT_ID}),
    );
    assert!(search.ok, "{:?}", search.result);
    assert_eq!(search.result["mode"], "search");
}

#[test]
fn specialists_are_restricted() {
    let dir = agents_dir();
    let learner = load_agent(&dir, LEARNER_ID).unwrap();
    let ctx = ToolContext::from_paths(
        dir,
        registry_path(),
        sessions_dir(),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        repo_root(),
        Some(LEARNER_ID.into()),
        learner.frontmatter.tools.clone(),
    );
    let denied = invoke_tool(
        &ctx,
        "write_agent",
        &json!({"id": "lab/x", "markdown": "x", "require_eval": false}),
    );
    assert!(!denied.ok, "learner must not write_agent");
    let learn = invoke_tool(
        &ctx,
        "learn",
        &json!({"targets": [AGENT_IMPLEMENTOR_ID], "dry_run": true}),
    );
    assert!(learn.ok, "{:?}", learn.result);
}
