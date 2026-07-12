use crate::tools::{filter_tools_for_agent, ToolContext, ToolError, ToolSpec};
use serde_json::{json, Value};

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "list_tools".into(),
        category: "introspect".into(),
        description: "List tools available to the current agent (respects frontmatter allowlist)."
            .into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "all": {
                    "type": "boolean",
                    "description": "If true and agent allows this tool, list the full registry (still only if '*' or list_tools alone)"
                }
            },
            "additionalProperties": false
        }),
    }
}

pub fn run(ctx: &ToolContext, args: &Value) -> Result<Value, ToolError> {
    let all = args
        .get("all")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let tools = if all && ctx.allowed_tools.iter().any(|p| p == "*") {
        crate::tools::builtin_tools()
    } else {
        filter_tools_for_agent(&ctx.allowed_tools)
    };
    Ok(json!({
        "tools": tools,
        "allowlist": ctx.allowed_tools,
        "count": tools.len(),
    }))
}
