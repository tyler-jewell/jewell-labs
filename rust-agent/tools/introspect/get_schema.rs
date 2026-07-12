use crate::schema::{schema_summary, LATEST_SCHEMA_VERSION};
use crate::tools::{ToolContext, ToolError, ToolSpec};
use serde_json::{json, Value};

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "get_schema".into(),
        category: "introspect".into(),
        description: "Return latest agent frontmatter schema version and field documentation."
            .into(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }),
    }
}

pub fn run(_ctx: &ToolContext, _args: &Value) -> Result<Value, ToolError> {
    Ok(json!({
        "latest_schema_version": LATEST_SCHEMA_VERSION,
        "fields": schema_summary(),
    }))
}
