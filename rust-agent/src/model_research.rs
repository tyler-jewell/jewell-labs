//! Model research: HF sources + project micro-eval (local inference). Never promote without both scores.

use crate::model_eval::{score_inference_target, ProjectEvalScore};
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
    ("lmarena", "LMSYS Chatbot Arena", "https://lmarena.ai/"),
    (
        "local-project-evals",
        "Jewell Labs project micro-eval",
        "in-repo: model_eval PROJECT_MICRO_CASES",
    ),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResearchRequest {
    pub candidate_id: String,
    pub candidate_path: Option<String>,
    #[serde(default)]
    pub baseline_path: Option<String>,
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
pub struct ProjectEvalScoreDto {
    pub score: f64,
    pub correct: usize,
    pub total: usize,
    pub model_requested: String,
    pub model_served: String,
    pub base_url: String,
    pub ok: bool,
    pub error: Option<String>,
}
impl From<&ProjectEvalScore> for ProjectEvalScoreDto {
    fn from(s: &ProjectEvalScore) -> Self {
        Self {
            score: s.score,
            correct: s.correct,
            total: s.total,
            model_requested: s.model_requested.clone(),
            model_served: s.model_served.clone(),
            base_url: s.base_url.clone(),
            ok: s.ok,
            error: s.error.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRef {
    pub id: String,
    pub name: String,
    pub url: String,
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
    pub baseline_eval: Option<ProjectEvalScoreDto>,
    pub candidate_eval: Option<ProjectEvalScoreDto>,
    pub metrics: Vec<GateMetric>,
    pub decision: String,
    pub reason: String,
    pub registry_changed: bool,
    pub report_path: Option<String>,
}

pub fn project_tool_plan_score() -> f64 {
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

pub fn model_fitness(path: &Path) -> f64 {
    if path.is_file() {
        1.0
    } else {
        0.0
    }
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
    let (baseline_model, registry_base_path) = match &reg {
        Ok(r) => {
            let key = r
                .models
                .keys()
                .next()
                .cloned()
                .unwrap_or_else(|| "unknown".into());
            let p = r.resolve(&key).ok().map(|m| m.path.display().to_string());
            (key, p)
        }
        Err(_) => ("unreadable".into(), None),
    };
    let base_target = req
        .baseline_path
        .clone()
        .or(registry_base_path)
        .unwrap_or_default();
    let cand_target = req
        .candidate_path
        .clone()
        .or_else(|| {
            reg.as_ref()
                .ok()
                .and_then(|r| r.resolve(&req.candidate_id).ok())
                .map(|m| m.path.display().to_string())
        })
        .unwrap_or_default();

    let base_eval = if base_target.is_empty() {
        ProjectEvalScore::unavailable("", "no baseline target")
    } else {
        score_inference_target(&base_target)
    };
    let cand_eval = if cand_target.is_empty() {
        ProjectEvalScore::unavailable("", "no candidate target")
    } else {
        score_inference_target(&cand_target)
    };

    let infra = project_tool_plan_score();
    let both = base_eval.ok && cand_eval.ok;
    let micro_ok = both && cand_eval.score + 1e-9 >= base_eval.score;
    let metrics = vec![
        GateMetric {
            name: "project_micro_eval".into(),
            baseline: if base_eval.ok { base_eval.score } else { 0.0 },
            candidate: if cand_eval.ok { cand_eval.score } else { 0.0 },
            not_worse: micro_ok,
        },
        GateMetric {
            name: "inference_available_both".into(),
            baseline: if base_eval.ok { 1.0 } else { 0.0 },
            candidate: if cand_eval.ok { 1.0 } else { 0.0 },
            not_worse: both,
        },
        GateMetric {
            name: "project_tool_plan_infra".into(),
            baseline: infra,
            candidate: infra,
            not_worse: infra >= 1.0,
        },
    ];
    let (decision, reason, registry_changed) = decide(
        req,
        both,
        micro_ok,
        infra >= 1.0,
        &baseline_model,
        registry_path.as_ref(),
        &cand_target,
        &base_eval,
        &cand_eval,
    );
    ModelResearchReport {
        id: format!("model-research-{}", Utc::now().format("%Y%m%dT%H%M%SZ")),
        created: Utc::now().to_rfc3339(),
        sources,
        baseline_model,
        candidate_id: req.candidate_id.clone(),
        candidate_path: req.candidate_path.clone(),
        network_status: if req.offline {
            "offline_hf_ok_local_inference_required".into()
        } else {
            "local_inference_required".into()
        },
        baseline_eval: Some((&base_eval).into()),
        candidate_eval: Some((&cand_eval).into()),
        metrics,
        decision,
        reason,
        registry_changed,
        report_path: None,
    }
}

fn decide(
    req: &ModelResearchRequest,
    both: bool,
    micro_ok: bool,
    infra_ok: bool,
    baseline: &str,
    registry_path: &Path,
    cand_target: &str,
    base_eval: &ProjectEvalScore,
    cand_eval: &ProjectEvalScore,
) -> (String, String, bool) {
    if req.candidate_id == baseline && req.baseline_path.is_none() {
        return (
            "reject".into(),
            "candidate is already baseline key".into(),
            false,
        );
    }
    if !infra_ok {
        return (
            "reject".into(),
            "host project_tool_plan_infra red".into(),
            false,
        );
    }
    if !both {
        return (
            "reject".into(),
            format!(
                "inference_unavailable base_err={:?} cand_err={:?}",
                base_eval.error, cand_eval.error
            ),
            false,
        );
    }
    if !micro_ok {
        return (
            "reject".into(),
            format!(
                "project_micro_eval regression cand={:.3} < base={:.3}",
                cand_eval.score, base_eval.score
            ),
            false,
        );
    }
    if req.apply {
        if cand_target.starts_with("ollama:") {
            return (
                "reject".into(),
                "apply unsupported for ollama: targets".into(),
                false,
            );
        }
        return match apply_registry(registry_path, &req.candidate_id, Some(cand_target)) {
            Ok(()) => (
                "promote".into(),
                format!(
                    "project_micro cand={:.3} >= base={:.3}; registry updated",
                    cand_eval.score, base_eval.score
                ),
                true,
            ),
            Err(e) => (
                "reject".into(),
                format!("registry write failed: {e}"),
                false,
            ),
        };
    }
    (
        "eligible".into(),
        format!(
            "project_micro cand={:.3} >= base={:.3}; apply=false",
            cand_eval.score, base_eval.score
        ),
        false,
    )
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
    let block = format!("  {id}:\n    path: {cand}\n    alias: {id}\n    defaults:\n      ctx: 2048\n      reasoning: \"off\"\n");
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
