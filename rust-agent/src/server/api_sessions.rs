//! Session REST endpoints.

use super::state::AppState;
use crate::{agent_id, ChatSession, SessionMessage};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct SessionsQuery {
    #[serde(default)]
    pub agent: Option<String>,
}

pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SessionsQuery>,
) -> impl IntoResponse {
    match state.session_store().list(q.agent.as_deref()) {
        Ok(sessions) => Json(json!({ "sessions": sessions })).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct UpsertSessionBody {
    pub agent_stem: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub messages: Vec<SessionMessage>,
}

pub async fn upsert_session(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpsertSessionBody>,
) -> impl IntoResponse {
    let store = state.session_store();
    let now = chrono::Utc::now();
    let id = body
        .session_id
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let existing = store.get(&body.agent_stem, &id).ok();
    let session = ChatSession {
        id,
        agent_stem: body.agent_stem,
        created: existing.as_ref().map(|s| s.created).unwrap_or(now),
        updated: now,
        title: body.title.or_else(|| existing.and_then(|s| s.title)),
        messages: body.messages,
    };
    match store.upsert(session) {
        Ok(s) => Json(json!(s)).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn get_session(
    State(state): State<Arc<AppState>>,
    Path((category, name, id)): Path<(String, String, String)>,
) -> impl IntoResponse {
    let agent = agent_id(&category, &name);
    match state.session_store().get(&agent, &id) {
        Ok(s) => Json(json!(s)).into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn put_session(
    State(state): State<Arc<AppState>>,
    Path((category, name, id)): Path<(String, String, String)>,
    Json(mut body): Json<ChatSession>,
) -> impl IntoResponse {
    body.agent_stem = agent_id(&category, &name);
    body.id = id;
    match state.session_store().upsert(body) {
        Ok(s) => Json(json!(s)).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
