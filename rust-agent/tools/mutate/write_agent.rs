//! Create/update specialty agents — fail-closed: red eval rolls back the write.

use crate::agents::write_agent_file;
use crate::eval::{require_green_eval, write_eval_report};
use crate::jail::agent_file_rel;
use crate::paths::repo_root;
use crate::tools::types::{arg_bool, arg_str, ToolError, ToolSpec};
use crate::tools::ToolContext;
use serde_json::{json, Value};
use std::path::PathBuf;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "write_agent".into(),
        category: "mutate".into(),
        description: "Create/replace agents/{category}/{name}.md (core locked). Default require_eval=true: write is rolled back if structural eval is red.".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "id": {"type": "string"},
                "markdown": {"type": "string"},
                "require_eval": {"type": "boolean", "description": "default true; red eval rolls back file"}
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

    let rel = agent_file_rel(&id).map_err(|e| ToolError::Msg(e.to_string()))?;
    let dest = ctx.agents_dir.join(&rel);
    let prior = if dest.is_file() {
        Some(std::fs::read_to_string(&dest).map_err(|e| ToolError::Msg(e.to_string()))?)
    } else {
        None
    };

    let path = write_agent_file(&ctx.agents_dir, &id, &markdown)
        .map_err(|e| ToolError::Msg(e.to_string()))?;

    let ds = crate::eval::dataset_path_for(&id);
    let mut created_dataset = false;
    if !ds.is_file() {
        scaffold_dataset(&ds, &id, &markdown)?;
        created_dataset = true;
    }

    let mut eval_report = None;
    if require_eval {
        match require_green_eval(&id) {
            Ok(r) => {
                let _ = write_eval_report(&r, repo_root().join("evals/runs"));
                eval_report = Some(json!({
                    "ok": true,
                    "accuracy": r.summary.tool_plan_accuracy,
                    "id": r.id,
                }));
            }
            Err(e) => {
                rollback(&ctx.agents_dir, &id, prior.as_deref(), &ds, created_dataset)?;
                return Err(ToolError::Msg(format!(
                    "publish blocked (rolled back): eval not green: {e}"
                )));
            }
        }
    }

    Ok(json!({
        "path": path.display().to_string(),
        "id": id,
        "dataset": ds.display().to_string(),
        "eval": eval_report,
        "rolled_back": false,
    }))
}

fn scaffold_dataset(ds: &PathBuf, id: &str, markdown: &str) -> Result<(), ToolError> {
    if let Some(parent) = ds.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ToolError::Msg(e.to_string()))?;
    }
    let tools: Vec<String> = crate::schema::parse_frontmatter(markdown)
        .map(|(fm, _)| fm.tools)
        .unwrap_or_default();
    let req = if tools.is_empty() {
        "list_tools".into()
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
    std::fs::write(ds, body).map_err(|e| ToolError::Msg(e.to_string()))
}

fn rollback(
    agents_dir: &std::path::Path,
    id: &str,
    prior: Option<&str>,
    ds: &PathBuf,
    created_dataset: bool,
) -> Result<(), ToolError> {
    match prior {
        Some(old) => {
            write_agent_file(agents_dir, id, old).map_err(|e| ToolError::Msg(e.to_string()))?;
        }
        None => {
            let rel = agent_file_rel(id).map_err(|e| ToolError::Msg(e.to_string()))?;
            let p = agents_dir.join(rel);
            let _ = std::fs::remove_file(p);
        }
    }
    if created_dataset {
        let _ = std::fs::remove_file(ds);
    }
    Ok(())
}
