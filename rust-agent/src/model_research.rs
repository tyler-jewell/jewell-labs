//! Model research: public sources + local comparison. Candidate fitness is measured
//! separately from baseline (not duplicated theater scores).
use crate::registry::ModelRegistry;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
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
/// Fitness of a weight file on disk: 0..1 from existence, size, GGUF magic.
pub fn model_fitness(path: &Path) -> f64 {
    if !path.is_file() {
        return 0.0;
    }
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if size == 0 {
        return 0.05;
    }
    let mut score: f64 = 0.4;
    if let Ok(mut f) = File::open(path) {
        let mut magic = [0u8; 4];
        if f.read_exact(&mut magic).is_ok() && &magic == b"GGUF" {
            score += 0.4;
        }
    }
    if size >= 1_000_000 {
        score += 0.2;
    } else if size >= 1024 {
        score += 0.1;
    }
    score.min(1.0)
}
/// Project structural gate (infra). Same for any model decision — not a fake candidate score.
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
    let (baseline_model, base_path) = match &reg {
        Ok(r) => {
            let key = r
                .models
                .keys()
                .next()
                .cloned()
                .unwrap_or_else(|| "unknown".into());
            match r.resolve(&key) {
                Ok(m) => (key, Some(m.path)),
                Err(_) => (key, None),
            }
        }
        Err(_) => ("unreadable".into(), None),
    };
    let cand_path = req
        .candidate_path
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| {
            reg.as_ref()
                .ok()
                .and_then(|r| r.resolve(&req.candidate_id).ok().map(|m| m.path))
        });
    let base_fit = base_path.as_ref().map(|p| model_fitness(p)).unwrap_or(0.0);
    let cand_fit = cand_path.as_ref().map(|p| model_fitness(p)).unwrap_or(0.0);
    let infra = project_tool_plan_score();
    // Temp registry resolve: candidate must be loadable as a ModelRegistry entry shape
    let cand_resolve_ok = cand_path
        .as_ref()
        .map(|p| probe_temp_registry_resolve(&req.candidate_id, p))
        .unwrap_or(false);
    let network_status = if req.offline {
        "offline_mode".into()
    } else {
        "probe_deferred_use_offline_or_cli".into()
    };
    let metrics = vec![
        GateMetric {
            name: "model_fitness".into(),
            baseline: base_fit,
            candidate: cand_fit,
            not_worse: cand_fit + 1e-9 >= base_fit,
        },
        GateMetric {
            name: "candidate_resolves_in_temp_registry".into(),
            baseline: 1.0,
            candidate: if cand_resolve_ok { 1.0 } else { 0.0 },
            not_worse: cand_resolve_ok,
        },
        GateMetric {
            name: "project_tool_plan_infra".into(),
            baseline: infra,
            candidate: infra, // infra is shared; candidate cannot run without green host
            not_worse: infra >= 1.0,
        },
    ];
    // Promote requires: infra green, candidate fitness not worse, resolve ok, path present
    let all_ok = metrics.iter().all(|m| m.not_worse) && base_fit > 0.0;
    let (decision, reason, registry_changed) = decide(
        req,
        all_ok,
        cand_path.as_ref(),
        &baseline_model,
        registry_path.as_ref(),
        cand_fit,
        base_fit,
    );
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
fn probe_temp_registry_resolve(id: &str, path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let tmp = dir.join(format!(".research-reg-{id}.yaml"));
    let body = format!(
        "models:\n  {id}:\n    path: {}\n    alias: {id}\n",
        path.display()
    );
    if std::fs::write(&tmp, body).is_err() {
        return false;
    }
    let ok = ModelRegistry::load(&tmp)
        .ok()
        .and_then(|r| r.resolve(id).ok())
        .map(|m| m.path.exists())
        .unwrap_or(false);
    let _ = std::fs::remove_file(&tmp);
    ok
}
fn decide(
    req: &ModelResearchRequest,
    all_ok: bool,
    cand_path: Option<&PathBuf>,
    baseline: &str,
    registry_path: &Path,
    cand_fit: f64,
    base_fit: f64,
) -> (String, String, bool) {
    if !all_ok {
        return ("reject".into(), format!("gates failed (cand_fit={cand_fit:.3} base_fit={base_fit:.3})"), false);
    }
    if req.candidate_id == baseline {
        return ("reject".into(), "candidate is already baseline".into(), false);
    }
    if cand_path.is_none() {
        return ("reject".into(), "candidate path missing".into(), false);
    }
    if req.apply {
        match apply_registry(
            registry_path,
            &req.candidate_id,
            cand_path.and_then(|p| p.to_str()),
        ) {
            Ok(()) => (
                "promote".into(),
                format!("cand_fit={cand_fit:.3} >= base_fit={base_fit:.3}; registry updated"),
                true,
            ),
            Err(e) => ("reject".into(), format!("registry write failed: {e}"), false),
        }
    } else {
        (
            "eligible".into(),
            format!("cand_fit={cand_fit:.3} >= base_fit={base_fit:.3}; apply=false"),
            false,
        )
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
