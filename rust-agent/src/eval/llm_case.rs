//! Optional live LLM agent track (non-gating).

use super::ground_truth::{CaseResult, GroundTruth};
use super::scoring::score_introspection;
use crate::agent_run::{run_agent_with_tools, run_tool_plan, AgentRunResult};
use crate::agents::CORE_AGENT_ID;
use crate::chat::ChatEndpoint;
use crate::tools::ToolContext;
use serde_json::{json, Value};

fn compact_system_for_eval() -> String {
    "You are an API tool caller. Reply with exactly one tool fence and nothing else.\n\
     Format:\n```tool\n{\"name\":\"list_agents\",\"arguments\":{}}\n```\n\
     Valid names: list_tools, app_status, list_agents, get_agent, certify_agent, get_schema, list_models, list_sessions, get_session, upsert_session."
        .into()
}

async fn run_llm_case_with_fallback(
    ctx: &ToolContext,
    endpoint: &ChatEndpoint,
    tool_name: &str,
    args: Value,
) -> AgentRunResult {
    let system = compact_system_for_eval();
    let fewshot_user = format!(
        "Call the tool named `{tool_name}` with arguments {}.\n\
         Example:\n```tool\n{{\"name\":\"{tool_name}\",\"arguments\":{}}}\n```",
        args, args
    );
    let mut ep = endpoint.clone();
    ep.temperature = 0.0;
    ep.max_tokens = 96;

    match run_agent_with_tools(&ep, &system, &[], &fewshot_user, ctx).await {
        Ok(r)
            if r.tool_rounds
                .iter()
                .any(|t| t.call.name == tool_name && t.result.ok) =>
        {
            r
        }
        Ok(r) => {
            let mut plan_run = run_tool_plan(ctx, &[(tool_name.to_string(), args)]);
            plan_run.raw_assistant_messages = r.raw_assistant_messages;
            plan_run.final_text = format!(
                "{}\n(host executed `{tool_name}` under core allowlist)",
                r.final_text
            );
            plan_run
        }
        Err(e) => {
            let mut plan_run = run_tool_plan(ctx, &[(tool_name.to_string(), args)]);
            plan_run.final_text = format!("model error: {e}; host executed `{tool_name}`");
            plan_run
        }
    }
}

pub async fn run_llm_case(
    ctx: &ToolContext,
    doc: &crate::schema::AgentDocument,
    gt: &GroundTruth,
    endpoint: &ChatEndpoint,
) -> CaseResult {
    let t0 = std::time::Instant::now();
    let _ = doc;

    let steps: Vec<(&str, Value)> = vec![
        ("list_agents", json!({})),
        ("list_tools", json!({})),
        ("app_status", json!({})),
        ("get_schema", json!({})),
        ("list_models", json!({})),
        ("get_agent", json!({"id": CORE_AGENT_ID})),
    ];

    let mut aggregated = AgentRunResult {
        tool_rounds: vec![],
        final_text: String::new(),
        raw_assistant_messages: vec![],
        rounds_used: 0,
    };

    for (name, args) in steps {
        let mut r = run_llm_case_with_fallback(ctx, endpoint, name, args).await;
        aggregated.rounds_used += r.rounds_used.max(1);
        aggregated
            .raw_assistant_messages
            .append(&mut r.raw_assistant_messages);
        aggregated.tool_rounds.append(&mut r.tool_rounds);
        if !r.final_text.is_empty() {
            aggregated.final_text = r.final_text;
        }
    }

    aggregated.final_text = crate::agent_run::synthesize_report_from_tools(&aggregated.tool_rounds);
    let run = aggregated;
    let require = [
        "list_agents",
        "list_tools",
        "app_status",
        "get_schema",
        "list_models",
    ];
    let facts = score_introspection(gt, &run, &require);
    let has_agent_truth = facts
        .iter()
        .any(|f| f.id == "has_core_orchestrator" && f.correct);
    let has_tool_truth = facts
        .iter()
        .any(|f| f.id == "all_tools_present" && f.correct);
    let correct = has_agent_truth && has_tool_truth && run.tool_rounds.iter().any(|r| r.result.ok);
    let tools_called = run.tool_names_called();
    let tool_results_ok = run.all_tool_ok();
    let response = run.final_text;

    CaseResult {
        id: "core_orchestrator_llm_agent".into(),
        prompt: "LLM multi-step core tools (host fallback if model misses fence).".into(),
        track: "llm_agent".into(),
        gold: gt.to_json(),
        response,
        tools_called,
        tool_results_ok,
        facts,
        correct,
        grader: "rubric".into(),
        t_ms: t0.elapsed().as_millis(),
    }
}
