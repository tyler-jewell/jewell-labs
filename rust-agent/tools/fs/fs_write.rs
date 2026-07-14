//! Write a file under the calling agent's sandbox `fs/` root.

use super::{resolve_in_sandbox, sandbox_root};
use crate::tools::{arg_str, ToolContext, ToolError, ToolSpec};
use serde_json::{json, Value};
use std::fs;
use std::io::Write;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "fs_write".into(),
        category: "fs".into(),
        description: "Write a text file under the agent sandbox (relative path, e.g. out.csv)."
            .into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Relative path under the sandbox root"},
                "content": {"type": "string", "description": "Full file contents to write"}
            },
            "required": ["path", "content"]
        }),
    }
}

pub fn run(ctx: &ToolContext, args: &Value) -> Result<Value, ToolError> {
    let rel = arg_str(args, "path").ok_or_else(|| ToolError::Args("path required".into()))?;
    let content =
        arg_str(args, "content").ok_or_else(|| ToolError::Args("content required".into()))?;
    if rel.trim().is_empty() || rel.trim() == "." {
        return Err(ToolError::Args(
            "path must be a file under the sandbox".into(),
        ));
    }
    let root = sandbox_root(ctx)?;
    let path = resolve_in_sandbox(&root, &rel)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| ToolError::Msg(e.to_string()))?;
    }
    let mut f = fs::File::create(&path).map_err(|e| ToolError::Msg(e.to_string()))?;
    f.write_all(content.as_bytes())
        .map_err(|e| ToolError::Msg(e.to_string()))?;
    f.sync_all().map_err(|e| ToolError::Msg(e.to_string()))?;
    Ok(json!({
        "ok": true,
        "path": rel,
        "bytes": content.len(),
        "sandbox": root.display().to_string(),
    }))
}
