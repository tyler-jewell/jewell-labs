//! List sessions; optional full-text search via `query`.

use crate::agents::parse_agent_ref;
use crate::tools::{arg_bool, arg_str, arg_u64, ToolContext, ToolError, ToolSpec};
use serde_json::{json, Value};

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "list_sessions".into(),
        category: "sessions".into(),
        description:
            "List server-side chat sessions. Optional query searches message text (merged search)."
                .into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "agent_id": { "type": "string" },
                "agent_stem": { "type": "string" },
                "all_agents": { "type": "boolean" },
                "query": { "type": "string", "description": "If set, full-text search messages instead of listing" },
                "limit": { "type": "integer", "description": "Max search hits (default 25)" }
            },
            "additionalProperties": false
        }),
    }
}

pub fn run(ctx: &ToolContext, args: &Value) -> Result<Value, ToolError> {
    let agent = arg_str(args, "agent_id").or_else(|| arg_str(args, "agent_stem"));
    if let Some(ref s) = agent {
        parse_agent_ref(s).map_err(|e| ToolError::Args(e.to_string()))?;
    }

    if let Some(query) = arg_str(args, "query") {
        let filter = if arg_bool(args, "all_agents", false) {
            None
        } else {
            agent.or_else(|| ctx.caller_agent.clone())
        };
        if let Some(ref s) = filter {
            parse_agent_ref(s).map_err(|e| ToolError::Args(e.to_string()))?;
        }
        let limit = arg_u64(args, "limit", 25) as usize;
        let hits = ctx
            .sessions
            .search(&query, filter.as_deref(), limit)
            .map_err(|e| ToolError::Msg(e.to_string()))?;
        return Ok(json!({
            "mode": "search",
            "query": query,
            "hits": hits,
            "count": hits.len(),
            "filter_agent": filter,
        }));
    }

    let all = arg_bool(args, "all_agents", false);
    let stem = if all {
        None
    } else {
        agent.or_else(|| ctx.caller_agent.clone())
    };
    if let Some(ref s) = stem {
        parse_agent_ref(s).map_err(|e| ToolError::Args(e.to_string()))?;
    }
    let list = ctx
        .sessions
        .list(stem.as_deref())
        .map_err(|e| ToolError::Msg(e.to_string()))?;
    Ok(json!({ "mode": "list", "sessions": list, "filter_agent": stem }))
}
