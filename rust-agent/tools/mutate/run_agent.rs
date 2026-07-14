//! Nested agent run (orchestrator → specialist). Depth ≤ 1; dry_run skips LLM.

use crate::agents::{load_agent, CORE_AGENT_ID};
use crate::chat::ChatEndpoint;
use crate::registry::ModelRegistry;
use crate::tools::types::{arg_bool, arg_str, arg_u64, ToolError, ToolSpec, MAX_TOOL_ROUNDS};
use crate::tools::{system_with_tools, ToolContext};
use serde_json::{json, Value};
use std::cell::Cell;

thread_local! {
    static AGENT_RUN_DEPTH: Cell<u32> = const { Cell::new(0) };
}

pub fn get_agent_run_depth() -> u32 {
    AGENT_RUN_DEPTH.with(|d| d.get())
}

pub fn set_agent_run_depth(depth: u32) {
    AGENT_RUN_DEPTH.with(|d| d.set(depth));
}

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "run_agent".into(),
        category: "mutate".into(),
        description: "Run a specialty agent with a user message (depth ≤1). dry_run=true loads agent metadata only (no LLM).".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "agent_id": {"type": "string"},
                "message": {"type": "string"},
                "max_rounds": {"type": "integer"},
                "dry_run": {"type": "boolean", "description": "default false; if true, no LLM call"}
            },
            "required": ["agent_id"]
        }),
    }
}

pub fn run(ctx: &ToolContext, args: &Value) -> Result<Value, ToolError> {
    let agent_id =
        arg_str(args, "agent_id").ok_or_else(|| ToolError::Args("agent_id required".into()))?;
    let message = arg_str(args, "message").unwrap_or_default();
    let dry_run = arg_bool(args, "dry_run", false);
    let max_rounds =
        (arg_u64(args, "max_rounds", MAX_TOOL_ROUNDS as u64) as usize).min(MAX_TOOL_ROUNDS);

    if get_agent_run_depth() >= 1 {
        return Err(ToolError::Msg("agent_run_depth_exceeded".into()));
    }
    if agent_id == CORE_AGENT_ID {
        return Err(ToolError::Msg(
            "cannot run_agent on core/orchestrator".into(),
        ));
    }
    if ctx.caller_agent.as_deref() == Some(agent_id.as_str()) {
        return Err(ToolError::Msg("cannot run_agent on self".into()));
    }

    let doc = load_agent(&ctx.agents_dir, &agent_id).map_err(|e| ToolError::Msg(e.to_string()))?;
    let tools = doc.frontmatter.tools.clone();

    if dry_run || message.is_empty() {
        return Ok(json!({
            "agent_id": agent_id,
            "dry_run": true,
            "model": doc.frontmatter.default_model,
            "tools": tools,
            "role": doc.frontmatter.role,
            "note": "metadata only; no LLM",
        }));
    }

    let registry =
        ModelRegistry::load(&ctx.registry_path).map_err(|e| ToolError::Msg(e.to_string()))?;
    let model = registry
        .resolve(&doc.frontmatter.default_model)
        .map_err(|e| ToolError::Msg(e.to_string()))?;
    let endpoint = ChatEndpoint::from_agent(&doc, &model);
    let system = system_with_tools(&doc.body, &tools);
    let child_ctx = ToolContext::from_paths(
        ctx.agents_dir.clone(),
        ctx.registry_path.clone(),
        ctx.sessions.root.clone(),
        ctx.crate_root.clone(),
        ctx.repo_root.clone(),
        Some(agent_id.clone()),
        tools,
    );

    set_agent_run_depth(1);
    let result = run_nested(&endpoint, &system, &message, &child_ctx, max_rounds);
    set_agent_run_depth(0);
    let run = result?;

    Ok(json!({
        "agent_id": agent_id,
        "dry_run": false,
        "final_text": run.final_text,
        "tools_called": run.tool_names_called(),
        "tool_rounds_ok": run.all_tool_ok(),
        "rounds_used": run.rounds_used,
    }))
}

fn run_nested(
    endpoint: &ChatEndpoint,
    system: &str,
    message: &str,
    child_ctx: &ToolContext,
    max_rounds: usize,
) -> Result<crate::agent_run::AgentRunResult, ToolError> {
    let handle = tokio::runtime::Handle::try_current().map_err(|_| {
        ToolError::Msg("run_agent live path requires Tokio runtime (use dry_run offline)".into())
    })?;
    handle
        .block_on(async {
            crate::agent_run::run_agent_with_tools_max(
                endpoint,
                system,
                &[],
                message,
                child_ctx,
                max_rounds,
            )
            .await
        })
        .map_err(|e| ToolError::Msg(e.to_string()))
}
