//! HTML page handlers (SSR shell). Default = core orchestrator.

use super::state::AppState;
use crate::app::{AgentRowView, AppShell, CategoryAgents, ShellFocus};
use crate::{
    agent_id, certify_agent_markdown, filter_tools_for_agent, group_by_category, list_agents,
    load_agent, normalize_agent_tab, ModelRegistry, CORE_AGENT_ID,
};
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
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

pub async fn home() -> impl IntoResponse {
    Redirect::temporary(&format!("/agents/{CORE_AGENT_ID}?tab=chat"))
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
                        }
                    }))
                    .unwrap_or_else(|_| "{}".into()),
                    Err(e) => format!(r#"{{"error":"{e}"}}"#),
                },
                Err(e) => format!(r#"{{"error":"{e}"}}"#),
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

fn sidebar_agents(state: &AppState, selected: Option<&str>) -> Vec<CategoryAgents> {
    let agents: Vec<AgentRowView> = list_agents(&state.agents_dir)
        .unwrap_or_default()
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
            selected: selected.map(|s| s == a.id.as_str()).unwrap_or(false),
        })
        .collect();
    group_by_category(agents, |a| a.category.clone())
        .into_iter()
        .map(|(category, agents)| CategoryAgents { category, agents })
        .collect()
}

fn render_shell(state: &AppState, focus: ShellFocus) -> Response {
    let selected = match &focus {
        ShellFocus::Agent { id, .. } => Some(id.as_str()),
        ShellFocus::Empty => None,
    };
    let agent_groups = sidebar_agents(state, selected);
    let html = view! { <AppShell agent_groups=agent_groups focus=focus /> }.to_html();
    Html(html).into_response()
}
