use crate::agents::{load_agent, parse_agent_ref};
use crate::schema::certify_agent_markdown;
use crate::tools::{arg_str, ToolContext, ToolError, ToolSpec};
use serde_json::{json, Value};

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "certify_agent".into(),
        category: "introspect".into(),
        description: "Certify an agent against the latest frontmatter schema (id and/or markdown)."
            .into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Existing agent id e.g. system/learner" },
                "stem": { "type": "string", "description": "Alias for id" },
                "markdown": { "type": "string", "description": "Raw markdown to certify without writing" }
            },
            "additionalProperties": false
        }),
    }
}

pub fn run(ctx: &ToolContext, args: &Value) -> Result<Value, ToolError> {
    if let Some(md) = arg_str(args, "markdown") {
        return Ok(json!(certify_agent_markdown(&md)));
    }
    let id = arg_str(args, "id")
        .or_else(|| arg_str(args, "stem"))
        .ok_or_else(|| ToolError::Args("provide id or markdown".into()))?;
    parse_agent_ref(&id).map_err(|e| ToolError::Args(e.to_string()))?;
    let doc = load_agent(&ctx.agents_dir, &id).map_err(|e| ToolError::Msg(e.to_string()))?;
    let text = std::fs::read_to_string(&doc.path).map_err(|e| ToolError::Msg(e.to_string()))?;
    Ok(json!(certify_agent_markdown(&text)))
}
