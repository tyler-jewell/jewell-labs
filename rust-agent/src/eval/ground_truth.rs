//! Ground truth collection and eval report types.

use crate::agents::CORE_AGENT_ID;
use crate::tools::{invoke_tool, ToolContext};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactResult {
    pub id: String,
    pub description: String,
    pub correct: bool,
    pub expected: String,
    pub got: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseResult {
    pub id: String,
    pub prompt: String,
    pub track: String,
    pub gold: Value,
    pub response: String,
    pub tools_called: Vec<String>,
    pub tool_results_ok: bool,
    pub facts: Vec<FactResult>,
    pub correct: bool,
    pub grader: String,
    pub t_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalReport {
    pub id: String,
    pub created: String,
    pub agent: String,
    pub tracks: Vec<String>,
    pub model: Option<String>,
    pub server: Option<String>,
    pub ground_truth: Value,
    pub cases: Vec<CaseResult>,
    pub summary: EvalSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalSummary {
    pub total: usize,
    pub correct: usize,
    pub accuracy: f64,
    pub tool_plan_accuracy: f64,
    pub llm_agent_accuracy: Option<f64>,
    pub all_tools_invoked_in_plan: bool,
    pub complete_introspection: bool,
}

#[derive(Debug, Clone)]
pub struct GroundTruth {
    pub agent_ids: BTreeSet<String>,
    pub tool_names: BTreeSet<String>,
    pub tool_categories: BTreeSet<String>,
    pub agent_count: usize,
    pub tool_count: usize,
    pub schema_version: u32,
    pub model_keys: BTreeSet<String>,
    pub orchestrator_role: String,
    pub orchestrator_model: String,
    pub orchestrator_tools: Vec<String>,
    pub core_agent_cert_ok: bool,
    pub agents_dir: String,
    pub registry_present: bool,
}

impl GroundTruth {
    pub fn collect(ctx: &ToolContext) -> Self {
        let list_agents = invoke_tool(ctx, "list_agents", &json!({}));
        let list_tools = invoke_tool(ctx, "list_tools", &json!({}));
        let schema = invoke_tool(ctx, "get_schema", &json!({}));
        let models = invoke_tool(ctx, "list_models", &json!({}));
        let status = invoke_tool(ctx, "app_status", &json!({}));
        let orch = invoke_tool(ctx, "get_agent", &json!({"id": CORE_AGENT_ID}));
        let core_cert = invoke_tool(ctx, "certify_agent", &json!({"id": CORE_AGENT_ID}));

        let agent_ids: BTreeSet<String> = list_agents
            .result
            .get("agents")
            .and_then(|a| a.as_array())
            .into_iter()
            .flatten()
            .filter_map(|a| a.get("id").and_then(|x| x.as_str()).map(|s| s.to_string()))
            .collect();

        let tool_names: BTreeSet<String> = list_tools
            .result
            .get("tools")
            .and_then(|a| a.as_array())
            .into_iter()
            .flatten()
            .filter_map(|t| {
                t.get("name")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
            })
            .collect();

        let tool_categories: BTreeSet<String> = list_tools
            .result
            .get("tools")
            .and_then(|a| a.as_array())
            .into_iter()
            .flatten()
            .filter_map(|t| {
                t.get("category")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
            })
            .collect();

        let model_keys: BTreeSet<String> = models
            .result
            .get("models")
            .and_then(|a| a.as_array())
            .into_iter()
            .flatten()
            .filter_map(|m| m.get("key").and_then(|x| x.as_str()).map(|s| s.to_string()))
            .collect();

        let orchestrator_tools = orch
            .result
            .pointer("/frontmatter/tools")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        Self {
            agent_count: agent_ids.len(),
            tool_count: tool_names.len(),
            agent_ids,
            tool_names,
            tool_categories,
            schema_version: schema
                .result
                .get("latest_schema_version")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            model_keys,
            orchestrator_role: orch
                .result
                .pointer("/frontmatter/role")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            orchestrator_model: orch
                .result
                .pointer("/frontmatter/default_model")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            orchestrator_tools,
            core_agent_cert_ok: core_cert
                .result
                .get("ok")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            agents_dir: status
                .result
                .get("agents_dir")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            registry_present: status
                .result
                .get("registry_present")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "core_agent_id": CORE_AGENT_ID,
            "agent_ids": self.agent_ids.iter().cloned().collect::<Vec<_>>(),
            "agent_count": self.agent_count,
            "tool_names": self.tool_names.iter().cloned().collect::<Vec<_>>(),
            "tool_count": self.tool_count,
            "tool_categories": self.tool_categories.iter().cloned().collect::<Vec<_>>(),
            "schema_version": self.schema_version,
            "model_keys": self.model_keys.iter().cloned().collect::<Vec<_>>(),
            "orchestrator_role": self.orchestrator_role,
            "orchestrator_model": self.orchestrator_model,
            "orchestrator_tools": self.orchestrator_tools,
            "core_agent_cert_ok": self.core_agent_cert_ok,
            "agents_dir": self.agents_dir,
            "registry_present": self.registry_present,
        })
    }
}
