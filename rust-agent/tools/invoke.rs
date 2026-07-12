//! Tool registry and invocation.

use super::allowlist::{resolve_allowlist, tool_matches_allowlist};
use super::context::ToolContext;
use super::introspect;
use super::mutate;
use super::sessions;
use super::types::{ToolCall, ToolError, ToolResult, ToolSpec};
use serde_json::{json, Value};

pub(crate) type ToolFn = fn(&ToolContext, &Value) -> Result<Value, ToolError>;

pub(crate) struct ToolEntry {
    pub spec: ToolSpec,
    pub run: ToolFn,
}

pub(crate) fn all_entries() -> Vec<ToolEntry> {
    vec![
        // introspect
        entry(introspect::list_tools::spec(), introspect::list_tools::run),
        entry(introspect::app_status::spec(), introspect::app_status::run),
        entry(introspect::list_agents::spec(), introspect::list_agents::run),
        entry(introspect::get_agent::spec(), introspect::get_agent::run),
        entry(
            introspect::certify_agent::spec(),
            introspect::certify_agent::run,
        ),
        entry(introspect::get_schema::spec(), introspect::get_schema::run),
        entry(introspect::list_models::spec(), introspect::list_models::run),
        // sessions (search folded into list_sessions via query=)
        entry(sessions::list_sessions::spec(), sessions::list_sessions::run),
        entry(sessions::get_session::spec(), sessions::get_session::run),
        entry(
            sessions::upsert_session::spec(),
            sessions::upsert_session::run,
        ),
        // mutate (orchestrator write / eval / learn / research)
        entry(mutate::write_agent::spec(), mutate::write_agent::run),
        entry(mutate::write_tool::spec(), mutate::write_tool::run),
        entry(mutate::run_eval::spec(), mutate::run_eval::run),
        entry(mutate::learn::spec(), mutate::learn::run),
        entry(
            mutate::research_models::spec(),
            mutate::research_models::run,
        ),
    ]
}

fn entry(spec: ToolSpec, run: ToolFn) -> ToolEntry {
    ToolEntry { spec, run }
}

/// All registered tools (from tools/{category}/*).
pub fn builtin_tools() -> Vec<ToolSpec> {
    all_entries().into_iter().map(|e| e.spec).collect()
}

/// Tool names only.
pub fn all_tool_names() -> Vec<String> {
    builtin_tools().into_iter().map(|t| t.name).collect()
}

pub fn invoke_tool(ctx: &ToolContext, name: &str, arguments: &Value) -> ToolResult {
    let entries = all_entries();
    let Some(entry) = entries.iter().find(|e| e.spec.name == name) else {
        return ToolResult {
            name: name.to_string(),
            ok: false,
            result: json!({ "error": format!("unknown tool: {name}") }),
        };
    };

    if !tool_matches_allowlist(&ctx.allowed_tools, &entry.spec.name, &entry.spec.category) {
        return ToolResult {
            name: name.to_string(),
            ok: false,
            result: json!({
                "error": format!("tool '{name}' is not allowed for this agent"),
                "allowed": resolve_allowlist(&ctx.allowed_tools),
            }),
        };
    }

    match (entry.run)(ctx, arguments) {
        Ok(v) => ToolResult {
            name: name.to_string(),
            ok: true,
            result: v,
        },
        Err(e) => ToolResult {
            name: name.to_string(),
            ok: false,
            result: json!({ "error": e.to_string() }),
        },
    }
}

pub fn invoke_tool_call(ctx: &ToolContext, call: &ToolCall) -> ToolResult {
    invoke_tool(ctx, &call.name, &call.arguments)
}
