//! Schema types for agent frontmatter and documents.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Latest schema version supported by this application.
pub const LATEST_SCHEMA_VERSION: u32 = 1;

/// Required + optional frontmatter for agents under `agents/{category}/{name}.md`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentFrontmatter {
    /// Schema version for this document. Defaults to 1 if omitted (legacy files).
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,

    /// Unique agent name (usually matches filename stem).
    pub name: String,

    /// Short description shown in UI.
    #[serde(default)]
    pub description: String,

    /// Default llama.cpp model key from `models/registry.yaml`.
    pub default_model: String,

    /// Optional role: orchestrator | agent (default agent).
    #[serde(default = "default_role")]
    pub role: String,

    /// Allowed tools: tool names, `category/*`, or `*` for all.
    /// Empty means the agent may not call tools.
    #[serde(default)]
    pub tools: Vec<String>,

    /// Optional nested server overrides.
    #[serde(default)]
    pub server: Option<ServerConfig>,

    /// Optional sampling defaults.
    #[serde(default)]
    pub sampling: Option<SamplingConfig>,
}

fn default_schema_version() -> u32 {
    LATEST_SCHEMA_VERSION
}

fn default_role() -> String {
    "agent".into()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ServerConfig {
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub ctx: Option<u32>,
    #[serde(default)]
    pub reasoning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SamplingConfig {
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub seed: Option<i64>,
}

/// Fully loaded agent document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentDocument {
    pub path: String,
    /// Full id `category/name`.
    pub id: String,
    pub category: String,
    /// Filename stem (agent name segment).
    pub stem: String,
    pub frontmatter: AgentFrontmatter,
    /// Markdown body (system prompt).
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CertificationResult {
    pub ok: bool,
    pub schema_version: u32,
    pub latest_schema_version: u32,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Error)]
pub enum SchemaError {
    #[error("missing YAML frontmatter (expected leading --- block)")]
    MissingFrontmatter,
    #[error("invalid frontmatter YAML: {0}")]
    InvalidYaml(String),
    #[error("certification failed: {0}")]
    CertificationFailed(String),
}
