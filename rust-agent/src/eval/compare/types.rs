//! Multi-harness compare report types (jewell / hermes / dry / future).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ItemStatus {
    Pass,
    Fail,
    Skip,
    Error,
}

impl ItemStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Skip => "skip",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareItem {
    pub harness: String,
    pub task_id: String,
    pub track: String,
    pub run_index: u32,
    pub status: String,
    pub correct: bool,
    pub score: f64,
    pub t_ms: u64,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub metrics: BTreeMap<String, Value>,
    #[serde(default)]
    pub workspace: String,
    #[serde(default)]
    pub capabilities_missing: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessSummary {
    pub n_items: usize,
    pub n_scored: usize,
    pub n_skip: usize,
    pub n_error: usize,
    pub avg_score: f64,
    pub solid_base: f64,
    pub pass_rate: f64,
    #[serde(default)]
    pub per_task_avg: BTreeMap<String, f64>,
    pub mean_t_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareSummary {
    pub harnesses: BTreeMap<String, HarnessSummary>,
    /// Scored items only (pass+fail), for UI badge compatibility with EvalReport.
    pub total: usize,
    pub correct: usize,
    pub accuracy: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareReport {
    pub id: String,
    pub created: String,
    /// Discriminator for UI / store (`compare`).
    pub kind: String,
    pub agent: String,
    pub tracks: Vec<String>,
    pub meta: Value,
    pub summary: CompareSummary,
    pub items: Vec<CompareItem>,
    /// Per-task matrix: task_id → harness → avg score (scored runs only).
    #[serde(default)]
    pub task_matrix: BTreeMap<String, BTreeMap<String, f64>>,
}

impl CompareReport {
    pub const KIND: &'static str = "compare";
}
