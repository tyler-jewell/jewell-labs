use crate::agents::{load_agent, parse_agent_ref};
use crate::schema::certify_agent_markdown;
use crate::tools::{arg_bool, arg_str, ToolContext, ToolError, ToolSpec};
use serde_json::{json, Value};

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "get_agent".into(),
        category: "introspect".into(),
        description:
            "Load one agent by id `category/name`: frontmatter, tools allowlist, certification, body."
                .into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Agent id e.g. system/orchestrator" },
                "stem": { "type": "string", "description": "Alias for id (legacy)" },
                "include_body": { "type": "boolean", "description": "Include system prompt (default true)" }
            },
            "additionalProperties": false
        }),
    }
}

pub fn run(ctx: &ToolContext, args: &Value) -> Result<Value, ToolError> {
    let id = arg_str(args, "id")
        .or_else(|| arg_str(args, "stem"))
        .ok_or_else(|| ToolError::Args("id (or stem) required".into()))?;
    parse_agent_ref(&id).map_err(|e| ToolError::Args(e.to_string()))?;
    let include_body = arg_bool(args, "include_body", true);
    let doc = load_agent(&ctx.agents_dir, &id).map_err(|e| ToolError::Msg(e.to_string()))?;
    let text = std::fs::read_to_string(&doc.path).unwrap_or_default();
    let cert = certify_agent_markdown(&text);
    Ok(json!({
        "id": doc.id,
        "category": doc.category,
        "name": doc.stem,
        "path": doc.path,
        "frontmatter": doc.frontmatter,
        "body": if include_body { Value::String(doc.body) } else { Value::Null },
        "certification": cert,
    }))
}
