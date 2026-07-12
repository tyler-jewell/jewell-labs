//! Eval-gated improve: claim → snapshot → patch → score → keep|revert. dry_run default.

use crate::agents::{load_agent, write_agent_file, AgentsError};
use crate::jail::WriteJail;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

static CLAIMS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn claims() -> &'static Mutex<HashSet<String>> {
    CLAIMS.get_or_init(|| Mutex::new(HashSet::new()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnRequest {
    pub targets: Vec<String>,
    pub patches: Vec<String>,
    #[serde(default = "default_true")]
    pub dry_run: bool,
    #[serde(default = "default_max_diff")]
    pub max_diff_lines: usize,
    #[serde(default)]
    pub require_substrings: Vec<String>,
}

fn default_true() -> bool {
    true
}
fn default_max_diff() -> usize {
    40
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnTargetResult {
    pub target: String,
    pub baseline_score: f64,
    pub after_score: f64,
    pub diff_lines: usize,
    pub verdict: String,
    pub reason: String,
    pub disk_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnReport {
    pub dry_run: bool,
    pub results: Vec<LearnTargetResult>,
    pub kept: usize,
    pub reverted: usize,
    pub rejected: usize,
}

pub fn score_agent_markdown(markdown: &str, require: &[String]) -> f64 {
    if !crate::schema::certify_agent_markdown(markdown).ok {
        return 0.0;
    }
    if require.is_empty() {
        return 1.0;
    }
    require.iter().filter(|s| markdown.contains(s.as_str())).count() as f64 / require.len() as f64
}

pub fn diff_line_count(a: &str, b: &str) -> usize {
    let la: Vec<_> = a.lines().collect();
    let lb: Vec<_> = b.lines().collect();
    (0..la.len().max(lb.len()))
        .filter(|&i| la.get(i) != lb.get(i))
        .count()
}

pub fn learn_improve(
    agents_dir: impl AsRef<Path>,
    req: &LearnRequest,
) -> Result<LearnReport, AgentsError> {
    let dir = agents_dir.as_ref();
    if !req.patches.is_empty() && req.patches.len() != req.targets.len() {
        return Err(AgentsError::PublishBlocked(
            "patches length must match targets".into(),
        ));
    }
    let mut results = Vec::new();
    for (i, target) in req.targets.iter().enumerate() {
        let mut g = claims().lock().map_err(|e| AgentsError::PublishBlocked(e.to_string()))?;
        if !g.insert(target.clone()) {
            results.push(reject(target, "claim held"));
            continue;
        }
        drop(g);

        let r = improve_one(dir, target, req.patches.get(i).map(|s| s.as_str()).unwrap_or(""), req);
        if let Ok(mut g) = claims().lock() {
            g.remove(target);
        }
        results.push(match r {
            Ok(x) => x,
            Err(e) => reject(target, &e.to_string()),
        });
    }
    Ok(LearnReport {
        dry_run: req.dry_run,
        kept: results.iter().filter(|r| r.verdict == "keep").count(),
        reverted: results.iter().filter(|r| r.verdict == "revert").count(),
        rejected: results.iter().filter(|r| r.verdict == "rejected").count(),
        results,
    })
}

fn reject(target: &str, reason: &str) -> LearnTargetResult {
    LearnTargetResult {
        target: target.into(),
        baseline_score: 0.0,
        after_score: 0.0,
        diff_lines: 0,
        verdict: "rejected".into(),
        reason: reason.into(),
        disk_changed: false,
    }
}

fn improve_one(
    dir: &Path,
    target: &str,
    patch: &str,
    req: &LearnRequest,
) -> Result<LearnTargetResult, AgentsError> {
    WriteJail::agents_dir(dir).assert_not_core_lock(target)?;
    let doc = load_agent(dir, target)?;
    let baseline_text = std::fs::read_to_string(&doc.path)?;
    let baseline = score_agent_markdown(&baseline_text, &req.require_substrings);
    if patch.is_empty() {
        return Ok(LearnTargetResult {
            target: target.into(),
            baseline_score: baseline,
            after_score: baseline,
            diff_lines: 0,
            verdict: "noop".into(),
            reason: "empty patch".into(),
            disk_changed: false,
        });
    }
    let diff = diff_line_count(&baseline_text, patch);
    if diff > req.max_diff_lines {
        return Ok(LearnTargetResult {
            target: target.into(),
            baseline_score: baseline,
            after_score: baseline,
            diff_lines: diff,
            verdict: "rejected".into(),
            reason: format!("diff_lines {diff} > max {}", req.max_diff_lines),
            disk_changed: false,
        });
    }
    let after = score_agent_markdown(patch, &req.require_substrings);
    let would_keep = after >= baseline;
    if req.dry_run || !would_keep {
        return Ok(LearnTargetResult {
            target: target.into(),
            baseline_score: baseline,
            after_score: after,
            diff_lines: diff,
            verdict: if would_keep {
                "would_keep".into()
            } else {
                "revert".into()
            },
            reason: if would_keep {
                "dry_run".into()
            } else {
                "score regression".into()
            },
            disk_changed: false,
        });
    }
    write_agent_file(dir, target, patch)?;
    Ok(LearnTargetResult {
        target: target.into(),
        baseline_score: baseline,
        after_score: after,
        diff_lines: diff,
        verdict: "keep".into(),
        reason: "score >= baseline".into(),
        disk_changed: true,
    })
}

pub fn write_learn_report(report: &LearnReport, path: impl Into<PathBuf>) -> std::io::Result<PathBuf> {
    let path = path.into();
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(report)?)?;
    Ok(path)
}
