//! Host-runnable eval for any agent (including orchestrator). Depth ≤ 1.

use crate::eval::{run_agent_eval, write_eval_report, EvalError};
use crate::paths::repo_root;
use crate::tools::types::{arg_str, ToolError, ToolSpec};
use crate::tools::ToolContext;
use serde_json::{json, Value};

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "run_eval".into(),
        category: "mutate".into(),
        description: "Run structural tool_plan eval for an agent id (host-scored). Nested self-eval is rejected.".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "agent_id": {"type": "string", "description": "category/name"},
                "track": {"type": "string", "description": "default tool_plan"}
            },
            "required": ["agent_id"]
        }),
    }
}

pub fn run(_ctx: &ToolContext, args: &Value) -> Result<Value, ToolError> {
    let agent_id =
        arg_str(args, "agent_id").ok_or_else(|| ToolError::Args("agent_id required".into()))?;
    let track = arg_str(args, "track").unwrap_or_else(|| "tool_plan".into());
    match run_agent_eval(&agent_id, &track) {
        Ok(report) => {
            let path = write_eval_report(&report, repo_root().join("evals/runs"))
                .map_err(|e| ToolError::Msg(e.to_string()))?;
            Ok(json!({
                "ok": report.summary.tool_plan_accuracy >= 1.0,
                "agent": report.agent,
                "accuracy": report.summary.tool_plan_accuracy,
                "total": report.summary.total,
                "correct": report.summary.correct,
                "report_id": report.id,
                "report_path": path.display().to_string(),
                "cases": report.cases.iter().map(|c| json!({
                    "id": c.id,
                    "correct": c.correct,
                    "tools_called": c.tools_called,
                    "facts": c.facts,
                })).collect::<Vec<_>>(),
            }))
        }
        Err(EvalError::DepthExceeded) => Err(ToolError::Msg("eval_depth_exceeded".into())),
        Err(e) => Err(ToolError::Msg(e.to_string())),
    }
}
