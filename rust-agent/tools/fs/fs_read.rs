//! Read a file from the calling agent's sandbox `fs/` root.

use super::{resolve_in_sandbox, sandbox_root};
use crate::tools::{arg_str, ToolContext, ToolError, ToolSpec};
use serde_json::{json, Value};
use std::fs;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "fs_read".into(),
        category: "fs".into(),
        description: "Read a text file from the agent sandbox (relative path, e.g. data.csv).".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Relative path under the sandbox root"}
            },
            "required": ["path"]
        }),
    }
}

pub fn run(ctx: &ToolContext, args: &Value) -> Result<Value, ToolError> {
    let rel = arg_str(args, "path").ok_or_else(|| ToolError::Args("path required".into()))?;
    let root = sandbox_root(ctx)?;
    let path = resolve_in_sandbox(&root, &rel)?;
    if !path.is_file() {
        return Err(ToolError::Msg(format!("not a file: {rel}")));
    }
    let content = fs::read_to_string(&path).map_err(|e| ToolError::Msg(e.to_string()))?;
    Ok(json!({
        "ok": true,
        "path": rel,
        "content": content,
        "bytes": content.len(),
    }))
}
