use super::{AdapterResult, HarnessAdapter};
use crate::eval::compare::task::{copy_dir_all, Task};
use std::fs;
use std::path::Path;
use std::time::Instant;

pub struct DryAdapter;

impl HarnessAdapter for DryAdapter {
    fn name(&self) -> &str {
        "dry"
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
            "host_eval",
        ]
    }

    fn run(&self, task: &Task, workspace: &Path, _instruction: &str) -> AdapterResult {
        let t0 = Instant::now();
        let gold = task.workspace_gold();
        let detail = if gold.is_dir() {
            // replace workspace with gold
            if let Ok(rd) = fs::read_dir(workspace) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        let _ = fs::remove_dir_all(&p);
                    } else {
                        let _ = fs::remove_file(&p);
                    }
                }
            }
            if let Ok(rd) = fs::read_dir(&gold) {
                for e in rd.flatten() {
                    let src = e.path();
                    let dest = workspace.join(e.file_name());
                    if src.is_dir() {
                        let _ = copy_dir_all(&src, &dest);
                    } else {
                        let _ = fs::copy(&src, &dest);
                    }
                }
            }
            "applied workspace_gold".to_string()
        } else {
            let expected = task.path.join("tests").join("expected.txt");
            if expected.is_file() {
                if let Ok(text) = fs::read_to_string(&expected) {
                    let _ = fs::write(workspace.join("agent_answer.txt"), text);
                }
                "wrote expected answer for closed_form".into()
            } else {
                "no gold; left seed workspace".into()
            }
        };
        let detail = if task.capabilities.iter().any(|c| c == "host_eval") {
            let _ = fs::write(workspace.join("host_eval_ok.txt"), "ok\n");
            "host_eval dry marker".to_string()
        } else {
            detail
        };
        AdapterResult {
            status: "ok".into(),
            stdout: detail.clone(),
            stderr: String::new(),
            t_ms: t0.elapsed().as_millis() as u64,
            detail,
            capabilities_missing: vec![],
            exit_code: Some(0),
        }
    }
}
