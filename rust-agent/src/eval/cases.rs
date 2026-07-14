//! Tool-plan eval case and shared plan helpers.

use super::ground_truth::{CaseResult, GroundTruth};
use super::scoring::{fact, score_introspection};
use crate::agent_run::run_tool_plan;
use crate::agents::{
    load_agent, AGENT_IMPLEMENTOR_ID, CORE_AGENT_ID, CORE_AGENT_TOOLS, LEARNER_ID,
    TEAM_AGENT_IDS, TOOL_IMPLEMENTOR_ID,
};
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

/// Tools the orch plan must invoke (excludes run_eval depth + live run_agent).
pub fn core_plan_require_tools() -> Vec<&'static str> {
    CORE_AGENT_TOOLS
        .iter()
        .copied()
        .filter(|n| *n != "run_eval")
        .collect()
}

/// Full lean orch plan: every CORE tool with safe args (dry_run for run_agent).
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
        ("list_sessions".into(), json!({"agent_id": CORE_AGENT_ID})),
        (
            "get_session".into(),
            json!({"agent_id": CORE_AGENT_ID, "session_id": "eval-core"}),
        ),
        (
            "list_sessions".into(),
            json!({
                "query": "JEWELL_CORE_PI",
                "agent_id": CORE_AGENT_ID
            }),
        ),
        // Nested LLM not required for CI — dry_run loads specialist metadata
        (
            "run_agent".into(),
            json!({"agent_id": LEARNER_ID, "dry_run": true}),
        ),
        // Sandbox FS probe (agents/core/orchestrator/fs/)
        (
            "fs_write".into(),
            json!({"path": "eval-probe.txt", "content": "JEWELL_FS_OK"}),
        ),
        ("fs_read".into(), json!({"path": "eval-probe.txt"})),
        ("fs_list".into(), json!({})),
        // Do NOT call run_eval here: core self-eval sets eval depth=1
    ]
}

pub fn run_tool_plan_case(ctx: &ToolContext, gt: &GroundTruth) -> CaseResult {
    let t0 = std::time::Instant::now();
    let plan = full_introspection_plan(gt);
    let run = run_tool_plan(ctx, &plan);
    let require = core_plan_require_tools();
    let mut facts = score_introspection(gt, &run, &require);

    let reg: BTreeSet<_> = all_tool_names().into_iter().collect();
    let claimed: BTreeSet<_> = CORE_AGENT_TOOLS.iter().map(|s| (*s).to_string()).collect();
    facts.push(fact(
        "core_subset_of_registry",
        "CORE_AGENT_TOOLS ⊆ full registry",
        claimed.is_subset(&reg),
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
    facts.push(fact(
        "run_eval_registered",
        "run_eval in CORE_AGENT_TOOLS",
        claimed.contains("run_eval"),
        "run_eval",
        if claimed.contains("run_eval") {
            "run_eval"
        } else {
            "missing"
        },
    ));
    facts.push(fact(
        "run_agent_dry",
        "run_agent dry_run loads learner metadata",
        run.result_for("run_agent")
            .map(|v| v.get("dry_run").and_then(|d| d.as_bool()) == Some(true))
            .unwrap_or(false),
        "dry_run true",
        "see run_agent result",
    ));

    // Fixed team present
    let team: BTreeSet<_> = TEAM_AGENT_IDS.iter().map(|s| (*s).to_string()).collect();
    facts.push(fact(
        "team_agents_present",
        "all four team agents listed",
        team.is_subset(&gt.agent_ids),
        format!("{:?}", team),
        format!("{:?}", gt.agent_ids),
    ));
    facts.push(fact(
        "team_ids_known",
        "learner + implementors exist as constants",
        [LEARNER_ID, AGENT_IMPLEMENTOR_ID, TOOL_IMPLEMENTOR_ID]
            .iter()
            .all(|id| gt.agent_ids.contains(*id)),
        "system/*",
        format!("{:?}", gt.agent_ids),
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
