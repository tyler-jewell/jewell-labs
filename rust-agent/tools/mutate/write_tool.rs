//! Write shared tool draft under tools/ or agent-local under agents/{id}/tools/.

use crate::agents::{parse_agent_ref, validate_segment};
use crate::jail::WriteJail;
use crate::tools::types::{arg_str, ToolError, ToolSpec};
use crate::tools::ToolContext;
use serde_json::{json, Value};

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "write_tool".into(),
        category: "mutate".into(),
        description: "Write a tool draft: shared tools/{category}/{name}.rs (propose only) or agent-local agents/{cat}/{name}/tools/{tool}.md. Never writes src/.".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "scope": {"type": "string", "description": "shared | agent_local"},
                "category": {"type": "string", "description": "for shared tools category folder"},
                "name": {"type": "string", "description": "tool name stem"},
                "content": {"type": "string", "description": "file body"},
                "agent_id": {"type": "string", "description": "required for agent_local"}
            },
            "required": ["scope", "name", "content"]
        }),
    }
}

pub fn run(ctx: &ToolContext, args: &Value) -> Result<Value, ToolError> {
    let scope = arg_str(args, "scope").ok_or_else(|| ToolError::Args("scope required".into()))?;
    let name = arg_str(args, "name").ok_or_else(|| ToolError::Args("name required".into()))?;
    let content =
        arg_str(args, "content").ok_or_else(|| ToolError::Args("content required".into()))?;
    validate_segment(&name).map_err(|e| ToolError::Args(e.to_string()))?;

    match scope.as_str() {
        "shared" => {
            let category = arg_str(args, "category")
                .ok_or_else(|| ToolError::Args("category required".into()))?;
            validate_segment(&category).map_err(|e| ToolError::Args(e.to_string()))?;
            // Draft only — .rs.draft so it is not mistaken for live registerable source
            let jail = WriteJail::workspace_roots(&ctx.crate_root, &["tools"]);
            let rel = format!("tools/{category}/{name}.rs.draft");
            let path = jail
                .write_file(&rel, content.as_bytes())
                .map_err(|e| ToolError::Msg(e.to_string()))?;
            Ok(json!({
                "scope": "shared",
                "path": path.display().to_string(),
                "registered": false,
                "note": "draft only; host rebuild required to register",
            }))
        }
        "agent_local" => {
            let agent_id = arg_str(args, "agent_id")
                .ok_or_else(|| ToolError::Args("agent_id required".into()))?;
            parse_agent_ref(&agent_id).map_err(|e| ToolError::Args(e.to_string()))?;
            let (cat, stem) = parse_agent_ref(&agent_id).unwrap();
            // under agents_dir: {cat}/{stem}/tools/{name}.md
            let jail = WriteJail::agents_dir(&ctx.agents_dir);
            let rel = format!("{cat}/{stem}/tools/{name}.md");
            let path = jail
                .write_file(&rel, content.as_bytes())
                .map_err(|e| ToolError::Msg(e.to_string()))?;
            Ok(json!({
                "scope": "agent_local",
                "agent_id": agent_id,
                "path": path.display().to_string(),
                "visible_to": agent_id,
            }))
        }
        other => Err(ToolError::Args(format!(
            "scope must be shared|agent_local, got {other}"
        ))),
    }
}
