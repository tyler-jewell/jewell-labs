use crate::agents::parse_agent_ref;
use crate::tools::{arg_str, ToolContext, ToolError, ToolSpec};
use serde_json::{json, Value};

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "get_session".into(),
        category: "sessions".into(),
        description: "Read a full chat log by agent_id + session_id.".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "agent_id": { "type": "string" },
                "agent_stem": { "type": "string" },
                "session_id": { "type": "string" },
                "max_messages": { "type": "integer", "description": "Optional cap from the end of the log" }
            },
            "required": ["session_id"],
            "additionalProperties": false
        }),
    }
}

pub fn run(ctx: &ToolContext, args: &Value) -> Result<Value, ToolError> {
    let session_id =
        arg_str(args, "session_id").ok_or_else(|| ToolError::Args("session_id required".into()))?;
    let agent = arg_str(args, "agent_id")
        .or_else(|| arg_str(args, "agent_stem"))
        .or_else(|| ctx.caller_agent.clone())
        .ok_or_else(|| ToolError::Args("agent_id required (or call from an agent chat)".into()))?;
    parse_agent_ref(&agent).map_err(|e| ToolError::Args(e.to_string()))?;
    let mut session = ctx
        .sessions
        .get(&agent, &session_id)
        .map_err(|e| ToolError::Msg(e.to_string()))?;
    if let Some(max) = args.get("max_messages").and_then(|v| v.as_u64()) {
        let max = max as usize;
        if session.messages.len() > max {
            session.messages = session.messages.split_off(session.messages.len() - max);
        }
    }
    Ok(json!(session))
}
