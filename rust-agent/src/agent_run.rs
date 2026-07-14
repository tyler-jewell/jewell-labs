//! Shared agent tool loop (used by HTTP server and evals).

use crate::chat::{complete_chat, ChatEndpoint, ChatError, ChatMessage};
use crate::tools::{
    extract_tool_call, invoke_tool_call, ToolCall, ToolContext, ToolResult, MAX_TOOL_ROUNDS,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRound {
    pub round: usize,
    pub call: ToolCall,
    pub result: ToolResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRunResult {
    pub tool_rounds: Vec<ToolRound>,
    pub final_text: String,
    pub raw_assistant_messages: Vec<String>,
    pub rounds_used: usize,
}

impl AgentRunResult {
    pub fn tool_names_called(&self) -> Vec<String> {
        self.tool_rounds
            .iter()
            .map(|r| r.call.name.clone())
            .collect()
    }

    pub fn result_for(&self, name: &str) -> Option<&Value> {
        self.tool_rounds
            .iter()
            .rev()
            .find(|r| r.call.name == name && r.result.ok)
            .map(|r| &r.result.result)
    }

    pub fn all_tool_ok(&self) -> bool {
        !self.tool_rounds.is_empty() && self.tool_rounds.iter().all(|r| r.result.ok)
    }
}

/// Run the agent with host-side tool execution (same protocol as the console).
pub async fn run_agent_with_tools(
    endpoint: &ChatEndpoint,
    system: &str,
    history: &[ChatMessage],
    user: &str,
    tool_ctx: &ToolContext,
) -> Result<AgentRunResult, ChatError> {
    run_agent_with_tools_max(endpoint, system, history, user, tool_ctx, MAX_TOOL_ROUNDS).await
}

pub async fn run_agent_with_tools_max(
    endpoint: &ChatEndpoint,
    system: &str,
    history: &[ChatMessage],
    user: &str,
    tool_ctx: &ToolContext,
    max_rounds: usize,
) -> Result<AgentRunResult, ChatError> {
    let mut working_history: Vec<ChatMessage> = history.to_vec();
    let mut next_user = user.to_string();
    let mut tool_rounds = Vec::new();
    let mut raw_assistant_messages = Vec::new();

    for round in 0..max_rounds {
        let text = complete_chat(endpoint, system, &working_history, &next_user).await?;
        raw_assistant_messages.push(text.clone());

        if let Some(call) = extract_tool_call(&text) {
            let result = invoke_tool_call(tool_ctx, &call);
            tool_rounds.push(ToolRound {
                round,
                call: call.clone(),
                result: result.clone(),
            });

            working_history.push(ChatMessage {
                role: "user".into(),
                content: next_user.clone(),
            });
            working_history.push(ChatMessage {
                role: "assistant".into(),
                content: text,
            });
            // Mechanical payload only — multi-step policy lives in the agent system prompt.
            next_user = format!(
                "tool_result for {}:\n{}",
                result.name,
                serde_json::to_string_pretty(&result.result).unwrap_or_else(|_| "{}".into())
            );
            continue;
        }

        return Ok(AgentRunResult {
            tool_rounds,
            final_text: text,
            raw_assistant_messages,
            rounds_used: round + 1,
        });
    }

    Ok(AgentRunResult {
        tool_rounds,
        final_text: format!("(tool loop exceeded {max_rounds} rounds)"),
        raw_assistant_messages,
        rounds_used: max_rounds,
    })
}

/// Deterministic "perfect agent": run an explicit tool plan, then optional LLM summarize.
pub fn run_tool_plan(tool_ctx: &ToolContext, plan: &[(String, Value)]) -> AgentRunResult {
    let mut tool_rounds = Vec::new();
    for (i, (name, args)) in plan.iter().enumerate() {
        let call = ToolCall {
            name: name.clone(),
            arguments: args.clone(),
        };
        let result = invoke_tool_call(tool_ctx, &call);
        tool_rounds.push(ToolRound {
            round: i,
            call,
            result,
        });
    }
    let summary = synthesize_report_from_tools(&tool_rounds);
    AgentRunResult {
        rounds_used: tool_rounds.len(),
        tool_rounds,
        final_text: summary,
        raw_assistant_messages: vec![],
    }
}

/// Compact machine-readable report from tool results (used as gold-path answer).
pub fn synthesize_report_from_tools(rounds: &[ToolRound]) -> String {
    let mut report = serde_json::Map::new();
    for r in rounds {
        if r.result.ok {
            report.insert(r.call.name.clone(), r.result.result.clone());
        }
    }
    serde_json::to_string_pretty(&Value::Object(report)).unwrap_or_else(|_| "{}".into())
}
