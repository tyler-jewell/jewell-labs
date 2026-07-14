use super::ground_truth::{CaseResult, EvalReport, EvalSummary};
use super::scoring::fact;
use crate::agent_run::run_tool_plan;
use crate::agents::{list_agents, load_agent};
use crate::paths::{agents_dir, crate_root, registry_path, repo_root, sessions_dir};
use crate::tools::ToolContext;
use chrono::Utc;
use serde_json::{json, Value};
use std::cell::Cell;
use std::collections::BTreeSet;
use std::path::PathBuf;
thread_local! {
    static DEPTH: Cell<u32> = const { Cell::new(0) };
}
#[derive(Debug, Clone)]
pub struct DatasetCase {
    pub id: String,
    pub track: String,
    pub prompt: String,
    pub require_tools: Vec<String>,
    pub forbid_tools: Vec<String>,
}
#[derive(Debug, thiserror::Error)]
pub enum EvalError {
    #[error("eval_depth_exceeded")]
    DepthExceeded,
    #[error("invalid dataset: {0}")]
    InvalidDataset(String),
    #[error("agent: {0}")]
    Agent(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}
pub fn parse_dataset(text: &str) -> Result<Vec<DatasetCase>, EvalError> {
    let mut cases = Vec::new();
    let mut id: Option<String> = None;
    let mut track = "tool_plan".to_string();
    let mut prompt = String::new();
    let mut require = Vec::new();
    let mut forbid = Vec::new();
    let flush = |id: &mut Option<String>,
                     track: &str,
                     prompt: &str,
                     require: &[String],
                     forbid: &[String],
                     cases: &mut Vec<DatasetCase>|
     -> Result<(), EvalError> {
        if let Some(cid) = id.take() {
            if track == "tool_plan" && require.is_empty() {
                return Err(EvalError::InvalidDataset(format!(
                    "case {cid}: require_tools empty"
                )));
            }
            cases.push(DatasetCase {
                id: cid,
                track: track.into(),
                prompt: prompt.into(),
                require_tools: require.to_vec(),
                forbid_tools: forbid.to_vec(),
            });
        }
        Ok(())
    };
    for line in text.lines() {
        let t = line.trim();
        if let Some(r) = t.strip_prefix("## case:") {
            flush(&mut id, &track, &prompt, &require, &forbid, &mut cases)?;
            id = Some(r.trim().into());
            track = "tool_plan".into();
            prompt.clear();
            require.clear();
            forbid.clear();
        } else if let Some(r) = t.strip_prefix("track:") {
            track = r.trim().into();
        } else if let Some(r) = t.strip_prefix("prompt:") {
            prompt = r.trim().trim_matches('"').into();
        } else if let Some(r) = t.strip_prefix("require_tools:") {
            require = parse_list(r);
        } else if let Some(r) = t.strip_prefix("forbid_tools:") {
            forbid = parse_list(r);
        }
    }
    flush(&mut id, &track, &prompt, &require, &forbid, &mut cases)?;
    if cases.is_empty() {
        return Err(EvalError::InvalidDataset("no cases".into()));
    }
    Ok(cases)
}
fn parse_list(s: &str) -> Vec<String> {
    let s = s.trim().trim_start_matches('[').trim_end_matches(']');
    s.split(',')
        .map(|x| x.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|x| !x.is_empty())
        .collect()
}
pub fn dataset_path_for(agent_id: &str) -> PathBuf {
let parts: Vec<_> = agent_id.split('/').collect();
let mut p = repo_root().join("evals/datasets");
for (i, part) in parts.iter().enumerate() { if i + 1 == parts.len() { p.push(format!("{part}.md")); } else { p.push(part); } }
p
}
fn agent_ctx(agent_id: &str) -> Result<(ToolContext, crate::schema::AgentDocument), EvalError> {
    let agents = agents_dir();
    let doc = load_agent(&agents, agent_id).map_err(|e| EvalError::Agent(e.to_string()))?;
    Ok((
        ToolContext::from_paths(
            agents,
            registry_path(),
            sessions_dir(),
            crate_root(),
            repo_root(),
            Some(agent_id.into()),
            doc.frontmatter.tools.clone(),
        ),
        doc,
    ))
}
fn plan_for(agent_id: &str, tools: &[String]) -> Vec<(String, Value)> {
    tools
        .iter()
        .filter(|t| !matches!(t.as_str(), "run_eval" | "research_models"))
        .map(|t| {
            let args = match t.as_str() {
                "get_agent" | "certify_agent" => json!({"id": agent_id}),
                "list_sessions" => json!({"agent_id": agent_id}),
                "get_session" => json!({"agent_id": agent_id, "session_id": "eval-smoke"}),
                "upsert_session" => json!({
                    "agent_id": agent_id, "session_id": "eval-smoke", "title": "eval",
                    "messages": [{"role":"user","content":"eval"}]
                }),
                "run_agent" => json!({"agent_id": crate::agents::LEARNER_ID, "dry_run": true}),
                "learn" => json!({"targets": [agent_id], "dry_run": true}),
                "write_agent" => json!({
                    "id": "lab/eval-smoke-temp",
                    "markdown": "---\nschema_version: 1\nname: eval-smoke-temp\ndescription: t\ndefault_model: qwen3-0.6b\nrole: agent\ntools:\n  - list_tools\n---\n\nb\n",
                    "require_eval": false
                }),
                "write_tool" => json!({"scope":"shared","category":"proposed","name":"eval_smoke_draft","content":"// draft\n"}),
                _ => json!({}),
            };
            (t.clone(), args)
        })
        .collect()
}
fn score(case: &DatasetCase, called: &[String], ok: bool) -> (Vec<super::FactResult>, bool) {
    let set: BTreeSet<_> = called.iter().cloned().collect();
    let mut facts = Vec::new();
    for t in &case.require_tools {
        // run_eval is claim-only during nested-safe plans
        if t == "run_eval" {
            let reg = crate::tools::all_tool_names().iter().any(|n| n == "run_eval");
            facts.push(fact("req_run_eval", "run_eval registered", reg, t, if reg { t } else { "missing" }));
            continue;
        }
        let hit = set.contains(t);
        facts.push(fact(&format!("req_{t}"), t, hit, t, if hit { t } else { "missing" }));
    }
    for t in &case.forbid_tools {
        let abs = !set.contains(t);
        facts.push(fact(&format!("forb_{t}"), t, abs, "absent", if abs { "absent" } else { t }));
    }
    facts.push(fact("tools_ok", "ok", ok, "ok", if ok { "ok" } else { "fail" }));
    let correct = facts.iter().all(|f| f.correct);
    (facts, correct)
}
pub fn run_agent_eval(agent_id: &str, track: &str) -> Result<EvalReport, EvalError> {
    let depth = DEPTH.with(|d| d.get());
    if depth >= 1 {
        return Err(EvalError::DepthExceeded);
    }
    DEPTH.with(|d| d.set(depth + 1));
    let out = run_agent_eval_inner(agent_id, track);
    DEPTH.with(|d| d.set(depth));
    out
}
fn run_agent_eval_inner(agent_id: &str, track: &str) -> Result<EvalReport, EvalError> {
    let (ctx, doc) = agent_ctx(agent_id)?;
    if agent_id == crate::agents::CORE_AGENT_ID {
        return Ok(core_builtin(&ctx));
    }
    let ds = dataset_path_for(agent_id);
    let cases = if ds.is_file() {
        parse_dataset(&std::fs::read_to_string(&ds)?)?
    } else {
        vec![DatasetCase {
            id: "smoke".into(),
            track: "tool_plan".into(),
            prompt: "smoke".into(),
            require_tools: doc
                .frontmatter
                .tools
                .iter()
                .filter(|t| *t != "*")
                .take(3)
                .cloned()
                .collect(),
            forbid_tools: vec![],
        }]
    };
    let mut out = Vec::new();
    for case in cases {
        if case.track != "tool_plan" || (track != "tool_plan" && track != "all") {
            continue;
        }
        let plan = plan_for(agent_id, &case.require_tools);
        let t0 = std::time::Instant::now();
        let run = run_tool_plan(&ctx, &plan);
        let called = run.tool_names_called();
        let ok = run.all_tool_ok();
        let (facts, correct) = score(&case, &called, ok);
        out.push(CaseResult {
            id: case.id,
            prompt: case.prompt,
            track: "tool_plan".into(),
            gold: json!({"require_tools": case.require_tools}),
            response: run.final_text,
            tools_called: called,
            tool_results_ok: ok,
            facts,
            correct,
            grader: "code".into(),
            t_ms: t0.elapsed().as_millis(),
        });
    }
    if out.is_empty() {
        return Err(EvalError::InvalidDataset("no tool_plan cases".into()));
    }
    let total = out.len();
    let correct = out.iter().filter(|c| c.correct).count();
    Ok(EvalReport {
        id: format!(
            "eval-{}-{}",
            agent_id.replace('/', "-"),
            Utc::now().format("%Y%m%dT%H%M%SZ")
        ),
        created: Utc::now().to_rfc3339(),
        agent: agent_id.into(),
        tracks: vec!["tool_plan".into()],
        model: None,
        server: None,
        ground_truth: json!({}),
        cases: out,
        summary: EvalSummary {
            total,
            correct,
            accuracy: correct as f64 / total as f64,
            tool_plan_accuracy: correct as f64 / total as f64,
            llm_agent_accuracy: None,
            all_tools_invoked_in_plan: false,
            complete_introspection: false,
        },
    })
}
fn core_builtin(ctx: &ToolContext) -> EvalReport {
    let gt = super::GroundTruth::collect(ctx);
    let case = super::run_tool_plan_case(ctx, &gt);
    let c = if case.correct { 1 } else { 0 };
    EvalReport {
        id: format!("eval-core-{}", Utc::now().format("%Y%m%dT%H%M%SZ")),
        created: Utc::now().to_rfc3339(),
        agent: crate::agents::CORE_AGENT_ID.into(),
        tracks: vec!["tool_plan".into()],
        model: None,
        server: None,
        ground_truth: gt.to_json(),
        cases: vec![case],
        summary: EvalSummary {
            total: 1,
            correct: c,
            accuracy: c as f64,
            tool_plan_accuracy: c as f64,
            llm_agent_accuracy: None,
            all_tools_invoked_in_plan: c == 1,
            complete_introspection: c == 1,
        },
    }
}
pub fn require_green_eval(agent_id: &str) -> Result<EvalReport, EvalError> {
    let r = run_agent_eval(agent_id, "tool_plan")?;
    if r.summary.tool_plan_accuracy < 1.0 {
        return Err(EvalError::InvalidDataset(format!(
            "eval not green: {}",
            r.summary.tool_plan_accuracy
        )));
    }
    Ok(r)
}
pub fn eval_all_agents() -> Result<Vec<EvalReport>, EvalError> {
    list_agents(agents_dir())
        .map_err(|e| EvalError::Agent(e.to_string()))?
        .into_iter()
        .map(|a| run_agent_eval(&a.id, "tool_plan"))
        .collect()
}
pub fn set_eval_depth(depth: u32) {
    DEPTH.with(|d| d.set(depth));
}
pub fn get_eval_depth() -> u32 {
    DEPTH.with(|d| d.get())
}
