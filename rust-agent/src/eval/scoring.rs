//! Score agent runs against ground truth.

use super::ground_truth::{FactResult, GroundTruth};
use crate::agent_run::AgentRunResult;
use crate::agents::CORE_AGENT_ID;
use serde_json::json;
use std::collections::BTreeSet;

pub(crate) fn fact(
    id: &str,
    desc: &str,
    correct: bool,
    expected: impl ToString,
    got: impl ToString,
) -> FactResult {
    FactResult {
        id: id.into(),
        description: desc.into(),
        correct,
        expected: expected.to_string(),
        got: got.to_string(),
    }
}

fn set_eq(a: &BTreeSet<String>, b: &BTreeSet<String>) -> bool {
    a == b
}

/// Score a run against ground truth using tool results.
pub fn score_introspection(
    gt: &GroundTruth,
    run: &AgentRunResult,
    require_tools: &[&str],
) -> Vec<FactResult> {
    let mut facts = Vec::new();
    let called: BTreeSet<String> = run.tool_names_called().into_iter().collect();

    for t in require_tools {
        facts.push(fact(
            &format!("called_{t}"),
            &format!("agent called tool {t}"),
            called.contains(*t),
            t.to_string(),
            if called.contains(*t) {
                "called"
            } else {
                "missing"
            },
        ));
    }

    if let Some(v) = run.result_for("list_agents") {
        let got: BTreeSet<String> = v
            .get("agents")
            .and_then(|a| a.as_array())
            .into_iter()
            .flatten()
            .filter_map(|a| a.get("id").and_then(|x| x.as_str()).map(|s| s.to_string()))
            .collect();
        // Team must be present; transient lab/* fixtures allowed during concurrent tests
        let team: BTreeSet<_> = crate::agents::TEAM_AGENT_IDS
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        facts.push(fact(
            "agent_ids",
            "list_agents includes fixed team",
            team.is_subset(&got),
            format!("{:?}", team),
            format!("{:?}", got),
        ));
        facts.push(fact(
            "has_core_orchestrator",
            "includes core/orchestrator",
            got.contains(CORE_AGENT_ID),
            CORE_AGENT_ID,
            format!("{:?}", got),
        ));
    }

    if let Some(v) = run.result_for("list_tools") {
        let got: BTreeSet<String> = v
            .get("tools")
            .and_then(|a| a.as_array())
            .into_iter()
            .flatten()
            .filter_map(|t| t.get("name").and_then(|x| x.as_str()).map(|s| s.to_string()))
            .collect();
        // gt.tool_names = allowlist view for this agent
        let missing: Vec<_> = gt.tool_names.difference(&got).cloned().collect();
        facts.push(fact(
            "all_tools_present",
            "list_tools covers agent allowlist",
            missing.is_empty(),
            "[]",
            format!("{:?}", missing),
        ));
    }

    if let Some(v) = run.result_for("app_status") {
        let ac = v.get("agent_count").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
        let tc = v
            .get("builtin_tool_count")
            .and_then(|x| x.as_u64())
            .unwrap_or(0) as usize;
        let registry_n = crate::tools::all_tool_names().len();
        facts.push(fact(
            "agent_count",
            "app_status.agent_count >= fixed team size",
            ac >= crate::agents::TEAM_AGENT_IDS.len(),
            format!(">= {}", crate::agents::TEAM_AGENT_IDS.len()),
            ac,
        ));
        // app_status reports full registry size; gt.tool_count is allowlist size
        facts.push(fact(
            "tool_count",
            "app_status.builtin_tool_count == registry",
            tc == registry_n,
            registry_n,
            tc,
        ));
        let reg = v
            .get("registry_present")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        facts.push(fact(
            "registry_present",
            "registry file present",
            reg == gt.registry_present,
            gt.registry_present,
            reg,
        ));
    }

    if let Some(v) = run.result_for("get_schema") {
        let ver = v
            .get("latest_schema_version")
            .and_then(|x| x.as_u64())
            .unwrap_or(0) as u32;
        facts.push(fact(
            "schema_version",
            "latest schema version",
            ver == gt.schema_version,
            gt.schema_version,
            ver,
        ));
        let fields = v.get("fields").cloned().unwrap_or(json!({}));
        facts.push(fact(
            "schema_fields",
            "schema documents tools + default_model",
            fields.get("tools").is_some() && fields.get("default_model").is_some(),
            "tools, default_model",
            format!("{fields}"),
        ));
    }

    if let Some(v) = run.result_for("list_models") {
        let got: BTreeSet<String> = v
            .get("models")
            .and_then(|a| a.as_array())
            .into_iter()
            .flatten()
            .filter_map(|m| m.get("key").and_then(|x| x.as_str()).map(|s| s.to_string()))
            .collect();
        facts.push(fact(
            "model_keys",
            "registry model keys match",
            set_eq(&got, &gt.model_keys),
            format!("{:?}", gt.model_keys),
            format!("{:?}", got),
        ));
    }

    if let Some(v) = run.result_for("get_agent") {
        let role = v
            .pointer("/frontmatter/role")
            .and_then(|x| x.as_str())
            .unwrap_or("");
        let model = v
            .pointer("/frontmatter/default_model")
            .and_then(|x| x.as_str())
            .unwrap_or("");
        facts.push(fact(
            "orchestrator_role",
            "core agent role",
            role == gt.orchestrator_role && role == "orchestrator",
            "orchestrator",
            role,
        ));
        facts.push(fact(
            "orchestrator_model",
            "core agent default_model",
            model == gt.orchestrator_model,
            &gt.orchestrator_model,
            model,
        ));
    }

    if let Some(v) = run.result_for("certify_agent") {
        let ok = v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
        facts.push(fact(
            "core_agent_cert",
            "core agent certifies against live schema",
            ok == gt.core_agent_cert_ok && ok,
            true,
            ok,
        ));
    }

    facts
}
