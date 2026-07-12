//! Full eval orchestration and report persistence.

use super::cases::{orchestrator_ctx, run_tool_plan_case};
use super::llm_case::run_llm_case;
use super::ground_truth::{CaseResult, EvalReport, EvalSummary, GroundTruth};
use super::scoring::fact;
use crate::chat::ChatEndpoint;
use crate::paths::registry_path;
use crate::registry::ModelRegistry;
use chrono::Utc;
use std::collections::BTreeSet;
use std::path::PathBuf;

pub async fn run_full_eval(include_llm: bool) -> EvalReport {
    let (ctx, doc) = orchestrator_ctx();
    let gt = GroundTruth::collect(&ctx);

    let mut cases = vec![run_tool_plan_case(&ctx, &gt)];
    let mut tracks = vec!["tool_plan".to_string()];
    let mut model = None;
    let mut server = None;

    if include_llm {
        if let Ok(reg) = ModelRegistry::load(registry_path()) {
            if let Ok(resolved) = reg.resolve(&doc.frontmatter.default_model) {
                let endpoint = ChatEndpoint::from_agent(&doc, &resolved);
                server = Some(endpoint.base_url.clone());
                model = Some(resolved.alias.clone());
                let client = reqwest::Client::new();
                let ok = client
                    .get(format!("{}/v1/models", endpoint.base_url))
                    .send()
                    .await
                    .map(|r| r.status().is_success())
                    .unwrap_or(false);
                if ok {
                    cases.push(run_llm_case(&ctx, &doc, &gt, &endpoint).await);
                    tracks.push("llm_agent".into());
                } else {
                    cases.push(CaseResult {
                        id: "core_orchestrator_llm_agent".into(),
                        prompt: "skipped — llama-server not reachable".into(),
                        track: "llm_agent".into(),
                        gold: gt.to_json(),
                        response: format!("server unreachable at {}", endpoint.base_url),
                        tools_called: vec![],
                        tool_results_ok: false,
                        facts: vec![fact(
                            "server",
                            "llama-server reachable",
                            false,
                            "up",
                            "down",
                        )],
                        correct: false,
                        grader: "rubric".into(),
                        t_ms: 0,
                    });
                    tracks.push("llm_agent".into());
                }
                let _ = resolved;
            }
        }
    }

    let total = cases.len();
    let correct = cases.iter().filter(|c| c.correct).count();
    let tool_plan_acc = cases
        .iter()
        .filter(|c| c.track == "tool_plan")
        .map(|c| if c.correct { 1.0 } else { 0.0 })
        .sum::<f64>()
        / cases
            .iter()
            .filter(|c| c.track == "tool_plan")
            .count()
            .max(1) as f64;
    let llm_cases: Vec<_> = cases.iter().filter(|c| c.track == "llm_agent").collect();
    let llm_acc = if llm_cases.is_empty() {
        None
    } else {
        Some(llm_cases.iter().filter(|c| c.correct).count() as f64 / llm_cases.len() as f64)
    };

    let plan = cases.iter().find(|c| c.track == "tool_plan");
    let all_tools = plan
        .map(|c| {
            let called: BTreeSet<_> = c.tools_called.iter().cloned().collect();
            gt.tool_names.is_subset(&called) && c.correct
        })
        .unwrap_or(false);

    EvalReport {
        id: format!(
            "agent-introspection-{}",
            Utc::now().format("%Y%m%dT%H%M%SZ")
        ),
        created: Utc::now().to_rfc3339(),
        agent: crate::agents::CORE_AGENT_ID.into(),
        tracks,
        model,
        server,
        ground_truth: gt.to_json(),
        cases,
        summary: EvalSummary {
            total,
            correct,
            accuracy: if total == 0 {
                0.0
            } else {
                correct as f64 / total as f64
            },
            tool_plan_accuracy: tool_plan_acc,
            llm_agent_accuracy: llm_acc,
            all_tools_invoked_in_plan: all_tools,
            complete_introspection: all_tools,
        },
    }
}

pub fn write_report(report: &EvalReport, path: impl Into<PathBuf>) -> std::io::Result<PathBuf> {
    let path = path.into();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(report).unwrap();
    std::fs::write(&path, text)?;
    Ok(path)
}
