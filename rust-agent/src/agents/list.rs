//! List, load, write, and certify agent files.

use super::path::{
    agent_id, parse_agent_ref, path_is_under_agents, resolve_agent_path, validate_segment,
};
use crate::jail::{agent_file_rel, JailError, WriteJail};
use crate::schema::{
    certify_agent_markdown, parse_frontmatter, AgentDocument, CertificationResult, SchemaError,
};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentsError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("schema: {0}")]
    Schema(#[from] SchemaError),
    #[error("agent not found: {0}")]
    NotFound(String),
    #[error("invalid agent ref '{0}': expected category/name with safe segments")]
    InvalidStem(String),
    #[error("path escape rejected for '{0}'")]
    PathEscape(String),
    #[error("jail: {0}")]
    Jail(#[from] JailError),
    #[error("publish blocked: {0}")]
    PublishBlocked(String),
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentListItem {
    /// Full id: `category/name`
    pub id: String,
    pub category: String,
    /// Filename stem / agent name
    pub stem: String,
    pub name: String,
    pub description: String,
    pub default_model: String,
    pub role: String,
    pub tools: Vec<String>,
    pub path: String,
    pub certification: CertificationResult,
}

/// List all `agents/{category}/*.md`, sorted by id.
pub fn list_agents(agents_dir: impl AsRef<Path>) -> Result<Vec<AgentListItem>, AgentsError> {
    let dir = agents_dir.as_ref();
    let mut items = Vec::new();
    if !dir.is_dir() {
        return Ok(items);
    }

    let mut category_dirs: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .filter(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .map(|s| validate_segment(s).is_ok())
                .unwrap_or(false)
        })
        .collect();
    category_dirs.sort();

    for cat_dir in category_dirs {
        let category = cat_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let mut files: Vec<PathBuf> = fs::read_dir(&cat_dir)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("md"))
                    .unwrap_or(false)
            })
            .filter(|p| {
                p.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| validate_segment(s).is_ok())
                    .unwrap_or(false)
            })
            .collect();
        files.sort();

        for path in files {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            let id = agent_id(&category, &stem);
            let text = fs::read_to_string(&path)?;
            let certification = certify_agent_markdown(&text);
            let (name, description, default_model, role, tools) = match parse_frontmatter(&text) {
                Ok((fm, _)) => (fm.name, fm.description, fm.default_model, fm.role, fm.tools),
                Err(_) => (
                    stem.clone(),
                    String::new(),
                    String::new(),
                    "agent".into(),
                    Vec::new(),
                ),
            };
            items.push(AgentListItem {
                id,
                category: category.clone(),
                stem,
                name,
                description,
                default_model,
                role,
                tools,
                path: path.display().to_string(),
                certification,
            });
        }
    }
    items.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(items)
}

/// Load a single agent by id `category/name`.
pub fn load_agent(
    agents_dir: impl AsRef<Path>,
    agent_ref: &str,
) -> Result<AgentDocument, AgentsError> {
    let path = resolve_agent_path(agents_dir.as_ref(), agent_ref)?;
    if !path.is_file() {
        return Err(AgentsError::NotFound(agent_ref.to_string()));
    }
    if !path_is_under_agents(agents_dir.as_ref(), &path) {
        return Err(AgentsError::PathEscape(agent_ref.to_string()));
    }
    let (category, name) = parse_agent_ref(agent_ref)?;
    let text = fs::read_to_string(&path)?;
    let (frontmatter, body) = parse_frontmatter(&text)?;
    Ok(AgentDocument {
        path: path.display().to_string(),
        id: agent_ref.to_string(),
        category,
        stem: name,
        frontmatter,
        body,
    })
}

pub fn certify_agent_file(
    agents_dir: impl AsRef<Path>,
    agent_ref: &str,
) -> Result<CertificationResult, AgentsError> {
    let path = resolve_agent_path(agents_dir.as_ref(), agent_ref)?;
    if !path.is_file() {
        return Err(AgentsError::NotFound(agent_ref.to_string()));
    }
    if !path_is_under_agents(agents_dir.as_ref(), &path) {
        return Err(AgentsError::PathEscape(agent_ref.to_string()));
    }
    let text = fs::read_to_string(&path)?;
    Ok(certify_agent_markdown(&text))
}

/// Create/replace `agents/{category}/{name}.md` via pre-open path jail.
/// Core agent (`core/orchestrator`) is write-locked by default.
pub fn write_agent_file(
    agents_dir: impl AsRef<Path>,
    agent_ref: &str,
    markdown: &str,
) -> Result<PathBuf, AgentsError> {
    let dir = agents_dir.as_ref();
    parse_agent_ref(agent_ref)?;
    let cert = certify_agent_markdown(markdown);
    if !cert.ok {
        return Err(AgentsError::Schema(SchemaError::CertificationFailed(
            cert.errors.join("; "),
        )));
    }
    let jail = WriteJail::agents_dir(dir);
    jail.assert_not_core_lock(agent_ref)?;
    let rel = agent_file_rel(agent_ref)?;
    Ok(jail.write_file(&rel, markdown.as_bytes())?)
}

/// Write without core lock (host/bootstrap only — not agent tools).
pub fn write_agent_file_unlocked(
    agents_dir: impl AsRef<Path>,
    agent_ref: &str,
    markdown: &str,
) -> Result<PathBuf, AgentsError> {
    let dir = agents_dir.as_ref();
    parse_agent_ref(agent_ref)?;
    let cert = certify_agent_markdown(markdown);
    if !cert.ok {
        return Err(AgentsError::Schema(SchemaError::CertificationFailed(
            cert.errors.join("; "),
        )));
    }
    let mut jail = WriteJail::agents_dir(dir);
    jail.locked_agent_ids.clear();
    let rel = agent_file_rel(agent_ref)?;
    Ok(jail.write_file(&rel, markdown.as_bytes())?)
}
