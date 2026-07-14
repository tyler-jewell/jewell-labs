//! List files under the calling agent's sandbox `fs/` root.

use super::{resolve_in_sandbox, sandbox_root};
use crate::tools::{arg_str, ToolContext, ToolError, ToolSpec};
use serde_json::{json, Value};
use std::fs;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "fs_list".into(),
        category: "fs".into(),
        description: "List sandbox directory entries (relative path, default \".\").".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Relative directory under fs/ (default \".\")"}
            }
        }),
    }
}

pub fn run(ctx: &ToolContext, args: &Value) -> Result<Value, ToolError> {
    let rel = arg_str(args, "path").unwrap_or_else(|| ".".into());
    let root = sandbox_root(ctx)?;
    let path = if rel == "." || rel.is_empty() {
        root.clone()
    } else {
        resolve_in_sandbox(&root, &rel)?
    };
    if !path.is_dir() {
        return Err(ToolError::Msg(format!("not a directory: {rel}")));
    }
    let mut entries = Vec::new();
    for ent in fs::read_dir(&path).map_err(|e| ToolError::Msg(e.to_string()))? {
        let ent = ent.map_err(|e| ToolError::Msg(e.to_string()))?;
        let name = ent.file_name().to_string_lossy().to_string();
        let ft = ent.file_type().map_err(|e| ToolError::Msg(e.to_string()))?;
        entries.push(json!({
            "name": name,
            "is_dir": ft.is_dir(),
            "is_file": ft.is_file(),
        }));
    }
    entries.sort_by(|a, b| {
        a["name"]
            .as_str()
            .unwrap_or("")
            .cmp(b["name"].as_str().unwrap_or(""))
    });
    Ok(json!({
        "ok": true,
        "path": rel,
        "count": entries.len(),
        "entries": entries,
    }))
}
