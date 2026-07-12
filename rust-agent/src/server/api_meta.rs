//! Schema, nav, and tools listing endpoints.

use super::state::AppState;
use crate::{
    agents_dir, builtin_tools, invoke_tool, list_tools_from_fs, load_agent, schema_summary,
    tools_dir, LATEST_SCHEMA_VERSION,
};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

pub async fn schema() -> impl IntoResponse {
    Json(json!({
        "latest_schema_version": LATEST_SCHEMA_VERSION,
        "fields": schema_summary(),
    }))
}

pub async fn list_tools() -> impl IntoResponse {
    Json(json!({
        "tools": builtin_tools(),
        "from_disk": list_tools_from_fs(tools_dir()),
        "tools_dir": tools_dir().display().to_string(),
    }))
}

pub async fn nav() -> impl IntoResponse {
    Json(json!({
        "agent_tabs": crate::AGENT_TABS,
        "agents_dir": agents_dir().display().to_string(),
        "tools_dir": tools_dir().display().to_string(),
        "alignment": {
            "sidebar_agents": "agents/{category}/{name}.md",
            "sidebar_tools": "tools/{category}/{name}.rs",
            "agent_tabs": "src/{chat,sessions,schema,registry}.rs",
        }
    }))
}

#[derive(Debug, Deserialize)]
pub struct InvokeToolBody {
    pub name: String,
    #[serde(default)]
    pub arguments: Value,
    #[serde(default)]
    pub caller_agent: Option<String>,
    #[serde(default)]
    pub allowed_tools: Option<Vec<String>>,
}

fn resolve_caller_allowlist(
    state: &AppState,
    caller: &Option<String>,
    override_list: Option<Vec<String>>,
) -> Vec<String> {
    if let Some(list) = override_list {
        return list;
    }
    if let Some(id) = caller {
        if let Ok(doc) = load_agent(&state.agents_dir, id) {
            return doc.frontmatter.tools;
        }
    }
    vec!["*".into()]
}

pub async fn invoke_tool_api(
    State(state): State<Arc<AppState>>,
    Json(body): Json<InvokeToolBody>,
) -> impl IntoResponse {
    let allowed = resolve_caller_allowlist(&state, &body.caller_agent, body.allowed_tools);
    let ctx = state.tool_ctx(body.caller_agent.clone(), allowed);
    let result = invoke_tool(&ctx, &body.name, &body.arguments);
    let status = if result.ok {
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    };
    (status, Json(result)).into_response()
}
