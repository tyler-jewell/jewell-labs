//! Create/update specialty agents under agents/{cat}/{name}.md (core locked).

use crate::agents::write_agent_file;
use crate::eval::{require_green_eval, write_eval_report};
use crate::paths::repo_root;
use crate::tools::types::{arg_bool, arg_str, ToolError, ToolSpec};
use crate::tools::ToolContext;
use serde_json::{json, Value};

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "write_agent".into(),
        category: "mutate".into(),
        description: "Create or replace an agent markdown file under agents/{category}/{name}.md. Core orchestrator is write-locked. Optionally require green structural eval before success.".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "id": {"type": "string", "description": "category/name"},
                "markdown": {"type": "string", "description": "full agent markdown with frontmatter"},
                "require_eval": {"type": "boolean", "description": "if true, run structural eval and fail if red (default true)"}
            },
            "required": ["id", "markdown"]
        }),
    }
}

pub fn run(ctx: &ToolContext, args: &Value) -> Result<Value, ToolError> {
    let id = arg_str(args, "id").ok_or_else(|| ToolError::Args("id required".into()))?;
    let markdown =
        arg_str(args, "markdown").ok_or_else(|| ToolError::Args("markdown required".into()))?;
    let require_eval = arg_bool(args, "require_eval", true);

    let path = write_agent_file(&ctx.agents_dir, &id, &markdown)
        .map_err(|e| ToolError::Msg(e.to_string()))?;

    let ds = crate::eval::dataset_path_for(&id);
    if !ds.is_file() {
        if let Some(parent) = ds.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let tools: Vec<String> = crate::schema::parse_frontmatter(&markdown)
            .map(|(fm, _)| fm.tools)
            .unwrap_or_default();
        let req = if tools.is_empty() {
            "list_tools".to_string()
        } else {
            tools
                .iter()
                .filter(|t| t.as_str() != "*")
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        };
        let body = format!(
            "# eval dataset for {id}\n\n## case: smoke\ntrack: tool_plan\nprompt: \"Smoke tools\"\nrequire_tools: [{req}]\n"
        );
        let _ = std::fs::write(&ds, body);
    }

    let mut eval_report = None;
    if require_eval {
        match require_green_eval(&id) {
            Ok(r) => {
                let runs = repo_root().join("evals/runs");
                let _ = write_eval_report(&r, &runs);
                eval_report = Some(json!({
                    "ok": true,
                    "accuracy": r.summary.tool_plan_accuracy,
                    "id": r.id,
                }));
            }
            Err(e) => {
                return Err(ToolError::Msg(format!(
                    "write ok but publish blocked: eval not green: {e}"
                )));
            }
        }
    }

    Ok(json!({
        "path": path.display().to_string(),
        "id": id,
        "dataset": ds.display().to_string(),
        "eval": eval_report,
    }))
}
