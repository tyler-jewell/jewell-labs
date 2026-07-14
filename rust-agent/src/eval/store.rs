//! List and skim eval run artifacts under evals/runs/.

use super::ground_truth::EvalReport;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub fn write_eval_report(report: &EvalReport, dir: impl AsRef<Path>) -> std::io::Result<PathBuf> {
    let dir = dir.as_ref();
    fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}.json", report.id));
    fs::write(&path, serde_json::to_string_pretty(report)?)?;
    Ok(path)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalRunSummary {
    pub id: String,
    pub created: Option<String>,
    pub agent: Option<String>,
    pub accuracy: Option<f64>,
    pub tool_plan_accuracy: Option<f64>,
    pub total: Option<usize>,
    pub correct: Option<usize>,
    pub kind: String,
    /// Human label for UI (not a path).
    pub label: String,
    pub passed: bool,
}

/// Classify run filename for UI filters.
pub fn run_kind(name: &str) -> &'static str {
    if name.starts_with("agent-introspection-") {
        "introspection"
    } else if name.starts_with("compare-") {
        "compare"
    } else if name.starts_with("catalog-") {
        "catalog"
    } else if name.starts_with("eval-") {
        "agent"
    } else if name.starts_with("learn-") {
        "learn"
    } else if name.starts_with("gsm8k") {
        "gsm8k"
    } else {
        "other"
    }
}

pub fn human_label(kind: &str, agent: Option<&str>) -> String {
    match kind {
        "introspection" => "Team gate".into(),
        "compare" => "Harness compare".into(),
        "catalog" => "Catalog sample".into(),
        "learn" => "Learn".into(),
        "gsm8k" => "GSM8K".into(),
        "agent" => agent
            .and_then(|a| a.rsplit_once('/').map(|(_, s)| s.to_string()))
            .or_else(|| agent.map(|s| s.to_string()))
            .unwrap_or_else(|| "Agent".into()),
        _ => agent
            .and_then(|a| a.rsplit_once('/').map(|(_, s)| s.to_string()))
            .unwrap_or_else(|| "Run".into()),
    }
}

fn is_passed(accuracy: Option<f64>, correct: Option<usize>, total: Option<usize>) -> bool {
    if let (Some(c), Some(t)) = (correct, total) {
        return t > 0 && c == t;
    }
    accuracy.map(|a| a >= 1.0).unwrap_or(false)
}

pub fn evals_runs_dir(repo_root: impl AsRef<Path>) -> PathBuf {
    repo_root.as_ref().join("evals").join("runs")
}

/// List recent JSON run summaries (newest first).
pub fn list_eval_runs(
    runs_dir: impl AsRef<Path>,
    limit: usize,
    prefix: Option<&str>,
) -> std::io::Result<Vec<EvalRunSummary>> {
    let dir = runs_dir.as_ref();
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    let mut files: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("json"))
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if let Some(pre) = prefix {
                if !pre.is_empty() {
                    return name.starts_with(pre);
                }
            }
            // Product eval UI: team gates + agent tool_plan + multi-harness compare
            name.starts_with("agent-introspection-")
                || name.starts_with("eval-")
                || name.starts_with("compare-")
                || name.starts_with("catalog-")
        })
        .collect();
    files.sort_by(|a, b| {
        let ma = fs::metadata(a).and_then(|m| m.modified()).ok();
        let mb = fs::metadata(b).and_then(|m| m.modified()).ok();
        mb.cmp(&ma)
    });
    let mut out = Vec::new();
    for path in files.into_iter().take(limit.max(1)) {
        if let Some(s) = skim_run(&path) {
            out.push(s);
        }
    }
    Ok(out)
}

fn skim_run(path: &Path) -> Option<EvalRunSummary> {
    let name = path.file_stem()?.to_str()?.to_string();
    let text = fs::read_to_string(path).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    let summary = v.get("summary");
    let agent = v
        .get("agent")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    let kind = run_kind(path.file_name()?.to_str()?).to_string();

    // Multi-harness compare reports use summary.harnesses + accuracy on scored items
    let (accuracy, total, correct) =
        if kind == "compare" || v.get("kind").and_then(|x| x.as_str()) == Some("compare") {
            let accuracy = summary
                .and_then(|s| s.get("accuracy"))
                .and_then(|x| x.as_f64());
            let total = summary
                .and_then(|s| s.get("total"))
                .and_then(|x| x.as_u64())
                .map(|n| n as usize);
            let correct = summary
                .and_then(|s| s.get("correct"))
                .and_then(|x| x.as_u64())
                .map(|n| n as usize);
            (accuracy, total, correct)
        } else {
            let accuracy = summary
                .and_then(|s| s.get("accuracy"))
                .and_then(|x| x.as_f64());
            let total = summary
                .and_then(|s| s.get("total"))
                .and_then(|x| x.as_u64())
                .map(|n| n as usize);
            let correct = summary
                .and_then(|s| s.get("correct"))
                .and_then(|x| x.as_u64())
                .map(|n| n as usize);
            (accuracy, total, correct)
        };

    let label = if kind == "compare" {
        let n = summary
            .and_then(|s| s.get("harnesses"))
            .and_then(|h| h.as_object())
            .map(|o| o.len())
            .unwrap_or(0);
        format!("Compare ({n} harnesses)")
    } else if kind == "catalog" {
        let n = v
            .get("selection")
            .and_then(|s| s.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        format!("Catalog ({n} items)")
    } else {
        human_label(&kind, agent.as_deref())
    };
    let passed = is_passed(accuracy, correct, total);
    Some(EvalRunSummary {
        id: v
            .get("id")
            .and_then(|x| x.as_str())
            .unwrap_or(&name)
            .to_string(),
        created: v
            .get("created")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string()),
        agent,
        accuracy,
        tool_plan_accuracy: summary
            .and_then(|s| s.get("tool_plan_accuracy"))
            .and_then(|x| x.as_f64()),
        total,
        correct,
        kind,
        label,
        passed,
    })
}

pub fn load_eval_run(runs_dir: impl AsRef<Path>, id: &str) -> std::io::Result<Value> {
    let dir = runs_dir.as_ref();
    let path = dir.join(format!("{id}.json"));
    if path.is_file() {
        let text = fs::read_to_string(path)?;
        return Ok(serde_json::from_str(&text)?);
    }
    for e in fs::read_dir(dir)? {
        let p = e?.path();
        if p.file_stem().and_then(|s| s.to_str()) == Some(id) {
            let text = fs::read_to_string(p)?;
            return Ok(serde_json::from_str(&text)?);
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("eval run not found: {id}"),
    ))
}
