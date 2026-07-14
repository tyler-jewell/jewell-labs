//! Execute catalog items across discovered vendors (same EvalModel for all).
//!
//! Fairness contract (poka-yoke):
//! - **Shared task surface**: every vendor receives the same instruction bytes
//!   from [`build_instruction`] (item prompt + optional tool schemas as data).
//! - **No per-vendor coaching**: adapters must not wrap instruction with
//!   format/answer hints. Differences live only in `evals/vendors/*/vendor.toml`.
//! - **Grade is post-hoc**: gold is never injected into the user message.

use super::filter::filter_items;
use super::grade::grade_catalog_item;
use super::load::load_catalog_items;
use super::sample::sample_items;
use super::types::{CatalogFilter, CatalogItem};
use crate::eval::store::evals_runs_dir;
use crate::eval::vendors::{
    load_eval_model, pin_dir_for_run, preflight_eval_model, resolve_vendors, vendor_meta_json,
    EvalModel, VendorContext, VendorDriver,
};
use crate::paths::repo_root;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct CatalogRunOpts {
    /// Empty = all enabled vendors from `evals/vendors/*/vendor.toml`.
    pub harnesses: Vec<String>,
    pub sources: Vec<String>,
    pub tags_any: Vec<String>,
    pub tags_all: Vec<String>,
    pub tracks: Vec<String>,
    pub sample_n: Option<usize>,
    pub seed: u64,
    pub run_id: Option<String>,
    pub include_disabled: bool,
    /// Drop Docker/Harbor sandbox items from the pool (default true).
    pub exclude_sandbox: bool,
    /// Skip model preflight (tests only).
    pub skip_model_preflight: bool,
    pub model_file: Option<PathBuf>,
    pub runs_dir: Option<PathBuf>,
}

