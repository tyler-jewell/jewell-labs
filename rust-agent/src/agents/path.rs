//! Agent path validation and resolution.

use super::list::AgentsError;
use crate::schema::is_valid_agent_name;
use std::fs;
use std::path::{Path, PathBuf};

/// Validate a single path segment (category or name).
pub fn validate_segment(seg: &str) -> Result<(), AgentsError> {
    if seg.is_empty()
        || seg.contains('/')
        || seg.contains('\\')
        || seg.contains('\0')
        || seg == "."
        || seg == ".."
        || seg.contains("..")
        || Path::new(seg).components().count() != 1
        || !is_valid_agent_name(seg)
    {
        return Err(AgentsError::InvalidStem(seg.to_string()));
    }
    match Path::new(seg).file_name().and_then(|s| s.to_str()) {
        Some(name) if name == seg => Ok(()),
        _ => Err(AgentsError::InvalidStem(seg.to_string())),
    }
}

/// Alias used by older call sites / session ids of single segments.
pub fn validate_stem(stem: &str) -> Result<(), AgentsError> {
    // Allow full refs category/name or single segment
    if stem.contains('/') {
        parse_agent_ref(stem).map(|_| ())
    } else {
        validate_segment(stem)
    }
}

/// Parse `category/name` agent id.
pub fn parse_agent_ref(agent_ref: &str) -> Result<(String, String), AgentsError> {
    if agent_ref.is_empty() || agent_ref.contains('\\') || agent_ref.contains('\0') {
        return Err(AgentsError::InvalidStem(agent_ref.to_string()));
    }
    let parts: Vec<&str> = agent_ref.split('/').collect();
    if parts.len() != 2 {
        return Err(AgentsError::InvalidStem(agent_ref.to_string()));
    }
    validate_segment(parts[0])?;
    validate_segment(parts[1])?;
    Ok((parts[0].to_string(), parts[1].to_string()))
}

pub fn agent_id(category: &str, name: &str) -> String {
    format!("{category}/{name}")
}

/// Resolve `agents_dir/{category}/{name}.md`.
pub fn resolve_agent_path(
    agents_dir: impl AsRef<Path>,
    agent_ref: &str,
) -> Result<PathBuf, AgentsError> {
    let (category, name) = parse_agent_ref(agent_ref)?;
    let dir = agents_dir.as_ref();
    let path = dir.join(&category).join(format!("{name}.md"));

    let expected_rel = format!("{category}/{name}.md");
    if path.strip_prefix(dir).ok().and_then(|rel| {
        rel.to_str()
            .map(|s| s == expected_rel || s.replace('\\', "/") == expected_rel)
    }) != Some(true)
    {
        return Err(AgentsError::PathEscape(agent_ref.to_string()));
    }

    if dir.is_dir() {
        let dir_canon = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
        if path.is_file() || path.exists() {
            if let Ok(path_canon) = fs::canonicalize(&path) {
                if !path_canon.starts_with(&dir_canon) {
                    return Err(AgentsError::PathEscape(agent_ref.to_string()));
                }
                return Ok(path_canon);
            }
        } else if let Some(parent) = path.parent() {
            if parent.is_dir() {
                if let Ok(parent_canon) = fs::canonicalize(parent) {
                    if !parent_canon.starts_with(&dir_canon) {
                        return Err(AgentsError::PathEscape(agent_ref.to_string()));
                    }
                }
            }
        }
    }

    Ok(path)
}

pub fn path_is_under_agents(agents_dir: impl AsRef<Path>, path: impl AsRef<Path>) -> bool {
    let dir = agents_dir.as_ref();
    let path = path.as_ref();
    let dir_canon = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let path_canon = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    path_canon.starts_with(&dir_canon)
}
