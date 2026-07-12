use crate::agents::list_agents;
use crate::tools::{ToolContext, ToolError, ToolSpec};
use serde_json::{json, Value};

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "list_agents".into(),
        category: "introspect".into(),
        description:
            "List all agents under agents/{category}/{name}.md with role, model, tools, cert status."
                .into(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }),
    }
}

pub fn run(ctx: &ToolContext, _args: &Value) -> Result<Value, ToolError> {
    let items = list_agents(&ctx.agents_dir).map_err(|e| ToolError::Msg(e.to_string()))?;
    let agents: Vec<Value> = items
        .into_iter()
        .map(|a| {
            json!({
                "id": a.id,
                "category": a.category,
                "name": a.name,
                "description": a.description,
                "role": a.role,
                "default_model": a.default_model,
                "tools": a.tools,
                "certification_ok": a.certification.ok,
                "certification_errors": a.certification.errors,
                "path": a.path,
            })
        })
        .collect();
    Ok(json!({ "agents": agents }))
}
