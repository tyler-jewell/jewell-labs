//! Hermes Agent CLI driver (vendored or HERMES_BIN).

use super::{AdapterResult, HarnessAdapter};
use crate::eval::compare::task::Task;
use crate::paths::repo_root;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

pub struct HermesAdapter {
    bin: PathBuf,
    mode: String,
    home: Option<PathBuf>,
}

impl HermesAdapter {
    pub fn new() -> Self {
        let (bin, home) = resolve_hermes();
        let mode = std::env::var("HERMES_MODE").unwrap_or_else(|_| "chat".into());
        Self { bin, mode, home }
    }
}

fn resolve_hermes() -> (PathBuf, Option<PathBuf>) {
    if let Ok(b) = std::env::var("HERMES_BIN") {
        let home = std::env::var("HERMES_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                let v = repo_root()
                    .join("evals")
                    .join("harness_compare")
                    .join("vendor")
                    .join("hermes-home");
                v.is_dir().then_some(v)
            });
        return (PathBuf::from(b), home);
    }
    let vendor_bin = repo_root()
        .join("evals")
        .join("harness_compare")
        .join("vendor")
        .join("bin")
        .join("hermes");
    let vendor_home = repo_root()
        .join("evals")
        .join("harness_compare")
        .join("vendor")
        .join("hermes-home");
    if vendor_bin.exists() {
        return (vendor_bin, vendor_home.is_dir().then_some(vendor_home));
    }
    (
        PathBuf::from("hermes"),
        vendor_home.is_dir().then_some(vendor_home),
    )
}

fn hermes_on_path(bin: &Path) -> bool {
    if bin.is_file() || bin.is_symlink() {
        return true;
    }
    which(bin.to_str().unwrap_or("hermes")).is_some()
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

impl HarnessAdapter for HermesAdapter {
    fn name(&self) -> &str {
        "hermes"
    }

    fn provides(&self) -> &[&str] {
        &[
            "terminal",
            "write_file",
            "read_file",
            "coding",
            "closed_form",
            "multi_step",
            "agent_os",
        ]
    }

    fn run(&self, _task: &Task, workspace: &Path, instruction: &str) -> AdapterResult {
        if !hermes_on_path(&self.bin) {
            return AdapterResult::skip(
                format!(
                    "`{}` not found — run evals/harness_compare/install_and_compare.sh",
                    self.bin.display()
                ),
                vec!["hermes_cli".into()],
            );
        }

        // Fairness: shared instruction only — workspace isolation is cwd, not coach text.
        let prompt = instruction.trim().to_string();

        let mut cmd = Command::new(&self.bin);
        if self.mode == "z" {
            cmd.arg("-z").arg(&prompt);
        } else {
            cmd.arg("chat").arg("-q").arg(&prompt);
            if let Ok(m) = std::env::var("HERMES_MODEL") {
                cmd.arg("-m").arg(m);
            }
        }
        cmd.current_dir(workspace);
        if let Some(home) = &self.home {
            cmd.env("HERMES_HOME", home);
        }
        if let Ok(h) = std::env::var("HERMES_HOME") {
            cmd.env("HERMES_HOME", h);
        }

        let t0 = Instant::now();
        let out = cmd.output();
        let t_ms = t0.elapsed().as_millis() as u64;
        match out {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                if !o.status.success() {
                    return AdapterResult {
                        status: "error".into(),
                        stdout,
                        stderr,
                        t_ms,
                        detail: format!("hermes exit {:?}", o.status.code()),
                        capabilities_missing: vec![],
                        exit_code: o.status.code(),
                    };
                }
                AdapterResult {
                    status: "ok".into(),
                    stdout,
                    stderr,
                    t_ms,
                    detail: "hermes completed".into(),
                    capabilities_missing: vec![],
                    exit_code: Some(0),
                }
            }
            Err(e) => AdapterResult::error(format!("hermes spawn: {e}")),
        }
    }
}
