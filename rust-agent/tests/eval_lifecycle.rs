//! Fail-closed write + nested eval depth + implementor tools.

use rust_agent::{
    agents_dir, invoke_tool, list_agent_local_tools, load_agent, probe_eval_depth_exceeded,
    ToolContext, AGENT_IMPLEMENTOR_ID, CORE_AGENT_ID, TOOL_IMPLEMENTOR_ID,
};
use rust_agent::{crate_root, registry_path, repo_root, sessions_dir};
use serde_json::json;
use std::path::PathBuf;

fn agent_ctx(id: &str) -> ToolContext {
    let dir = agents_dir();
    let doc = load_agent(&dir, id).unwrap();
    ToolContext::from_paths(
        dir,
        registry_path(),
        sessions_dir(),
        crate_root(),
        repo_root(),
        Some(id.into()),
        doc.frontmatter.tools.clone(),
    )
}

#[test]
fn nested_eval_depth_exceeded() {
    assert!(
        probe_eval_depth_exceeded(CORE_AGENT_ID),
        "host depth guard must reject re-entrant eval"
    );
    let r = rust_agent::run_agent_eval(CORE_AGENT_ID, "tool_plan");
    assert!(r.is_ok(), "after probe, normal eval still works: {r:?}");
}

#[test]
fn write_agent_red_eval_rolls_back() {
    let ctx = agent_ctx(AGENT_IMPLEMENTOR_ID);
    let id = "lab/red-publish";
    let agent_path = agents_dir().join("lab/red-publish.md");
    let ds = rust_agent::eval::dataset_path_for(id);
    let _ = std::fs::remove_file(&agent_path);
    let _ = std::fs::remove_file(&ds);

    if let Some(p) = ds.parent() {
        std::fs::create_dir_all(p).unwrap();
    }
    std::fs::write(
        &ds,
        "## case: impossible\ntrack: tool_plan\nprompt: x\nrequire_tools: [list_agents]\n",
    )
    .unwrap();

    let md = r#"---
schema_version: 1
name: red-publish
description: should roll back
default_model: qwen3-0.6b
role: agent
tools:
  - list_tools
---

body
"#;
    let res = invoke_tool(
        &ctx,
        "write_agent",
        &json!({
            "id": id,
            "markdown": md,
            "require_eval": true
        }),
    );
    assert!(!res.ok, "red eval must fail tool: {:?}", res.result);
    let err = res
        .result
        .get("error")
        .and_then(|e| e.as_str())
        .unwrap_or("");
    assert!(
        err.contains("rolled back") || err.contains("publish blocked"),
        "error={err}"
    );
    assert!(
        !agent_path.is_file(),
        "agent file must not remain after red publish"
    );
    let _ = std::fs::remove_file(&ds);
}

#[test]
fn write_agent_green_stays() {
    let ctx = agent_ctx(AGENT_IMPLEMENTOR_ID);
    let id = "lab/green-publish";
    let agent_path = agents_dir().join("lab/green-publish.md");
    let ds = rust_agent::eval::dataset_path_for(id);
    let _ = std::fs::remove_file(&agent_path);
    let _ = std::fs::remove_file(&ds);

    let md = r#"---
schema_version: 1
name: green-publish
description: ok
default_model: qwen3-0.6b
role: agent
tools:
  - list_tools
---

body
"#;
    let res = invoke_tool(
        &ctx,
        "write_agent",
        &json!({
            "id": id,
            "markdown": md,
            "require_eval": true
        }),
    );
    assert!(res.ok, "green publish: {:?}", res.result);
    assert!(agent_path.is_file());
    let _ = std::fs::remove_file(&agent_path);
    let _ = std::fs::remove_file(&ds);
}

#[test]
fn write_tool_via_tool_implementor() {
    let ctx = agent_ctx(TOOL_IMPLEMENTOR_ID);
    let shared = invoke_tool(
        &ctx,
        "write_tool",
        &json!({
            "scope": "shared",
            "category": "proposed",
            "name": "echo_draft",
            "content": "// draft tool\n"
        }),
    );
    assert!(shared.ok, "{:?}", shared.result);
    let path = shared.result["path"].as_str().unwrap();
    assert!(path.ends_with(".rs.draft"));
    assert!(PathBuf::from(path).is_file());
    assert_eq!(shared.result["registered"], false);

    let local = invoke_tool(
        &ctx,
        "write_tool",
        &json!({
            "scope": "agent_local",
            "agent_id": AGENT_IMPLEMENTOR_ID,
            "name": "local_hint",
            "content": "# local hint\n"
        }),
    );
    assert!(local.ok, "{:?}", local.result);
    let names = list_agent_local_tools(agents_dir(), AGENT_IMPLEMENTOR_ID);
    assert!(
        names.iter().any(|n| n == "local_hint"),
        "local tools={names:?}"
    );
    let bad = invoke_tool(
        &ctx,
        "write_tool",
        &json!({
            "scope": "shared",
            "category": "../src",
            "name": "evil",
            "content": "x"
        }),
    );
    assert!(!bad.ok, "must deny traversal category");
}
