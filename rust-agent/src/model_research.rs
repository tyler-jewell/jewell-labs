//! Model research job: document public sources + local gate compare. Never promote on rank alone.

use crate::registry::ModelRegistry;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const BENCHMARK_SOURCES: &[(&str, &str, &str)] = &[
    (
        "hf-open-llm-leaderboard",
        "Hugging Face Open LLM Leaderboard",
        "https://huggingface.co/spaces/open-llm-leaderboard/open_llm_leaderboard",
    ),
    (
        "hf-hub-models",
        "Hugging Face Hub models",
        "https://huggingface.co/models",
    ),
    (
        "lmarena",
        "LMSYS Chatbot Arena",
        "https://lmarena.ai/",
    ),
    (
        "local-project-evals",
        "Jewell Labs structural evals",
        "in-repo: evals/ + rust-agent tool_plan",
    ),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResearchRequest {
    pub candidate_id: String,
    pub candidate_path: Option<String>,
    #[serde(default = "default_true")]
    pub offline: bool,
    #[serde(default)]
    pub apply: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateMetric {
    pub name: String,
    pub baseline: f64,
    pub candidate: f64,
    pub not_worse: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResearchReport {
    pub id: String,
    pub created: String,
    pub sources: Vec<SourceRef>,
    pub baseline_model: String,
    pub candidate_id: String,
    pub candidate_path: Option<String>,
    pub network_status: String,
    pub metrics: Vec<GateMetric>,
    pub decision: String,
    pub reason: String,
    pub registry_changed: bool,
    pub report_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRef {
    pub id: String,
    pub name: String,
    pub url: String,
}

pub fn run_model_research(
    registry_path: impl AsRef<Path>,
    req: &ModelResearchRequest,
) -> ModelResearchReport {
    let sources: Vec<SourceRef> = BENCHMARK_SOURCES
        .iter()
        .map(|(id, name, url)| SourceRef {
            id: (*id).into(),
            name: (*name).into(),
            url: (*url).into(),
        })
        .collect();

    let reg = ModelRegistry::load(registry_path.as_ref());
    let (baseline_model, baseline_ok, baseline_path_ok) = match &reg {
        Ok(r) => {
            let key = r.models.keys().next().cloned().unwrap_or_else(|| "unknown".into());
            match r.resolve(&key) {
                Ok(m) => (key, true, m.path.exists()),
                Err(_) => (key, false, false),
            }
        }
        Err(_) => ("unreadable".into(), false, false),
    };

    let cand_ok = req
        .candidate_path
        .as_ref()
        .map(|p| PathBuf::from(p).exists())
        .unwrap_or(false)
        || reg
            .as_ref()
            .map(|r| r.contains(&req.candidate_id))
            .unwrap_or(false);

    let network_status = if req.offline {
        "offline_mode".into()
    } else {
        "probe_deferred_use_offline_or_cli".into()
    };

    let structural = structural_smoke();
    let metrics: Vec<GateMetric> = vec![
        GateMetric {
            name: "baseline_resolves".into(),
            baseline: b(baseline_ok),
            candidate: b(baseline_ok),
            not_worse: baseline_ok,
        },
        GateMetric {
            name: "baseline_weights_present".into(),
            baseline: b(baseline_path_ok),
            candidate: b(baseline_path_ok),
            not_worse: true,
        },
        GateMetric {
            name: "candidate_weights_present".into(),
            baseline: 1.0,
            candidate: b(cand_ok),
            not_worse: cand_ok,
        },
        GateMetric {
            name: "project_tool_plan_smoke".into(),
            baseline: structural,
            candidate: structural,
            not_worse: structural >= 1.0,
        },
    ];

    let all_ok = metrics.iter().all(|m| m.not_worse) && baseline_ok;
    let (decision, reason, registry_changed) = decide(req, all_ok, cand_ok, &baseline_model, registry_path.as_ref());

    ModelResearchReport {
        id: format!("model-research-{}", Utc::now().format("%Y%m%dT%H%M%SZ")),
        created: Utc::now().to_rfc3339(),
        sources,
        baseline_model,
        candidate_id: req.candidate_id.clone(),
        candidate_path: req.candidate_path.clone(),
        network_status,
        metrics,
        decision,
        reason,
        registry_changed,
        report_path: None,
    }
}

fn b(x: bool) -> f64 {
    if x {
        1.0
    } else {
        0.0
    }
}

fn decide(
    req: &ModelResearchRequest,
    all_ok: bool,
    cand_ok: bool,
    baseline: &str,
    registry_path: &Path,
) -> (String, String, bool) {
    if !all_ok {
        return (
            "reject".into(),
            "gate metrics not all not_worse or baseline broken".into(),
            false,
        );
    }
    if req.candidate_id == baseline {
        return ("reject".into(), "candidate is already baseline".into(), false);
    }
    if !cand_ok {
        return (
            "reject".into(),
            "candidate path missing and not in registry".into(),
            false,
        );
    }
    if req.apply {
        match apply_registry(registry_path, &req.candidate_id, req.candidate_path.as_deref()) {
            Ok(()) => (
                "promote".into(),
                "local gates passed; registry updated".into(),
                true,
            ),
            Err(e) => ("reject".into(), format!("registry write failed: {e}"), false),
        }
    } else {
        (
            "eligible".into(),
            "gates passed; apply=false so registry unchanged".into(),
            false,
        )
    }
}

fn structural_smoke() -> f64 {
    let agents = crate::paths::agents_dir();
    if crate::agents::load_agent(&agents, crate::agents::CORE_AGENT_ID).is_err() {
        return 0.0;
    }
    let (ctx, _) = crate::eval::orchestrator_ctx();
    let gt = crate::eval::GroundTruth::collect(&ctx);
    if crate::eval::run_tool_plan_case(&ctx, &gt).correct {
        1.0
    } else {
        0.0
    }
}

fn apply_registry(path: &Path, id: &str, cand: Option<&str>) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let cand = cand.ok_or_else(|| "candidate path required".to_string())?;
    if !PathBuf::from(cand).exists() {
        return Err("candidate path does not exist".into());
    }
    if text.contains(&format!("{id}:")) {
        return Err("key exists; refuse silent overwrite".into());
    }
    if !text.contains("models:") {
        return Err("registry missing models:".into());
    }
    let block = format!(
        "  {id}:\n    path: {cand}\n    alias: {id}\n    defaults:\n      ctx: 2048\n      reasoning: \"off\"\n"
    );
    let tmp = path.with_extension("yaml.tmp");
    std::fs::write(&tmp, format!("{}\n{block}", text.trim_end())).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

pub fn write_research_report(
    report: &ModelResearchReport,
    path: impl Into<PathBuf>,
) -> std::io::Result<PathBuf> {
    let path = path.into();
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let mut r = report.clone();
    r.report_path = Some(path.display().to_string());
    std::fs::write(&path, serde_json::to_string_pretty(&r)?)?;
    Ok(path)
}
