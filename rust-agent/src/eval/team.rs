//! Structural team collaboration gate (no LLM).

use super::ground_truth::CaseResult;
use super::scoring::fact;
use crate::agents::{
    load_agent, AGENT_IMPLEMENTOR_ID, CORE_AGENT_ID, CORE_AGENT_TOOLS, LEARNER_ID, TEAM_AGENT_IDS,
    TOOL_IMPLEMENTOR_ID,
};
use crate::paths::{agents_dir, crate_root, registry_path, repo_root, sessions_dir};
use crate::tools::{invoke_tool, ToolContext};
use serde_json::json;
use std::collections::BTreeSet;

fn ctx_for(agent_id: &str) -> Result<ToolContext, String> {
    let agents = agents_dir();
    let doc = load_agent(&agents, agent_id).map_err(|e| e.to_string())?;
    Ok(ToolContext::from_paths(
        agents,
        registry_path(),
        sessions_dir(),
        crate_root(),
        repo_root(),
        Some(agent_id.into()),
        doc.frontmatter.tools.clone(),
    ))
}

fn tools_of(id: &str) -> BTreeSet<String> {
    load_agent(agents_dir(), id)
        .map(|d| d.frontmatter.tools.into_iter().collect())
        .unwrap_or_default()
}

fn model_of(id: &str) -> String {
    load_agent(agents_dir(), id)
        .map(|d| d.frontmatter.default_model)
        .unwrap_or_default()
}

