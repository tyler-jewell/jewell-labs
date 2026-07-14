//! Evals history + run APIs.

use super::state::AppState;
use crate::eval::{
    eval_all_agents, evals_runs_dir, list_eval_runs, load_eval_run, run_agent_eval,
    run_catalog_sample, run_compare, run_full_eval, write_eval_report, write_report,
    CatalogRunOpts, CompareOpts, EvalReport,
};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::Json;
use futures_util::stream::{self, Stream};
use serde::Deserialize;
use serde_json::{json, Value};
use std::convert::Infallible;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub prefix: Option<String>,
}

fn default_limit() -> usize {
    50
}

pub async fn list_runs(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListQuery>,
) -> impl IntoResponse {
    let dir = evals_runs_dir(&state.repo_root);
    match list_eval_runs(&dir, q.limit, q.prefix.as_deref()) {
        Ok(runs) => Json(json!({ "runs": runs })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn get_run(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let dir = evals_runs_dir(&state.repo_root);
    match load_eval_run(&dir, &id) {
        Ok(v) => Json(v).into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct RunBody {
    /// team | agent | all | compare | catalog
    #[serde(default = "default_suite")]
    pub suite: String,
    pub agent_id: Option<String>,
    /// For suite=compare/catalog: jewell,hermes (comma-separated)
    pub harnesses: Option<String>,
    /// For suite=compare: all | track | task ids (legacy local tasks)
    pub tasks: Option<String>,
    pub n_runs: Option<u32>,
    /// catalog: comma source ids
    pub sources: Option<String>,
    /// catalog: comma tags (OR)
    pub tags: Option<String>,
    pub sample_n: Option<usize>,
    pub seed: Option<u64>,
}

fn default_suite() -> String {
    "team".into()
}

/// SSE stream: progress events then final report.
pub async fn run_stream(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RunBody>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let presence = state.presence.clone();
    let runs_dir = evals_runs_dir(&state.repo_root);
    let suite = body.suite.clone();
    let agent_id = body.agent_id.clone();
    let harnesses = body.harnesses.clone();
    let tasks = body.tasks.clone();
    let n_runs = body.n_runs;
    let sources = body.sources.clone();
    let tags = body.tags.clone();
    let sample_n = body.sample_n;
    let seed = body.seed;

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
    tokio::spawn(async move {
        presence.set_busy("eval/team", None);
        let _ = tx.send(json!({"type":"log","msg": format!("starting suite={suite}")}));

        let out = match suite.as_str() {
            "team" => {
                let _ =
                    tx.send(json!({"type":"log","msg":"running tool_plan + team_collaboration"}));
                let report = run_full_eval(false).await;
                persist_report(&report, &runs_dir, true)
            }
            "catalog" => {
                // Empty harnesses → all enabled vendors (evals/vendors/*/vendor.toml)
                let harnesses = harnesses.unwrap_or_default();
                let _ = tx.send(json!({
                    "type":"log",
                    "msg": format!("catalog harnesses={} sample_n={sample_n:?}", if harnesses.is_empty() { "(all enabled)" } else { harnesses.as_str() })
                }));
                tokio::task::spawn_blocking({
                    let tx = tx.clone();
                    move || {
                        let hs: Vec<String> = harnesses
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        let sources: Vec<String> = sources
                            .unwrap_or_default()
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        let tags_any: Vec<String> = tags
                            .unwrap_or_default()
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        let _ = tx.send(json!({"type":"log","msg":"run_catalog_sample"}));
                        let report = run_catalog_sample(&CatalogRunOpts {
                            harnesses: hs,
                            sources,
                            tags_any,
                            sample_n: sample_n.or(Some(20)),
                            seed: seed.unwrap_or(42),
                            exclude_sandbox: true,
                            ..Default::default()
                        })?;
                        let mut v = serde_json::to_value(&report).map_err(|e| e.to_string())?;
                        if let Some(obj) = v.as_object_mut() {
                            obj.insert(
                                "path".into(),
                                json!(format!("evals/runs/{}.json", report.id)),
                            );
                        }
                        Ok(v)
                    }
                })
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r)
            }
            "compare" => {
                // Legacy local Harbor tasks only — prefer suite=catalog for public benches.
                let harnesses = harnesses.unwrap_or_else(|| "dry".into());
                let tasks = tasks.unwrap_or_else(|| "all".into());
                let _ = tx.send(json!({
                    "type":"log",
                    "msg": format!("legacy compare tasks={tasks} (use suite=catalog for multi-source)")
                }));
                tokio::task::spawn_blocking({
                    let tx = tx.clone();
                    move || {
                        let hs: Vec<String> = harnesses
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        let _ = tx.send(json!({"type":"log","msg":"run_compare legacy"}));
                        let report = run_compare(&CompareOpts {
                            harnesses: hs,
                            tasks_filter: tasks,
                            n_runs,
                            ..Default::default()
                        })?;
                        let mut v = serde_json::to_value(&report).map_err(|e| e.to_string())?;
                        if let Some(obj) = v.as_object_mut() {
                            obj.insert(
                                "path".into(),
                                json!(format!("evals/runs/{}.json", report.id)),
                            );
                        }
                        Ok(v)
                    }
                })
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r)
            }
            "all" => tokio::task::spawn_blocking({
                let runs_dir = runs_dir.clone();
                let tx = tx.clone();
                move || {
                    let _ = tx.send(json!({"type":"log","msg":"eval_all_agents"}));
                    let reports = eval_all_agents().map_err(|e| e.to_string())?;
                    let mut last = None;
                    for r in &reports {
                        let _ = tx.send(json!({"type":"log","msg": format!("agent {}", r.agent)}));
                        let p = write_eval_report(r, &runs_dir).map_err(|e| e.to_string())?;
                        last = Some((serde_json::to_value(r).map_err(|e| e.to_string())?, p));
                    }
                    let (mut v, p) = last.ok_or_else(|| "no agents".to_string())?;
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("path".into(), json!(p.display().to_string()));
                        obj.insert("suite_count".into(), json!(reports.len()));
                    }
                    Ok(v)
                }
            })
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r),
            "agent" => {
                let id = agent_id
                    .clone()
                    .unwrap_or_else(|| crate::CORE_AGENT_ID.to_string());
                let _ = tx.send(json!({"type":"log","msg": format!("run_agent_eval {id}")}));
                tokio::task::spawn_blocking({
                    let runs_dir = runs_dir.clone();
                    let id = id.clone();
                    move || {
                        let r = run_agent_eval(&id, "tool_plan").map_err(|e| e.to_string())?;
                        let p = write_eval_report(&r, &runs_dir).map_err(|e| e.to_string())?;
                        let mut v = serde_json::to_value(&r).map_err(|e| e.to_string())?;
                        if let Some(obj) = v.as_object_mut() {
                            obj.insert("path".into(), json!(p.display().to_string()));
                        }
                        Ok(v)
                    }
                })
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r)
            }
            other => Err(format!("unknown suite: {other}")),
        };

        match out {
            Ok(report) => {
                let acc = report
                    .pointer("/summary/accuracy")
                    .and_then(|x| x.as_f64())
                    .unwrap_or(0.0);
                let id = report.get("id").and_then(|x| x.as_str()).unwrap_or("?");
                let kind = report.get("kind").and_then(|x| x.as_str()).unwrap_or("");
                let _ = tx.send(json!({
                    "type": "log",
                    "msg": format!("done id={id} kind={kind} accuracy={:.0}%", acc * 100.0)
                }));
                let _ = tx.send(json!({"type":"done","report": report}));
            }
            Err(e) => {
                let _ = tx.send(json!({"type":"error","msg": e}));
            }
        }
        presence.set_idle("eval/team");
        let _ = tx.send(json!({"type":"status","status":"idle"}));
    });

    let stream = stream::unfold(rx, |mut rx| async move {
        match rx.recv().await {
            Some(v) => {
                let data = serde_json::to_string(&v).unwrap_or_else(|_| "{}".into());
                Some((Ok(Event::default().data(data)), rx))
            }
            None => None,
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

fn persist_report(
    report: &EvalReport,
    runs_dir: &std::path::Path,
    use_write_report: bool,
) -> Result<Value, String> {
    let path = if use_write_report {
        write_report(report, runs_dir.join(format!("{}.json", report.id)))
            .map_err(|e| e.to_string())?
    } else {
        write_eval_report(report, runs_dir).map_err(|e| e.to_string())?
    };
    let mut v = serde_json::to_value(report).map_err(|e| e.to_string())?;
    if let Some(obj) = v.as_object_mut() {
        obj.insert("path".into(), json!(path.display().to_string()));
    }
    Ok(v)
}
