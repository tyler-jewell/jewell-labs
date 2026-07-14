//! Catalog source + item metadata contract.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceMeta {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Primary project / paper / leaderboard URLs.
    #[serde(default)]
    pub links: Vec<String>,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub version: String,
    /// Capability tags: coding, tool_call, autonomy, terminal, assistant, multi_step, ...
    #[serde(default)]
    pub tags: Vec<String>,
    /// When false, excluded from default discovery.
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub notes: String,
    /// Filled at load time.
    #[serde(default)]
    pub item_count: usize,
    #[serde(skip)]
    pub path: PathBuf,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradeSpec {
    /// exact | boxed | contains | tool_call | no_tool_call | sandbox_skip | dry_marker |
    /// file_exact | file_lines_exact | solution_file (alias: python_workspace)
    pub kind: String,
    #[serde(default)]
    pub expected: Option<String>,
    #[serde(default)]
    pub expected_contains: Option<String>,
    #[serde(default)]
    pub tool_name: Option<String>,
    /// Grade this workspace-relative file instead of agent_answer.txt.
    #[serde(default)]
    pub answer_file: Option<String>,
    /// If true, dry harness auto-passes (catalog wiring smoke).
    #[serde(default)]
    pub dry_pass: bool,
    /// Honest skip when full sandbox unavailable.
    #[serde(default)]
    pub skip_reason: Option<String>,
    /// Harnesses that cannot run this item even if capabilities match.
    #[serde(default)]
    pub requires_sandbox: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogItem {
    pub id: String,
    #[serde(default)]
    pub source_id: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_track")]
    pub track: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub prompt: String,
    pub grade: GradeSpec,
    /// Tool/function schemas provided to the harness at runtime (BFCL-style).
    /// Empty means environment-native tools only (terminal/SWE).
    #[serde(default)]
    pub tools: Vec<Value>,
    #[serde(default)]
    pub workspace_files: BTreeMap<String, String>,
    #[serde(default)]
    pub links: Vec<String>,
    #[serde(default)]
    pub meta: Value,
}

fn default_track() -> String {
    "coding".into()
}

impl CatalogItem {
    pub fn full_id(&self) -> String {
        if self.source_id.is_empty() {
            self.id.clone()
        } else {
            format!("{}/{}", self.source_id, self.id)
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CatalogFilter {
    /// Empty = all enabled sources.
    pub sources: Vec<String>,
    /// Item must have ALL of these tags (AND). Empty = no tag filter.
    pub tags_all: Vec<String>,
    /// Item must have ANY of these tags (OR). Empty = no tag filter.
    pub tags_any: Vec<String>,
    /// Filter by track/kind.
    pub tracks: Vec<String>,
    /// When true, drop items that need Docker/Harbor sandboxes.
    pub exclude_sandbox: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSummary {
    pub id: String,
    pub name: String,
    pub links: Vec<String>,
    pub tags: Vec<String>,
    pub version: String,
    pub license: String,
    pub enabled: bool,
    pub item_count: usize,
    pub notes: String,
}
