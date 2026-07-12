use crate::agents::list_agents;
use crate::schema::LATEST_SCHEMA_VERSION;
use crate::tools::{builtin_tools, ToolContext, ToolError, ToolSpec};
use chrono::Utc;
use serde_json::{json, Value};

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "app_status".into(),
        category: "introspect".into(),
        description:
            "Introspect app runtime: paths, schema version, agent/session counts, registry presence."
                .into(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }),
    }
}

pub fn run(ctx: &ToolContext, _args: &Value) -> Result<Value, ToolError> {
    let agents = list_agents(&ctx.agents_dir).map_err(|e| ToolError::Msg(e.to_string()))?;
    let (sessions, messages) = ctx
        .sessions
        .count_all()
        .map_err(|e| ToolError::Msg(e.to_string()))?;
    Ok(json!({
        "crate_root": ctx.crate_root.display().to_string(),
        "repo_root": ctx.repo_root.display().to_string(),
        "agents_dir": ctx.agents_dir.display().to_string(),
        "registry_path": ctx.registry_path.display().to_string(),
        "sessions_dir": ctx.sessions.root.display().to_string(),
        "latest_schema_version": LATEST_SCHEMA_VERSION,
        "agent_count": agents.len(),
        "session_count": sessions,
        "message_count": messages,
        "registry_present": ctx.registry_path.is_file(),
        "caller_agent": ctx.caller_agent,
        "builtin_tool_count": builtin_tools().len(),
        "allowed_tools": ctx.allowed_tools,
        "now": Utc::now().to_rfc3339(),
    }))
}
