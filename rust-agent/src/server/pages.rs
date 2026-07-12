//! HTML page handlers (SSR shell).

use super::state::AppState;
use crate::app::{AgentRowView, AppShell, CategoryAgents, CategoryTools, ShellFocus};
use crate::{
    agent_id, builtin_tools, certify_agent_markdown, filter_tools_for_agent, group_by_category,
    list_agents, list_tools_from_fs, load_agent, normalize_agent_tab, tools_dir, ModelRegistry,
};
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Response};
use leptos::prelude::*;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct TabQuery {
    #[serde(default = "default_tab")]
    pub tab: String,
}

fn default_tab() -> String {
    crate::default_agent_tab().into()
}

pub async fn home(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    render_shell(&state, ShellFocus::Empty)
}

pub async fn agent_page(
    State(state): State<Arc<AppState>>,
    Path((category, name)): Path<(String, String)>,
    Query(q): Query<TabQuery>,
) -> impl IntoResponse {
    let id = agent_id(&category, &name);
    let tab = normalize_agent_tab(&q.tab).to_string();
    render_shell(&state, agent_focus(&state, &id, &tab))
}

pub async fn tool_page(
    State(state): State<Arc<AppState>>,
    Path((category, name)): Path<(String, String)>,
) -> impl IntoResponse {
    let mut tools = list_tools_from_fs(tools_dir());
    for t in &mut tools {
        t.selected = t.category == category && t.name == name;
    }
    let detail = tools
        .iter()
        .find(|t| t.selected)
        .cloned()
        .map(|t| {
            let reg = builtin_tools().into_iter().find(|s| s.name == t.name);
            json!({
                "id": t.id,
                "category": t.category,
                "name": t.name,
                "path": t.path,
                "registered": t.registered,
                "description": t.description,
                "spec": reg,
            })
        })
        .unwrap_or_else(|| {
            json!({ "error": format!("tool file not found: tools/{category}/{name}.rs") })
        });
    let detail_json = serde_json::to_string_pretty(&detail).unwrap_or_else(|_| "{}".into());
    render_shell(
        &state,
        ShellFocus::Tool {
            category,
            name,
            detail_json,
        },
    )
}

fn agent_focus(state: &AppState, id: &str, tab: &str) -> ShellFocus {
    match load_agent(&state.agents_dir, id) {
        Ok(doc) => {
            let text = std::fs::read_to_string(&doc.path).unwrap_or_default();
            let cert = certify_agent_markdown(&text);
            let settings_json =
                serde_json::to_string_pretty(&doc.frontmatter).unwrap_or_else(|_| "{}".into());
            let cert_json = serde_json::to_string_pretty(&cert).unwrap_or_else(|_| "{}".into());
            let allowed = filter_tools_for_agent(&doc.frontmatter.tools);
            let allowed_tools_json =
                serde_json::to_string_pretty(&allowed).unwrap_or_else(|_| "[]".into());
            let registry_json = match ModelRegistry::load(&state.registry_path) {
                Ok(reg) => match reg.resolve(&doc.frontmatter.default_model) {
                    Ok(m) => serde_json::to_string_pretty(&json!({
                        "default_model": doc.frontmatter.default_model,
                        "resolved": {
                            "key": m.key,
                            "alias": m.alias,
                            "path": m.path.display().to_string(),
                            "defaults": { "ctx": m.defaults.ctx, "reasoning": m.defaults.reasoning }
                        },
                        "registry_path": state.registry_path.display().to_string(),
                        "src_module": "src/registry.rs",
                    }))
                    .unwrap_or_else(|_| "{}".into()),
                    Err(e) => serde_json::to_string_pretty(&json!({
                        "default_model": doc.frontmatter.default_model,
                        "error": e.to_string(),
                        "registry_path": state.registry_path.display().to_string(),
                    }))
                    .unwrap_or_else(|_| "{}".into()),
                },
                Err(e) => format!(r#"{{"error":"registry load: {e}"}}"#),
            };
            ShellFocus::Agent {
                id: id.to_string(),
                tab: tab.to_string(),
                settings_json,
                cert_json,
                system_body: doc.body,
                registry_json,
                allowed_tools_json,
            }
        }
        Err(e) => ShellFocus::Agent {
            id: id.to_string(),
            tab: tab.to_string(),
            settings_json: format!(r#"{{"error":"{e}"}}"#),
            cert_json: format!(r#"{{"error":"{e}"}}"#),
            system_body: String::new(),
            registry_json: format!(r#"{{"error":"{e}"}}"#),
            allowed_tools_json: "[]".into(),
        },
    }
}

fn sidebar_data(
    state: &AppState,
    selected_agent: Option<&str>,
    selected_tool: Option<(&str, &str)>,
) -> (Vec<CategoryAgents>, Vec<CategoryTools>) {
    let items = list_agents(&state.agents_dir).unwrap_or_default();
    let agents: Vec<AgentRowView> = items
        .into_iter()
        .map(|a| AgentRowView {
            id: a.id.clone(),
            category: a.category.clone(),
            stem: a.stem.clone(),
            name: a.name.clone(),
            description: a.description.clone(),
            role: a.role.clone(),
            default_model: a.default_model.clone(),
            tools: a.tools.clone(),
            cert_ok: a.certification.ok,
            selected: selected_agent.map(|s| s == a.id.as_str()).unwrap_or(false),
        })
        .collect();
    let agent_groups: Vec<CategoryAgents> = group_by_category(agents, |a| a.category.clone())
        .into_iter()
        .map(|(category, agents)| CategoryAgents { category, agents })
        .collect();
    let mut tools = list_tools_from_fs(tools_dir());
    if let Some((cat, name)) = selected_tool {
        for t in &mut tools {
            t.selected = t.category == cat && t.name == name;
        }
    }
    let tool_groups: Vec<CategoryTools> = group_by_category(tools, |t| t.category.clone())
        .into_iter()
        .map(|(category, tools)| CategoryTools { category, tools })
        .collect();
    (agent_groups, tool_groups)
}

fn render_shell(state: &AppState, focus: ShellFocus) -> Response {
    let (selected_agent, selected_tool) = match &focus {
        ShellFocus::Agent { id, .. } => (Some(id.as_str()), None),
        ShellFocus::Tool { category, name, .. } => (None, Some((category.as_str(), name.as_str()))),
        ShellFocus::Empty => (None, None),
    };
    let (agent_groups, tool_groups) = sidebar_data(state, selected_agent, selected_tool);
    let html = view! {
        <AppShell agent_groups=agent_groups tool_groups=tool_groups focus=focus />
    }
    .to_html();
    Html(html).into_response()
}

