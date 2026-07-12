//! Agent REST endpoints.

use super::state::AppState;
use crate::{agent_id, certify_agent_markdown, list_agents, load_agent, write_agent_file};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

pub async fn list_agents_api(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match list_agents(&state.agents_dir) {
        Ok(items) => Json(json!({ "agents": items })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn get_agent(
    State(state): State<Arc<AppState>>,
    Path((category, name)): Path<(String, String)>,
) -> impl IntoResponse {
    let id = agent_id(&category, &name);
    match load_agent(&state.agents_dir, &id) {
        Ok(doc) => {
            let text = std::fs::read_to_string(&doc.path).unwrap_or_default();
            let cert = certify_agent_markdown(&text);
            Json(json!({ "agent": doc, "certification": cert })).into_response()
        }
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn certify(
    State(state): State<Arc<AppState>>,
    Path((category, name)): Path<(String, String)>,
) -> impl IntoResponse {
    let id = agent_id(&category, &name);
    match load_agent(&state.agents_dir, &id) {
        Ok(doc) => {
            let text = std::fs::read_to_string(&doc.path).unwrap_or_default();
            Json(certify_agent_markdown(&text)).into_response()
        }
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateAgentBody {
    pub id: String,
    pub markdown: String,
}

pub async fn create_agent(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateAgentBody>,
) -> impl IntoResponse {
    match write_agent_file(&state.agents_dir, &body.id, &body.markdown) {
        Ok(path) => (
            StatusCode::CREATED,
            Json(json!({
                "path": path.display().to_string(),
                "id": body.id,
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
