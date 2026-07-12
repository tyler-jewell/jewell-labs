//! Tool type definitions and argument helpers.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Max tool-loop rounds per chat turn.
pub const MAX_TOOL_ROUNDS: usize = 8;

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("{0}")]
    Msg(String),
    #[error("unknown tool: {0}")]
    Unknown(String),
    #[error("invalid arguments: {0}")]
    Args(String),
    #[error("tool '{0}' is not allowed for this agent")]
    NotAllowed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub category: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    #[serde(default)]
    pub arguments: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub name: String,
    pub ok: bool,
    pub result: Value,
}

// Shared arg helpers for tool modules
pub fn arg_str(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

pub fn arg_bool(args: &Value, key: &str, default: bool) -> bool {
    args.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
}

pub fn arg_u64(args: &Value, key: &str, default: u64) -> u64 {
    args.get(key)
        .and_then(|v| v.as_u64())
        .or_else(|| {
            args.get(key)
                .and_then(|v| v.as_i64())
                .map(|i| i.max(0) as u64)
        })
        .unwrap_or(default)
}
