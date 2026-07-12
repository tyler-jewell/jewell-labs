use crate::agents::parse_agent_ref;
use crate::sessions::{ChatSession, SessionMessage};
use crate::tools::{arg_str, ToolContext, ToolError, ToolSpec};
use chrono::Utc;
use serde_json::{json, Value};

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "upsert_session".into(),
        category: "sessions".into(),
        description: "Create or update a chat session (id, messages, optional title).".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "agent_id": { "type": "string" },
                "agent_stem": { "type": "string" },
                "session_id": { "type": "string" },
                "title": { "type": "string" },
                "messages": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "role": { "type": "string" },
                            "content": { "type": "string" }
                        },
                        "required": ["role", "content"]
                    }
                }
            },
            "additionalProperties": false
        }),
    }
}

pub fn run(ctx: &ToolContext, args: &Value) -> Result<Value, ToolError> {
    let agent = arg_str(args, "agent_id")
        .or_else(|| arg_str(args, "agent_stem"))
        .or_else(|| ctx.caller_agent.clone())
        .ok_or_else(|| ToolError::Args("agent_id required".into()))?;
    parse_agent_ref(&agent).map_err(|e| ToolError::Args(e.to_string()))?;
    let session_id = arg_str(args, "session_id").unwrap_or_default();
    let title = arg_str(args, "title");
    let messages = args
        .get("messages")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    Some(SessionMessage {
                        role: m.get("role")?.as_str()?.to_string(),
                        content: m.get("content")?.as_str()?.to_string(),
                        ts: None,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let existing = if !session_id.is_empty() {
        ctx.sessions.get(&agent, &session_id).ok()
    } else {
        None
    };

    let session = ChatSession {
        id: if session_id.is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            session_id
        },
        agent_stem: agent,
        created: existing
            .as_ref()
            .map(|s| s.created)
            .unwrap_or_else(Utc::now),
        updated: Utc::now(),
        title: title.or_else(|| existing.and_then(|s| s.title)),
        messages,
    };
    let saved = ctx
        .sessions
        .upsert(session)
        .map_err(|e| ToolError::Msg(e.to_string()))?;
    Ok(json!({
        "id": saved.id,
        "agent_id": saved.agent_stem,
        "message_count": saved.messages.len(),
        "updated": saved.updated,
    }))
}
