//! Native Jewell vendor: product HTTP chat + in-process host gates.

use super::model::EvalModel;
use super::VendorContext;
use crate::eval::compare::AdapterResult;
use crate::eval::{orchestrator_ctx, run_team_collaboration_case, run_tool_plan_case, GroundTruth};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeJewellSpec {
    #[serde(default = "default_agent")]
    pub agent: String,
    /// Product console base (rust-agent), not the LLM endpoint.
    #[serde(default = "default_product_base")]
    pub base_url: String,
    /// Optional eval sampling pin (declared in vendor.toml SSoT only).
    #[serde(default)]
    pub eval_temperature: Option<f64>,
}

fn default_agent() -> String {
    "core/orchestrator".into()
}

fn default_product_base() -> String {
    "http://127.0.0.1:3000".into()
}

#[derive(Debug, Clone)]
pub struct JewellVendor {
    pub id: String,
    pub capabilities: Vec<String>,
    pub product_base: String,
    pub agent: String,
    pub eval_temperature: Option<f64>,
    pub resolved_endpoint: String,
    pub resolved_model: String,
}

impl JewellVendor {
    pub fn from_manifest(id: &str, capabilities: Vec<String>, spec: NativeJewellSpec) -> Self {
        let product_base = std::env::var("JEWELL_BASE").unwrap_or(spec.base_url);
        let agent = std::env::var("JEWELL_AGENT").unwrap_or(spec.agent);
        Self {
            id: id.into(),
            capabilities,
            product_base,
            agent,
            eval_temperature: spec.eval_temperature,
            resolved_endpoint: String::new(),
            resolved_model: String::new(),
        }
    }

    pub fn pin_model(&mut self, model: &EvalModel) {
        // LLM pin is applied per-request via eval_* fields on chat API.
        self.resolved_endpoint = model.openai_base_url();
        self.resolved_model = model.model.clone();
    }

    pub fn run(&self, ctx: &VendorContext<'_>) -> AdapterResult {
        // Capability gate (honest: no terminal/write_file unless declared).
        let missing: Vec<String> = ctx
            .required_capabilities
            .iter()
            .filter(|c| !self.capabilities.iter().any(|p| p == *c))
            .cloned()
            .collect();
        if !missing.is_empty() {
            return AdapterResult::skip(format!("jewell lacks capabilities: {missing:?}"), missing);
        }

        if ctx.required_capabilities.iter().any(|c| c == "host_eval") {
            return self.run_host_eval(ctx.workspace);
        }

        // Ensure catalog workspaces stay under evals/runs (eval pin policy).
        if let Err(e) = crate::eval_guards::assert_sandbox_override_allowed(
            ctx.workspace,
            &crate::paths::repo_root(),
        ) {
            return AdapterResult::error(format!("eval workspace not allowed: {e}"));
        }

        self.run_chat(ctx)
    }

    fn run_host_eval(&self, workspace: &Path) -> AdapterResult {
        let t0 = Instant::now();
        let (ctx, _) = orchestrator_ctx();
        let gt = GroundTruth::collect(&ctx);
        let plan = run_tool_plan_case(&ctx, &gt);
        let team = run_team_collaboration_case();
        let code = if plan.correct && team.correct { 0 } else { 1 };
        let stdout = format!("tool_plan={} team={}", plan.correct, team.correct);
        let _ = fs::write(workspace.join("host_exit_code.txt"), format!("{code}\n"));
        AdapterResult {
            status: if code == 0 { "ok" } else { "error" }.into(),
            stdout,
            stderr: String::new(),
            t_ms: t0.elapsed().as_millis() as u64,
            detail: "eval_introspection".into(),
            capabilities_missing: vec![],
            exit_code: Some(code),
        }
    }

    fn run_chat(&self, ctx: &VendorContext<'_>) -> AdapterResult {
        // Fairness: send the shared catalog instruction only.
        // Wiring (model pin, fs sandbox, temperature) comes from vendor.toml / eval pins —
        // never from coach text in this adapter.
        let message = ctx.instruction.trim().to_string();
        let mut body = serde_json::json!({
            "agent_stem": self.agent,
            "message": message,
            "history": [],
            "eval_base_url": ctx.model.chat_host_base(),
            "eval_model": ctx.model.model,
            "eval_fs_root": ctx.workspace.display().to_string(),
        });
        // Optional sampling pin from vendor config (SSoT), not hard-coded coaching.
        if let Some(t) = self.eval_temperature {
            body["eval_temperature"] = serde_json::json!(t);
        }
        let url = format!(
            "{}/api/chat/stream",
            self.product_base.trim_end_matches('/')
        );
        let t0 = Instant::now();
        let client = match reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(ctx.timeout_s.max(30)))
            .build()
        {
            Ok(c) => c,
            Err(e) => return AdapterResult::error(format!("http client: {e}")),
        };
        // Server requires JEWELL_ALLOW_EVAL_PINS=1 for eval_fs_root / model pin fields.
        // Catalog runner is trusted local; public chat without the env ignores pins.
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
                    self.product_base
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
                detail: format!(
                    "HTTP {code} from {} — is rust-agent running?",
                    self.product_base
                ),
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
        let t_ms = t0.elapsed().as_millis() as u64;
        if text.trim().is_empty() {
            return AdapterResult {
                status: "error".into(),
                stdout: String::new(),
                stderr: stream_err,
                t_ms,
                detail: "chat error (empty transcript)".into(),
                capabilities_missing: vec![],
                exit_code: None,
            };
        }
        let _ = fs::write(ctx.workspace.join("agent_answer.txt"), &text);
        AdapterResult {
            status: "ok".into(),
            stdout: text,
            stderr: stream_err,
            t_ms,
            detail: format!(
                "jewell chat (llm={} @ {})",
                self.resolved_model, self.resolved_endpoint
            ),
            capabilities_missing: vec![],
            exit_code: Some(0),
        }
    }
}
