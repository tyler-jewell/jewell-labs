//! Agent-scoped sandbox filesystem tools.
//!
//! Root: `agents/{category}/{name}/fs/` (or `ToolContext.sandbox_override` for evals).
//! Paths are relative to that root only — no absolute paths, no `..`.

pub mod fs_list;
pub mod fs_read;
pub mod fs_write;

use crate::agents::parse_agent_ref;
use crate::jail::JailError;
use crate::tools::{ToolContext, ToolError};
use std::fs;
use std::path::{Component, Path, PathBuf};

/// Resolve the sandbox root for the calling agent.
pub fn sandbox_root(ctx: &ToolContext) -> Result<PathBuf, ToolError> {
    if let Some(ovr) = &ctx.sandbox_override {
        // Defense in depth: never trust a client path that escaped eval_guards.
        crate::eval_guards::assert_sandbox_override_allowed(ovr, &ctx.repo_root)
            .map_err(ToolError::Msg)?;
        fs::create_dir_all(ovr).map_err(|e| ToolError::Msg(e.to_string()))?;
        return Ok(ovr.clone());
    }
    let agent = ctx
        .caller_agent
        .as_deref()
        .ok_or_else(|| ToolError::Msg("caller_agent required for fs tools".into()))?;
    let (cat, name) = parse_agent_ref(agent).map_err(|e| ToolError::Args(e.to_string()))?;
    // agents/{cat}/{name}/fs — agent file is {cat}/{name}.md alongside this dir
    let root = ctx.agents_dir.join(cat).join(name).join("fs");
    fs::create_dir_all(&root).map_err(|e| ToolError::Msg(e.to_string()))?;
    Ok(root)
}

/// Clean relative path under sandbox; reject abs / `..` / null.
pub fn resolve_in_sandbox(root: &Path, rel: &str) -> Result<PathBuf, ToolError> {
    if rel.contains('\0') {
        return Err(ToolError::Args("null byte in path".into()));
    }
    let p = Path::new(rel);
    if p.is_absolute() {
        return Err(ToolError::Args(format!("absolute path rejected: {rel}")));
    }
    let mut clean = PathBuf::new();
    for c in p.components() {
        match c {
            Component::Normal(s) => clean.push(s),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(ToolError::Args(format!("path traversal rejected: {rel}")));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(ToolError::Args(format!("absolute path rejected: {rel}")));
            }
        }
    }
    // empty rel => sandbox root itself (for list)
    let abs = if clean.as_os_str().is_empty() {
        root.to_path_buf()
    } else {
        root.join(&clean)
    };
    // Lexical: joined path must stay under root
    let root_c = fs::canonicalize(root).map_err(|e| ToolError::Msg(e.to_string()))?;
    if abs.exists() {
        let meta = fs::symlink_metadata(&abs).map_err(|e| ToolError::Msg(e.to_string()))?;
        if meta.file_type().is_symlink() {
            return Err(ToolError::Msg(format!("symlink refused: {rel}")));
        }
        let canon = fs::canonicalize(&abs).map_err(|e| ToolError::Msg(e.to_string()))?;
        if !canon.starts_with(&root_c) {
            return Err(ToolError::Msg(format!("path outside sandbox: {rel}")));
        }
        Ok(canon)
    } else {
        // Ensure parent chain stays under root
        if let Some(parent) = abs.parent() {
            if parent.exists() {
                let pmeta =
                    fs::symlink_metadata(parent).map_err(|e| ToolError::Msg(e.to_string()))?;
                if pmeta.file_type().is_symlink() {
                    return Err(ToolError::Msg("symlink parent refused".into()));
                }
                let pcanon = fs::canonicalize(parent).map_err(|e| ToolError::Msg(e.to_string()))?;
                if !pcanon.starts_with(&root_c) {
                    return Err(ToolError::Msg(format!("path outside sandbox: {rel}")));
                }
            }
        }
        Ok(abs)
    }
}

pub fn jail_err(e: JailError) -> ToolError {
    ToolError::Msg(e.to_string())
}