impl Default for CatalogRunOpts {
    fn default() -> Self {
        Self {
            harnesses: vec![], // discover
            sources: vec![],
            tags_any: vec![],
            tags_all: vec![],
            tracks: vec![],
            sample_n: None,
            seed: 42,
            run_id: None,
            include_disabled: false,
            exclude_sandbox: true,
            skip_model_preflight: false,
            model_file: None,
            runs_dir: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogItemResult {
    pub harness: String,
    pub source_id: String,
    pub item_id: String,
    pub full_id: String,
    pub track: String,
    pub tags: Vec<String>,
    pub status: String,
    pub correct: bool,
    pub score: f64,
    pub t_ms: u64,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceBreakdown {
    pub n_items: usize,
    pub n_scored: usize,
    pub n_pass: usize,
    pub n_fail: usize,
    pub n_skip: usize,
    pub n_error: usize,
    pub accuracy: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogRunReport {
    pub id: String,
    pub created: String,
    pub kind: String,
    pub agent: String,
    pub tracks: Vec<String>,
    pub meta: Value,
    pub selection: Vec<String>,
    pub summary: CatalogSummary,
    pub by_source: BTreeMap<String, SourceBreakdown>,
    pub by_harness: BTreeMap<String, SourceBreakdown>,
    pub items: Vec<CatalogItemResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogSummary {
    pub total: usize,
    pub correct: usize,
    pub accuracy: f64,
    pub n_skip: usize,
    pub n_error: usize,
    pub n_scored: usize,
}

pub fn run_catalog_sample(opts: &CatalogRunOpts) -> Result<CatalogRunReport, String> {
    let model = load_eval_model(opts.model_file.as_deref())?;
    if !opts.skip_model_preflight {
        preflight_eval_model(&model)?;
    }

    let all =
        load_catalog_items(&opts.sources, opts.include_disabled).map_err(|e| e.to_string())?;
    let filter = CatalogFilter {
        sources: opts.sources.clone(),
        tags_all: opts.tags_all.clone(),
        tags_any: opts.tags_any.clone(),
        tracks: opts.tracks.clone(),
        exclude_sandbox: opts.exclude_sandbox,
    };
    let filtered = filter_items(&all, &filter);
    if filtered.is_empty() {
        return Err(format!(
            "no catalog items matched filter (sources={:?} tags_any={:?} tags_all={:?} tracks={:?} exclude_sandbox={}); loaded={}",
            opts.sources, opts.tags_any, opts.tags_all, opts.tracks, opts.exclude_sandbox, all.len()
        ));
    }
    let selected = if let Some(n) = opts.sample_n {
        sample_items(&filtered, n, opts.seed)
    } else {
        filtered.clone()
    };
    if selected.is_empty() {
        return Err("sample empty".into());
    }

    let run_id = opts
        .run_id
        .clone()
        .unwrap_or_else(|| format!("catalog-{}", Utc::now().format("%Y%m%dT%H%M%SZ")));
    let runs_root = opts
        .runs_dir
        .clone()
        .unwrap_or_else(|| evals_runs_dir(repo_root()));
    fs::create_dir_all(&runs_root).map_err(|e| e.to_string())?;
    let run_dir = runs_root.join(&run_id);
    fs::create_dir_all(&run_dir).map_err(|e| e.to_string())?;

    let pin_dir = pin_dir_for_run(&run_dir);
    fs::create_dir_all(&pin_dir).map_err(|e| e.to_string())?;
    let vendors = resolve_vendors(&opts.harnesses, &pin_dir, &model)?;
    let vendor_ids: Vec<String> = vendors.iter().map(|v| v.id().to_string()).collect();

    let selection_ids: Vec<String> = selected.iter().map(|i| i.full_id()).collect();
    let mut results = Vec::new();

    // Same selection for every vendor (outer = vendor, inner = items).
    for vendor in &vendors {
        for item in &selected {
            let r = run_one_item(vendor, &model, item, &run_dir)?;
            eprintln!(
                "→ {} {} [{}] score={:.2} {}",
                vendor.id(),
                item.full_id(),
                r.status,
                r.score,
                r.detail.chars().take(72).collect::<String>()
            );
            results.push(r);
        }
    }

    let (summary, by_source, by_harness) = aggregate(&results);
    let tracks: Vec<String> = {
        let mut t: Vec<_> = selected.iter().map(|i| i.track.clone()).collect();
        t.sort();
        t.dedup();
        t
    };

    let mut meta = vendor_meta_json(&vendors, &model);
    if let Some(obj) = meta.as_object_mut() {
        obj.insert("harnesses".into(), json!(vendor_ids));
        obj.insert("sources".into(), json!(opts.sources));
        obj.insert("tags_any".into(), json!(opts.tags_any));
        obj.insert("tags_all".into(), json!(opts.tags_all));
        obj.insert("sample_n".into(), json!(opts.sample_n));
        obj.insert("seed".into(), json!(opts.seed));
        obj.insert("exclude_sandbox".into(), json!(opts.exclude_sandbox));
        obj.insert("selection_count".into(), json!(selection_ids.len()));
        obj.insert("pool_filtered".into(), json!(filtered.len()));
        obj.insert("pool_all".into(), json!(all.len()));
        obj.insert(
            "fairness".into(),
            json!({
                "shared_instruction": true,
                "no_vendor_prompt_wrappers": true,
                "vendor_config_ssot": "evals/vendors/<id>/vendor.toml",
            }),
        );
    }

    let report = CatalogRunReport {
        id: run_id.clone(),
        created: Utc::now().to_rfc3339(),
        kind: "catalog".into(),
        agent: "multi-source-catalog".into(),
        tracks,
        meta,
        selection: selection_ids,
        summary,
        by_source,
        by_harness,
        items: results,
    };

    let text = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    fs::write(runs_root.join(format!("{run_id}.json")), &text).map_err(|e| e.to_string())?;
    fs::write(run_dir.join("report.json"), &text).map_err(|e| e.to_string())?;
    write_md(&run_dir, &report)?;
    Ok(report)
}

fn run_one_item(
    vendor: &VendorDriver,
    model: &EvalModel,
    item: &CatalogItem,
    run_dir: &Path,
) -> Result<CatalogItemResult, String> {
    let item_dir = run_dir
        .join(vendor.id())
        .join(&item.source_id)
        .join(&item.id);
    fs::create_dir_all(&item_dir).map_err(|e| e.to_string())?;
    let workspace = item_dir.join("workspace");
    fs::create_dir_all(&workspace).map_err(|e| e.to_string())?;
    for (rel, content) in &item.workspace_files {
        let p = workspace.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(p, content).map_err(|e| e.to_string())?;
    }

    // Capability gate
    let missing: Vec<String> = item
        .capabilities
        .iter()
        .filter(|c| !vendor.provides().iter().any(|p| p == *c))
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Ok(CatalogItemResult {
            harness: vendor.id().into(),
            source_id: item.source_id.clone(),
            item_id: item.id.clone(),
            full_id: item.full_id(),
            track: item.track.clone(),
            tags: item.tags.clone(),
            status: "skip".into(),
            correct: false,
            score: 0.0,
            t_ms: 0,
            detail: format!("missing capabilities: {missing:?}"),
        });
    }

    if item.grade.requires_sandbox || item.grade.kind == "sandbox_skip" {
        return Ok(CatalogItemResult {
            harness: vendor.id().into(),
            source_id: item.source_id.clone(),
            item_id: item.id.clone(),
            full_id: item.full_id(),
            track: item.track.clone(),
            tags: item.tags.clone(),
            status: "skip".into(),
            correct: false,
            score: 0.0,
            t_ms: 0,
            detail: item
                .grade
                .skip_reason
                .clone()
                .unwrap_or_else(|| "requires full benchmark sandbox (Docker/Harbor)".into()),
        });
    }

    // Shared task surface — identical bytes for every vendor.
    let instruction = build_instruction(item);
    let _ = fs::write(item_dir.join("instruction.txt"), &instruction);

    let ctx = VendorContext {
        model,
        workspace: &workspace,
        instruction: &instruction,
        timeout_s: 300,
        required_capabilities: &item.capabilities,
        track: item.track.as_str(),
    };
    let ar = vendor.run(&ctx);
    let _ = fs::write(item_dir.join("agent_stdout.txt"), &ar.stdout);
    let _ = fs::write(item_dir.join("agent_stderr.txt"), &ar.stderr);

    if ar.status == "skip" {
        return Ok(CatalogItemResult {
            harness: vendor.id().into(),
            source_id: item.source_id.clone(),
            item_id: item.id.clone(),
            full_id: item.full_id(),
            track: item.track.clone(),
            tags: item.tags.clone(),
            status: "skip".into(),
            correct: false,
            score: 0.0,
            t_ms: ar.t_ms,
            detail: ar.detail,
        });
    }
    if ar.status == "error" {
        return Ok(CatalogItemResult {
            harness: vendor.id().into(),
            source_id: item.source_id.clone(),
            item_id: item.id.clone(),
            full_id: item.full_id(),
            track: item.track.clone(),
            tags: item.tags.clone(),
            status: "error".into(),
            correct: false,
            score: 0.0,
            t_ms: ar.t_ms,
            detail: ar.detail,
        });
    }

    if !workspace.join("agent_answer.txt").is_file() {
        let _ = fs::write(workspace.join("agent_answer.txt"), &ar.stdout);
    }
    let g = grade_catalog_item(item, &workspace, vendor.id());
    let _ = fs::write(
        item_dir.join("grade.json"),
        serde_json::to_string_pretty(&json!({
            "correct": g.correct,
            "score": g.score,
            "detail": g.detail,
        }))
        .unwrap_or_default(),
    );
    Ok(CatalogItemResult {
        harness: vendor.id().into(),
        source_id: item.source_id.clone(),
        item_id: item.id.clone(),
        full_id: item.full_id(),
        track: item.track.clone(),
        tags: item.tags.clone(),
        status: if g.correct { "pass" } else { "fail" }.into(),
        correct: g.correct,
        score: g.score,
        t_ms: ar.t_ms,
        detail: g.detail,
    })
}

/// Shared task surface for every vendor.
///
/// Contents:
/// 1. Optional tool schemas from the catalog item (BFCL-style **data**).
/// 2. The item `prompt` verbatim (dataset SSoT; edit items, not vendor drivers).
///
/// Does **not** inject grade-kind hints, gold strings, output filenames, or
/// per-vendor coaching. Protocol text that is part of a function-calling
/// benchmark must live in the catalog item prompt (shared), never in
/// `evals/vendors/*/vendor.toml` wrappers or Rust adapter `format!` strings.
pub fn build_instruction(item: &CatalogItem) -> String {
    let mut s = String::new();

    if !item.tools.is_empty() {
        // Neutral tool payload — same bytes for every vendor.
        s.push_str("Tools:\n");
        let tools_json = serde_json::to_string_pretty(&item.tools).unwrap_or_else(|_| "[]".into());
        s.push_str(&tools_json);
        s.push_str("\n\n");
    }

    s.push_str(item.prompt.trim());
    s.push('\n');
    s
}

fn aggregate(
    results: &[CatalogItemResult],
) -> (
    CatalogSummary,
    BTreeMap<String, SourceBreakdown>,
    BTreeMap<String, SourceBreakdown>,
) {
    let scored: Vec<_> = results
        .iter()
        .filter(|r| r.status == "pass" || r.status == "fail")
        .collect();
    let n_skip = results.iter().filter(|r| r.status == "skip").count();
    let n_error = results.iter().filter(|r| r.status == "error").count();
    let n_pass = scored.iter().filter(|r| r.correct).count();
    let summary = CatalogSummary {
        total: results.len(),
        correct: n_pass,
        accuracy: if scored.is_empty() {
            0.0
        } else {
            n_pass as f64 / scored.len() as f64
        },
        n_skip,
        n_error,
        n_scored: scored.len(),
    };

    let mut by_source: BTreeMap<String, Vec<&CatalogItemResult>> = BTreeMap::new();
    let mut by_harness: BTreeMap<String, Vec<&CatalogItemResult>> = BTreeMap::new();
    for r in results {
        by_source.entry(r.source_id.clone()).or_default().push(r);
        by_harness.entry(r.harness.clone()).or_default().push(r);
    }
    let by_source = by_source
        .into_iter()
        .map(|(k, v)| (k, breakdown(&v)))
        .collect();
    let by_harness = by_harness
        .into_iter()
        .map(|(k, v)| (k, breakdown(&v)))
        .collect();
    (summary, by_source, by_harness)
}

fn breakdown(rows: &[&CatalogItemResult]) -> SourceBreakdown {
    let n_items = rows.len();
    let scored: Vec<_> = rows
        .iter()
        .filter(|r| r.status == "pass" || r.status == "fail")
        .collect();
    let n_pass = scored.iter().filter(|r| r.correct).count();
    let n_fail = scored.len() - n_pass;
    let n_skip = rows.iter().filter(|r| r.status == "skip").count();
    let n_error = rows.iter().filter(|r| r.status == "error").count();
    SourceBreakdown {
        n_items,
        n_scored: scored.len(),
        n_pass,
        n_fail,
        n_skip,
        n_error,
        accuracy: if scored.is_empty() {
            0.0
        } else {
            n_pass as f64 / scored.len() as f64
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::catalog::types::GradeSpec;
    use std::collections::BTreeMap;

    #[test]
    fn build_instruction_is_prompt_plus_tools_only() {
        let tool = CatalogItem {
            id: "simple-search".into(),
            source_id: "bfcl".into(),
            tags: vec!["tool_call".into()],
            track: "tool_call".into(),
            capabilities: vec!["closed_form".into()],
            prompt: "Search the web for: terminal bench agents".into(),
            grade: GradeSpec {
                kind: "tool_call".into(),
                expected: None,
                expected_contains: Some("terminal bench".into()),
                tool_name: Some("web_search".into()),
                dry_pass: true,
                skip_reason: None,
                requires_sandbox: false,
                answer_file: None,
            },
            tools: vec![serde_json::json!({"name":"web_search"})],
            workspace_files: BTreeMap::new(),
            links: vec![],
            meta: serde_json::Value::Null,
        };
        let s = build_instruction(&tool);
        assert!(s.contains("web_search"), "{s}");
        assert!(s.contains("Search the web"), "{s}");
        // No coaching / gold / grade leakage
        for banned in [
            "FORMAT-ONLY",
            "fictional",
            "Do not greet",
            "NO_TOOL",
            "never an unedited",
            "Double-check",
            "Chat-only answers",
            "do not execute",
        ] {
            assert!(
                !s.contains(banned),
                "instruction must not coach with {banned:?}: {s}"
            );
        }
    }

    #[test]
    fn build_instruction_no_grade_injection_for_files() {
        let it = CatalogItem {
            id: "env-config".into(),
            source_id: "terminal-bench".into(),
            tags: vec![],
            track: "terminal".into(),
            capabilities: vec!["write_file".into()],
            prompt: "Write .env.example from config.sample.json".into(),
            grade: GradeSpec {
                kind: "file_lines_exact".into(),
                expected: Some("API_KEY=\nHOST=".into()),
                expected_contains: None,
                tool_name: None,
                dry_pass: true,
                skip_reason: None,
                requires_sandbox: false,
                answer_file: Some(".env.example".into()),
            },
            tools: vec![],
            workspace_files: BTreeMap::new(),
            links: vec![],
            meta: serde_json::Value::Null,
        };
        let s = build_instruction(&it);
        assert_eq!(s.trim(), "Write .env.example from config.sample.json");
        assert!(!s.contains("API_KEY="), "must not leak gold: {s}");
        assert!(!s.contains("## Output"), "{s}");
    }
}

fn write_md(run_dir: &Path, report: &CatalogRunReport) -> Result<(), String> {
    let mut md = format!(
        "# Catalog run `{}`\n\nCreated: {}\n\nSelection ({} items): {}\n\n",
        report.id,
        report.created,
        report.selection.len(),
        report.selection.join(", ")
    );
    if let Some(em) = report.meta.get("eval_model") {
        md.push_str(&format!(
            "## Shared model\n\n```\n{}\n```\n\n",
            serde_json::to_string_pretty(em).unwrap_or_default()
        ));
    }
    md.push_str("## By source\n\n| Source | items | scored | pass | fail | skip | err | accuracy |\n| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n");
    for (s, b) in &report.by_source {
        md.push_str(&format!(
            "| {s} | {} | {} | {} | {} | {} | {} | {:.0}% |\n",
            b.n_items,
            b.n_scored,
            b.n_pass,
            b.n_fail,
            b.n_skip,
            b.n_error,
            b.accuracy * 100.0
        ));
    }
    md.push_str("\n## By harness\n\n| Harness | items | scored | pass | fail | skip | err | accuracy |\n| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n");
    for (h, b) in &report.by_harness {
        md.push_str(&format!(
            "| {h} | {} | {} | {} | {} | {} | {} | {:.0}% |\n",
            b.n_items,
            b.n_scored,
            b.n_pass,
            b.n_fail,
            b.n_skip,
            b.n_error,
            b.accuracy * 100.0
        ));
    }
    md.push_str("\n## Items\n\n| harness | id | status | score | detail |\n| --- | --- | --- | ---: | --- |\n");
    for it in &report.items {
        let d = it.detail.replace('|', "/");
        let d: String = d.chars().take(60).collect();
        md.push_str(&format!(
            "| {} | {} | {} | {:.2} | {d} |\n",
            it.harness, it.full_id, it.status, it.score
        ));
    }
    fs::write(run_dir.join("report.md"), md).map_err(|e| e.to_string())
}
