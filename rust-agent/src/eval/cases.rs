//! Tool-plan eval case and shared plan helpers.

use super::ground_truth::{CaseResult, GroundTruth};
use super::scoring::{fact, score_introspection};
use crate::agent_run::run_tool_plan;
use crate::agents::{load_agent, CORE_AGENT_ID, CORE_AGENT_TOOLS};
use crate::paths::{agents_dir, crate_root, registry_path, repo_root, sessions_dir};
use crate::tools::{all_tool_names, ToolContext};
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub fn orchestrator_ctx() -> (ToolContext, crate::schema::AgentDocument) {
    let agents = agents_dir();
    let doc = load_agent(&agents, CORE_AGENT_ID).expect("core orchestrator agent");
    let ctx = ToolContext::from_paths(
        agents,
        registry_path(),
        sessions_dir(),
        crate_root(),
        repo_root(),
        Some(CORE_AGENT_ID.into()),
        doc.frontmatter.tools.clone(),
    );
    (ctx, doc)
}

/// Full lean tool plan: every registered core tool with safe args.
pub fn full_introspection_plan(_gt: &GroundTruth) -> Vec<(String, Value)> {
    vec![
        ("list_tools".into(), json!({})),
        ("app_status".into(), json!({})),
        ("list_agents".into(), json!({})),
        ("get_agent".into(), json!({"id": CORE_AGENT_ID})),
        ("certify_agent".into(), json!({"id": CORE_AGENT_ID})),
        ("get_schema".into(), json!({})),
        ("list_models".into(), json!({})),
        (
            "upsert_session".into(),
            json!({
                "agent_id": CORE_AGENT_ID,
                "session_id": "eval-core",
                "title": "core-eval",
                "messages": [
                    {"role": "user", "content": "eval marker JEWELL_CORE_PI"},
                    {"role": "assistant", "content": "acked"}
                ]
            }),
        ),
        (
            "list_sessions".into(),
            json!({"agent_id": CORE_AGENT_ID}),
        ),
        (
            "get_session".into(),
            json!({"agent_id": CORE_AGENT_ID, "session_id": "eval-core"}),
        ),
        // search mode (merged into list_sessions)
        (
            "list_sessions".into(),
            json!({
                "query": "JEWELL_CORE_PI",
                "agent_id": CORE_AGENT_ID
            }),
        ),
        // mutate tools — dry paths only. Do NOT call run_eval here: core self-eval
        // sets eval depth=1, so nested run_eval would always fail closed.
        (
            "learn".into(),
            json!({"targets": ["tutoring/math-tutor"], "dry_run": true}),
        ),
        (
            "research_models".into(),
            json!({"list_sources_only": true}),
        ),
        (
            "write_agent".into(),
            json!({
                "id": "lab/eval-write-probe",
                "markdown": "---\nschema_version: 1\nname: eval-write-probe\ndescription: probe\ndefault_model: qwen3-0.6b\nrole: agent\ntools:\n  - list_tools\n---\n\nprobe body MUST_EVAL\n",
                "require_eval": false
            }),
        ),
    ]
}

pub fn run_tool_plan_case(ctx: &ToolContext, gt: &GroundTruth) -> CaseResult {
    let t0 = std::time::Instant::now();
    let plan = full_introspection_plan(gt);
    let run = run_tool_plan(ctx, &plan);
    let tool_names = all_tool_names();
    // require every registered tool except run_eval (self-eval depth guard)
    let require: Vec<&str> = tool_names
        .iter()
        .map(|s| s.as_str())
        .filter(|n| *n != "run_eval")
        .collect();
    let mut facts = score_introspection(gt, &run, &require);

    // Core allowlist matches registry and frontmatter
    let reg: BTreeSet<_> = tool_names.iter().cloned().collect();
    let claimed: BTreeSet<_> = CORE_AGENT_TOOLS.iter().map(|s| (*s).to_string()).collect();
    facts.push(fact(
        "lean_registry_eq_core_claims",
        "registered tools equal CORE_AGENT_TOOLS",
        reg == claimed,
        format!("{:?}", claimed),
        format!("{:?}", reg),
    ));
    facts.push(fact(
        "frontmatter_tools_eq_claims",
        "orchestrator frontmatter tools equal CORE_AGENT_TOOLS",
        {
            let fm: BTreeSet<_> = gt.orchestrator_tools.iter().cloned().collect();
            fm == claimed
        },
        format!("{:?}", claimed),
        format!("{:?}", gt.orchestrator_tools),
    ));

    // Search-via-list
    let search_ok = run
        .tool_rounds
        .iter()
        .filter(|r| r.call.name == "list_sessions")
        .any(|r| {
            r.result.ok
                && r.result.result.get("mode").and_then(|m| m.as_str()) == Some("search")
                && r.result
                    .result
                    .get("count")
                    .and_then(|c| c.as_u64())
                    .unwrap_or(0)
                    >= 1
        });
    facts.push(fact(
        "session_search_via_list",
        "list_sessions query mode finds eval marker",
        search_ok,
        "search count>=1",
        if search_ok { "ok" } else { "missing" },
    ));
    // run_eval is registered (not invoked in self-eval plan — avoids depth re-entry)
    facts.push(fact(
        "run_eval_registered",
        "run_eval in registry",
        tool_names.iter().any(|n| n == "run_eval"),
        "run_eval",
        if tool_names.iter().any(|n| n == "run_eval") {
            "run_eval"
        } else {
            "missing"
        },
    ));

    let correct = facts.iter().all(|f| f.correct) && run.all_tool_ok();
    CaseResult {
        id: "core_orchestrator_tool_plan".into(),
        prompt: "Run lean core tool plan as core/orchestrator.".into(),
        track: "tool_plan".into(),
        gold: gt.to_json(),
        response: run.final_text.clone(),
        tools_called: run.tool_names_called(),
        tool_results_ok: run.all_tool_ok(),
        facts,
        correct,
        grader: "rubric".into(),
        t_ms: t0.elapsed().as_millis(),
    }
}