/// Host-only collaboration case: partitions + dry pipeline.
pub fn run_team_collaboration_case() -> CaseResult {
    let t0 = std::time::Instant::now();
    let mut facts = Vec::new();
    let ids: BTreeSet<_> = crate::list_agents(agents_dir())
        .unwrap_or_default()
        .into_iter()
        .map(|a| a.id)
        .collect();
    let team: BTreeSet<_> = TEAM_AGENT_IDS.iter().map(|s| (*s).to_string()).collect();
    facts.push(fact(
        "team_four_present",
        "fixed team agents on disk",
        team.is_subset(&ids),
        format!("{team:?}"),
        format!("{ids:?}"),
    ));
    let legacy = ids.iter().any(|id| id.starts_with("tutoring/"));
    facts.push(fact(
        "no_legacy_specialty",
        "no tutoring/* agents",
        !legacy,
        "absent",
        if legacy { "present" } else { "absent" },
    ));

    let orch_tools = tools_of(CORE_AGENT_ID);
    let claimed: BTreeSet<_> = CORE_AGENT_TOOLS.iter().map(|s| (*s).to_string()).collect();
    facts.push(fact(
        "orch_tools",
        "orchestrator tools == CORE_AGENT_TOOLS",
        orch_tools == claimed,
        format!("{claimed:?}"),
        format!("{orch_tools:?}"),
    ));
    facts.push(fact(
        "orch_has_run_agent",
        "orchestrator has run_agent",
        orch_tools.contains("run_agent"),
        "run_agent",
        format!("{orch_tools:?}"),
    ));
    facts.push(fact(
        "orch_no_write",
        "orchestrator lacks write_agent/write_tool/learn",
        !orch_tools.contains("write_agent")
            && !orch_tools.contains("write_tool")
            && !orch_tools.contains("learn"),
        "no write/learn",
        format!("{orch_tools:?}"),
    ));

    let ltools = tools_of(LEARNER_ID);
    facts.push(fact(
        "learner_has_learn",
        "learner owns learn",
        ltools.contains("learn") && !ltools.contains("write_agent"),
        "learn only",
        format!("{ltools:?}"),
    ));
    let atools = tools_of(AGENT_IMPLEMENTOR_ID);
    facts.push(fact(
        "agent_impl_write",
        "agent-implementor owns write_agent",
        atools.contains("write_agent") && !atools.contains("write_tool"),
        "write_agent",
        format!("{atools:?}"),
    ));
    let ttools = tools_of(TOOL_IMPLEMENTOR_ID);
    facts.push(fact(
        "tool_impl_write",
        "tool-implementor owns write_tool",
        ttools.contains("write_tool") && !ttools.contains("write_agent"),
        "write_tool",
        format!("{ttools:?}"),
    ));

    let model_ok = model_of(CORE_AGENT_ID) == "qwen3-4b"
        && model_of(LEARNER_ID) == "qwen3.6-35b"
        && model_of(AGENT_IMPLEMENTOR_ID) == "qwen3-4b"
        && model_of(TOOL_IMPLEMENTOR_ID) == "qwen3-4b";
    facts.push(fact(
        "team_models",
        "models: orch/impl 4b, learner 35b",
        model_ok,
        "qwen3-4b / qwen3.6-35b",
        "see frontmatter",
    ));

    if let Ok(orch_ctx) = ctx_for(CORE_AGENT_ID) {
        let ra = invoke_tool(
            &orch_ctx,
            "run_agent",
            &json!({"agent_id": LEARNER_ID, "dry_run": true}),
        );
        facts.push(fact(
            "orch_run_agent_dry",
            "orch run_agent dry_run learner",
            ra.ok,
            "ok",
            format!("{:?}", ra.result),
        ));
        let denied = invoke_tool(
            &orch_ctx,
            "write_agent",
            &json!({"id": "lab/should-deny", "markdown": "x", "require_eval": false}),
        );
        facts.push(fact(
            "orch_denied_write",
            "orchestrator cannot write_agent",
            !denied.ok,
            "denied",
            if denied.ok { "allowed" } else { "denied" },
        ));
    }

    if let Ok(lctx) = ctx_for(LEARNER_ID) {
        let learn = invoke_tool(
            &lctx,
            "learn",
            &json!({"targets": [AGENT_IMPLEMENTOR_ID], "dry_run": true}),
        );
        facts.push(fact(
            "learner_learn_dry",
            "learner learn dry_run",
            learn.ok,
            "ok",
            format!("{:?}", learn.result),
        ));
    }

    let fixture_id = "lab/team-fixture";
    let fixture_path = agents_dir().join("lab/team-fixture.md");
    let ds = crate::eval::dataset_path_for(fixture_id);
    let _ = std::fs::remove_file(&fixture_path);
    let _ = std::fs::remove_file(&ds);
    if let Ok(actx) = ctx_for(AGENT_IMPLEMENTOR_ID) {
        let md = "---\nschema_version: 1\nname: team-fixture\ndescription: temp\ndefault_model: qwen3-0.6b\nrole: agent\ntools:\n  - list_tools\n---\n\nfixture\n";
        let w = invoke_tool(
            &actx,
            "write_agent",
            &json!({"id": fixture_id, "markdown": md, "require_eval": true}),
        );
        facts.push(fact(
            "agent_impl_publish",
            "agent-implementor green write_agent",
            w.ok && fixture_path.is_file(),
            "ok+file",
            format!("{:?}", w.result),
        ));
    }
    if let Ok(tctx) = ctx_for(TOOL_IMPLEMENTOR_ID) {
        let wt = invoke_tool(
            &tctx,
            "write_tool",
            &json!({"scope":"shared","category":"proposed","name":"team_echo","content":"// draft\n"}),
        );
        facts.push(fact(
            "tool_impl_draft",
            "tool-implementor write_tool draft",
            wt.ok && wt.result.get("registered").and_then(|r| r.as_bool()) == Some(false),
            "draft unregistered",
            format!("{:?}", wt.result),
        ));
    }

    let _ = std::fs::remove_file(&fixture_path);
    let _ = std::fs::remove_file(&ds);
    let _ = std::fs::remove_file(agents_dir().join("lab/eval-smoke-temp.md"));
    let _ = std::fs::remove_dir(agents_dir().join("lab"));

    let correct = facts.iter().all(|f| f.correct);
    CaseResult {
        id: "team_collaboration".into(),
        prompt: "Structural team collaboration pipeline".into(),
        track: "tool_plan".into(),
        gold: json!({"team": TEAM_AGENT_IDS}),
        response: format!("{} facts", facts.len()),
        tools_called: vec![
            "run_agent".into(),
            "learn".into(),
            "write_agent".into(),
            "write_tool".into(),
        ],
        tool_results_ok: correct,
        facts,
        correct,
        grader: "code".into(),
        t_ms: t0.elapsed().as_millis(),
    }
}
