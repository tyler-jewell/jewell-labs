//! Multi-harness compare orchestration.

use super::adapters::get_adapter;
use super::aggregate::{summarize_items, task_matrix};
use super::grade::grade_task;
use super::task::{compare_tasks_dir, discover_tasks, Task};
use super::types::{CompareItem, CompareReport};
use crate::eval::store::evals_runs_dir;
use crate::paths::repo_root;
use chrono::Utc;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct CompareOpts {
    pub harnesses: Vec<String>,
    pub tasks_filter: String,
    pub n_runs: Option<u32>,
    pub run_id: Option<String>,
    pub tasks_dir: Option<PathBuf>,
    pub runs_dir: Option<PathBuf>,
}

impl Default for CompareOpts {
    fn default() -> Self {
        Self {
            harnesses: vec!["dry".into()],
            tasks_filter: "all".into(),
            n_runs: None,
            run_id: None,
            tasks_dir: None,
            runs_dir: None,
        }
    }
}

pub fn run_compare(opts: &CompareOpts) -> Result<CompareReport, String> {
    let root = repo_root();
    let tasks_dir = opts
        .tasks_dir
        .clone()
        .unwrap_or_else(|| compare_tasks_dir(&root));
    let tasks = discover_tasks(&tasks_dir, &opts.tasks_filter).map_err(|e| e.to_string())?;
    if tasks.is_empty() {
        return Err(format!(
            "no tasks matched {:?} under {}",
            opts.tasks_filter,
            tasks_dir.display()
        ));
    }

    let run_id = opts
        .run_id
        .clone()
        .unwrap_or_else(|| format!("compare-{}", Utc::now().format("%Y%m%dT%H%M%SZ")));
    // Artifact dir under monorepo evals/runs/<id>/  + flat JSON for list API
    let runs_root = opts
        .runs_dir
        .clone()
        .unwrap_or_else(|| evals_runs_dir(&root));
    fs::create_dir_all(&runs_root).map_err(|e| e.to_string())?;
    let run_dir = runs_root.join(&run_id);
    fs::create_dir_all(&run_dir).map_err(|e| e.to_string())?;

    let mut items = Vec::new();
    for harness in &opts.harnesses {
        let adapter = get_adapter(harness)?;
        for task in &tasks {
            let n_runs = opts.n_runs.unwrap_or(task.n_runs_default).max(1);
            for run_i in 1..=n_runs {
                let item = run_one(adapter.as_ref(), task, &run_dir, run_i)?;
                eprintln!(
                    "→ {} {} run={}/{} [{}] score={:.2} {}",
                    harness,
                    task.id,
                    run_i,
                    n_runs,
                    item.status,
                    item.score,
                    item.detail.chars().take(80).collect::<String>()
                );
                items.push(item);
            }
        }
    }

    let summary = summarize_items(&items);
    let matrix = task_matrix(&items);
    let tracks: Vec<String> = {
        let mut t: Vec<String> = tasks.iter().map(|t| t.track.clone()).collect();
        t.sort();
        t.dedup();
        t
    };
    let report = CompareReport {
        id: run_id.clone(),
        created: Utc::now().to_rfc3339(),
        kind: CompareReport::KIND.into(),
        agent: "multi-harness".into(),
        tracks,
        meta: json!({
            "harnesses": opts.harnesses,
            "tasks": tasks.iter().map(|t| t.id.clone()).collect::<Vec<_>>(),
            "n_runs": opts.n_runs,
            "tasks_dir": tasks_dir.display().to_string(),
            "run_dir": run_dir.display().to_string(),
        }),
        summary,
        items,
        task_matrix: matrix,
    };

    // Persist flat JSON for list_eval_runs + detailed dir report
    let flat = runs_root.join(format!("{run_id}.json"));
    let text = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    fs::write(&flat, &text).map_err(|e| e.to_string())?;
    fs::write(run_dir.join("report.json"), &text).map_err(|e| e.to_string())?;
    write_report_md(&run_dir, &report)?;

    Ok(report)
}

