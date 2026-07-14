//! Harness drivers: dry (gold control), jewell, hermes (+ extension point).

mod dry;
mod hermes;
mod jewell;

use super::task::Task;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct AdapterResult {
    /// ok | skip | error
    pub status: String,
    pub stdout: String,
    pub stderr: String,
    pub t_ms: u64,
    pub detail: String,
    pub capabilities_missing: Vec<String>,
    pub exit_code: Option<i32>,
}

impl AdapterResult {
    pub fn skip(detail: impl Into<String>, missing: Vec<String>) -> Self {
        Self {
            status: "skip".into(),
            stdout: String::new(),
            stderr: String::new(),
            t_ms: 0,
            detail: detail.into(),
            capabilities_missing: missing,
            exit_code: None,
        }
    }

    pub fn error(detail: impl Into<String>) -> Self {
        Self {
            status: "error".into(),
            stdout: String::new(),
            stderr: String::new(),
            t_ms: 0,
            detail: detail.into(),
            capabilities_missing: vec![],
            exit_code: None,
        }
    }
}

pub trait HarnessAdapter: Send + Sync {
    fn name(&self) -> &str;
    fn provides(&self) -> &[&str];

    fn can_run(&self, task: &Task) -> (bool, Vec<String>) {
        let missing: Vec<String> = task
            .capabilities
            .iter()
            .filter(|c| !self.provides().contains(&c.as_str()))
            .cloned()
            .collect();
        (missing.is_empty(), missing)
    }

    fn run(&self, task: &Task, workspace: &Path, instruction: &str) -> AdapterResult;
}

pub fn get_adapter(name: &str) -> Result<Box<dyn HarnessAdapter>, String> {
    match name.trim().to_ascii_lowercase().as_str() {
        "dry" => Ok(Box::new(dry::DryAdapter)),
        "jewell" => Ok(Box::new(jewell::JewellAdapter::new())),
        "hermes" => Ok(Box::new(hermes::HermesAdapter::new())),
        other => Err(format!(
            "unknown harness adapter: {other} (known: dry, jewell, hermes)"
        )),
    }
}

pub fn known_harnesses() -> &'static [&'static str] {
    &["dry", "jewell", "hermes"]
}
