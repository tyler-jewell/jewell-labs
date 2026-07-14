//! Eval-gated learn/improve tool (dry_run default).

use crate::learn::{learn_improve, write_learn_report, LearnRequest};
use crate::paths::repo_root;
use crate::tools::types::{arg_bool, arg_u64, ToolError, ToolSpec};
use crate::tools::ToolContext;
use serde_json::{json, Value};

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "learn".into(),
        category: "mutate".into(),
        description: "Propose bounded agent markdown patches; keep only if exogenous score ≥ baseline. dry_run defaults true. Core agent locked.".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "targets": {"type": "array", "items": {"type": "string"}},
                "patches": {"type": "array", "items": {"type": "string"}},
                "dry_run": {"type": "boolean"},
                "max_diff_lines": {"type": "integer"},
                "require_substrings": {"type": "array", "items": {"type": "string"}}
            },
            "required": ["targets"]
        }),
    }
}

pub fn run(ctx: &ToolContext, args: &Value) -> Result<Value, ToolError> {
    let targets: Vec<String> = args
        .get("targets")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let patches: Vec<String> = args
        .get("patches")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let require_substrings: Vec<String> = args
        .get("require_substrings")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let req = LearnRequest {
        targets,
        patches,
        dry_run: arg_bool(args, "dry_run", true),
        max_diff_lines: arg_u64(args, "max_diff_lines", 40) as usize,
        require_substrings,
    };

    let report = learn_improve(&ctx.agents_dir, &req).map_err(|e| ToolError::Msg(e.to_string()))?;
    let path = write_learn_report(
        &report,
        repo_root().join("evals/runs").join(format!(
            "learn-{}.json",
            chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
        )),
    )
    .map_err(|e| ToolError::Msg(e.to_string()))?;

    Ok(json!({
        "dry_run": report.dry_run,
        "kept": report.kept,
        "reverted": report.reverted,
        "rejected": report.rejected,
        "results": report.results,
        "report_path": path.display().to_string(),
    }))
}
