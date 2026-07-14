//! Multi-harness compare: jewell / hermes (+ optional dry gold control).
//!
//! Entry points: [`run_compare`], binary `eval_compare`, API suite `compare`.

mod adapters;
mod aggregate;
mod grade;
mod runner;
mod task;
mod types;

pub use adapters::{get_adapter, known_harnesses, AdapterResult, HarnessAdapter};
pub use aggregate::{solid_base, summarize_items, task_matrix};
pub use grade::{grade_task, Grade};
pub use runner::{run_compare, run_dry_all, CompareOpts};
pub use task::{compare_tasks_dir, discover_tasks, load_task, parse_simple_toml, Task};
pub use types::{CompareItem, CompareReport, CompareSummary, HarnessSummary, ItemStatus};

// Re-export Task for catalog runner (same crate).