fn run_one(
    adapter: &dyn super::adapters::HarnessAdapter,
    task: &Task,
    run_dir: &Path,
    run_i: u32,
) -> Result<CompareItem, String> {
    let item_dir = run_dir
        .join(adapter.name())
        .join(&task.id)
        .join(format!("run-{run_i}"));
    fs::create_dir_all(&item_dir).map_err(|e| e.to_string())?;
    let workspace = task
        .materialize_workspace(&item_dir)
        .map_err(|e| e.to_string())?;
    let instruction = task.instruction().map_err(|e| e.to_string())?;

    let (can, missing) = adapter.can_run(task);
    let (status, score, correct, detail, t_ms, metrics, caps_missing, ar) = if !can {
        (
            "skip".to_string(),
            0.0,
            false,
            format!("missing capabilities: {missing:?}"),
            0,
            Default::default(),
            missing,
            None,
        )
    } else {
        let ar = adapter.run(task, &workspace, &instruction);
        let _ = fs::write(item_dir.join("agent_stdout.txt"), &ar.stdout);
        let _ = fs::write(item_dir.join("agent_stderr.txt"), &ar.stderr);

        if ar.status == "skip" {
            (
                "skip".into(),
                0.0,
                false,
                ar.detail.clone(),
                ar.t_ms,
                Default::default(),
                ar.capabilities_missing.clone(),
                Some(ar),
            )
        } else if ar.status == "error" {
            (
                "error".into(),
                0.0,
                false,
                ar.detail.clone(),
                ar.t_ms,
                Default::default(),
                vec![],
                Some(ar),
            )
        } else {
            if let Some(code) = ar.exit_code {
                let _ = fs::write(workspace.join("host_exit_code.txt"), format!("{code}\n"));
            }
            if task.track == "closed_form" && !workspace.join("agent_answer.txt").is_file() {
                let _ = fs::write(workspace.join("agent_answer.txt"), &ar.stdout);
            }
            let grade = grade_task(&task.id, &workspace, &task.path);
            let _ = fs::write(
                item_dir.join("grade.json"),
                serde_json::to_string_pretty(&grade.to_json()).unwrap_or_default(),
            );
            let status = if grade.correct { "pass" } else { "fail" };
            (
                status.into(),
                grade.score,
                grade.correct,
                if grade.detail.is_empty() {
                    ar.detail.clone()
                } else {
                    grade.detail.clone()
                },
                ar.t_ms,
                grade.metrics,
                vec![],
                Some(ar),
            )
        }
    };

    let meta = json!({
        "harness": adapter.name(),
        "task_id": task.id,
        "track": task.track,
        "run_index": run_i,
        "timeout_s": task.timeout_s,
        "capabilities": task.capabilities,
        "status": status,
        "detail": detail,
    });
    let _ = fs::write(
        item_dir.join("meta.json"),
        serde_json::to_string_pretty(&meta).unwrap_or_default(),
    );

    let _ = ar; // silence
    Ok(CompareItem {
        harness: adapter.name().into(),
        task_id: task.id.clone(),
        track: task.track.clone(),
        run_index: run_i,
        status,
        correct,
        score,
        t_ms,
        detail,
        metrics: metrics.into_iter().collect(),
        workspace: workspace.display().to_string(),
        capabilities_missing: caps_missing,
    })
}

fn write_report_md(run_dir: &Path, report: &CompareReport) -> Result<(), String> {
    let mut md = String::new();
    md.push_str(&format!("# Harness compare `{}`\n\n", report.id));
    md.push_str(&format!("Created: {}\n\n", report.created));
    md.push_str("## Leaderboard\n\n");
    md.push_str(
        "| Harness | avg_score | solid_base | pass_rate | n_scored | n_skip | n_error | mean_t_ms |\n",
    );
    md.push_str("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n");
    for (h, s) in &report.summary.harnesses {
        md.push_str(&format!(
            "| {h} | {:.2} | {:.2} | {:.0}% | {} | {} | {} | {} |\n",
            s.avg_score,
            s.solid_base,
            s.pass_rate * 100.0,
            s.n_scored,
            s.n_skip,
            s.n_error,
            s.mean_t_ms
        ));
    }
    md.push_str("\n## Task matrix (avg score)\n\n");
    let harnesses: Vec<_> = report.summary.harnesses.keys().cloned().collect();
    md.push_str("| Task |");
    for h in &harnesses {
        md.push_str(&format!(" {h} |"));
    }
    md.push('\n');
    md.push_str("| --- |");
    for _ in &harnesses {
        md.push_str(" ---: |");
    }
    md.push('\n');
    for (task, row) in &report.task_matrix {
        md.push_str(&format!("| {task} |"));
        for h in &harnesses {
            let cell = row
                .get(h)
                .map(|v| format!("{v:.2}"))
                .unwrap_or_else(|| "—".into());
            md.push_str(&format!(" {cell} |"));
        }
        md.push('\n');
    }
    md.push_str("\n## Per-item\n\n");
    md.push_str("| harness | task | run | status | score | t_ms | detail |\n");
    md.push_str("| --- | --- | ---: | --- | ---: | ---: | --- |\n");
    for it in &report.items {
        let detail = it.detail.replace('|', "/");
        let detail: String = detail.chars().take(80).collect();
        md.push_str(&format!(
            "| {} | {} | {} | {} | {:.2} | {} | {detail} |\n",
            it.harness, it.task_id, it.run_index, it.status, it.score, it.t_ms
        ));
    }
    fs::write(run_dir.join("report.md"), md).map_err(|e| e.to_string())?;
    Ok(())
}

/// Legacy local Harbor tasks only (off the default multi-source path).
/// Prefer [`crate::eval::catalog::run_catalog_sample`] for public benches.
pub fn run_dry_all() -> Result<CompareReport, String> {
    run_compare(&CompareOpts {
        harnesses: vec!["dry".into()],
        tasks_filter: "all".into(),
        ..Default::default()
    })
}
