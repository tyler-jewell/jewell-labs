//! Jewell harness: in-process host gates + optional HTTP chat for closed_form.

use super::{AdapterResult, HarnessAdapter};
use crate::eval::compare::task::Task;
use crate::eval::{
    orchestrator_ctx, run_team_collaboration_case, run_tool_plan_case, GroundTruth,
};
use std::fs;
use std::path::Path;
use std::time::Instant;

pub struct JewellAdapter {
    base: String,
    agent: String,
}

impl JewellAdapter {
    pub fn new() -> Self {
        Self {
            base: std::env::var("JEWELL_BASE").unwrap_or_else(|_| "http://127.0.0.1:3000".into()),
            agent: std::env::var("JEWELL_AGENT")
                .unwrap_or_else(|_| "core/orchestrator".into()),
        }
    }
}

impl HarnessAdapter for JewellAdapter {
    fn name(&self) -> &str {
        "jewell"
    }

    fn provides(&self) -> &[&str] {
        // Product chat + host gates + agent sandbox fs_* (write_file surface).
        &[
            "host_eval",
            "agent_os",
            "closed_form",
            "multi_step",
            "write_file",
            "read_file",
            "terminal",
            "coding",
        ]
    }

    fn run(&self, task: &Task, workspace: &Path, instruction: &str) -> AdapterResult {
        let (ok, missing) = self.can_run(task);
        if !ok {
            return AdapterResult::skip(
                format!("jewell lacks capabilities: {missing:?}"),
                missing,
            );
        }

        if task.capabilities.iter().any(|c| c == "host_eval") {
            return self.run_host_eval(task, workspace);
        }

        self.run_chat(task, workspace, instruction)
    }
}

impl JewellAdapter {
    fn run_host_eval(&self, task: &Task, workspace: &Path) -> AdapterResult {
        let mode_path = task.workspace_src().join("host_mode.txt");
        let mode = fs::read_to_string(&mode_path)
            .unwrap_or_else(|_| "tool_plan".into())
            .trim()
            .to_string();

        let t0 = Instant::now();
        let (exit, stdout, detail) = match mode.as_str() {
            "team" => {
                let case = run_team_collaboration_case();
                let code = if case.correct { 0 } else { 1 };
                (
                    code,
                    format!("team_collaboration correct={}", case.correct),
                    "team_collaboration_green".to_string(),
                )
            }
            _ => {
                // In-process tool_plan + team (no cargo subprocess)
                let (ctx, _) = orchestrator_ctx();
                let gt = GroundTruth::collect(&ctx);
                let plan = run_tool_plan_case(&ctx, &gt);
                let team = run_team_collaboration_case();
                let code = if plan.correct && team.correct { 0 } else { 1 };
                (
                    code,
                    format!(
                        "tool_plan={} team={}",
                        plan.correct, team.correct
                    ),
                    "eval_introspection".to_string(),
                )
            }
        };
        let _ = fs::write(workspace.join("host_exit_code.txt"), format!("{exit}\n"));
        AdapterResult {
            status: if exit == 0 { "ok" } else { "error" }.into(),
            stdout,
            stderr: String::new(),
            t_ms: t0.elapsed().as_millis() as u64,
            detail,
            capabilities_missing: vec![],
            exit_code: Some(exit),
        }
    }

    fn run_chat(&self, task: &Task, workspace: &Path, instruction: &str) -> AdapterResult {
        // Fairness: shared instruction only — no coach wrappers.
        let _ = (task, workspace);
        let body = serde_json::json!({
            "agent_stem": self.agent,
            "message": instruction.trim(),
            "history": [],
        });
        let url = format!("{}/api/chat/stream", self.base.trim_end_matches('/'));
        let t0 = Instant::now();
        let client = match reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(task.timeout_s.max(30)))
            .build()
        {
            Ok(c) => c,
            Err(e) => return AdapterResult::error(format!("http client: {e}")),
        };
        let resp = match client
            .post(&url)
            .header("content-type", "application/json")
            .header("accept", "text/event-stream")
            .json(&body)
            .send()
        {
            Ok(r) => r,
            Err(e) => {
                return AdapterResult::error(format!(
                    "cannot reach jewell at {}: {e}",
                    self.base
                ));
            }
        };
        if !resp.status().is_success() {
            let code = resp.status().as_u16();
            let err = resp.text().unwrap_or_default();
            return AdapterResult {
                status: "error".into(),
                stdout: String::new(),
                stderr: err,
                t_ms: t0.elapsed().as_millis() as u64,
                detail: format!("HTTP {code} from {} — is rust-agent running?", self.base),
                capabilities_missing: vec![],
                exit_code: None,
            };
        }
        let raw = resp.text().unwrap_or_default();
        let mut chunks = Vec::new();
        let mut stream_err = String::new();
        for line in raw.lines() {
            let line = line.trim();
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data.is_empty() || data == "[DONE]" {
                continue;
            }
            let Ok(obj) = serde_json::from_str::<serde_json::Value>(data) else {
                chunks.push(data.to_string());
                continue;
            };
            if let Some(e) = obj.get("error") {
                stream_err = e.to_string();
                break;
            }
            for k in ["text", "content", "delta", "message", "token"] {
                if let Some(s) = obj.get(k).and_then(|x| x.as_str()) {
                    chunks.push(s.to_string());
                }
            }
        }
        let text = chunks.join("");
        if text.trim().is_empty() {
            return AdapterResult {
                status: "error".into(),
                stdout: String::new(),
                stderr: stream_err,
                t_ms: t0.elapsed().as_millis() as u64,
                detail: "chat error (empty transcript)".into(),
                capabilities_missing: vec![],
                exit_code: None,
            };
        }
        if task.track == "closed_form" {
            let _ = fs::write(workspace.join("agent_answer.txt"), &text);
        } else {
            let _ = fs::write(workspace.join("agent_transcript.txt"), &text);
        }
        let detail = if stream_err.is_empty() {
            "jewell chat stream completed".into()
        } else {
            format!("partial stream: {}", &stream_err[..stream_err.len().min(120)])
        };
        AdapterResult {
            status: "ok".into(),
            stdout: text,
            stderr: stream_err,
            t_ms: t0.elapsed().as_millis() as u64,
            detail,
            capabilities_missing: vec![],
            exit_code: Some(0),
        }
    }
}
