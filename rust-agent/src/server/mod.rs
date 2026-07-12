//! HTTP server: Axum router, pages, and API handlers.
//!
//! Extracted from `main.rs` so integration tests can mount the same app.

mod api_agents;
mod api_chat;
mod api_meta;
mod api_sessions;
mod pages;
mod state;

pub use state::AppState;

use crate::{agents_dir, crate_root, registry_path, repo_root, sessions_dir};
use axum::routing::{get, post};
use axum::Router;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::services::ServeDir;

/// Build the full application router (shared by binary + tests).
pub fn build_router(state: AppState) -> Router {
    let static_dir = state.static_dir.clone();
    Router::new()
        .route("/", get(pages::home))
        .route("/agents/{category}/{name}", get(pages::agent_page))
        .route(
            "/api/agents",
            get(api_agents::list_agents_api).post(api_agents::create_agent),
        )
        .route("/api/agents/{category}/{name}", get(api_agents::get_agent))
        .route(
            "/api/agents/{category}/{name}/certify",
            get(api_agents::certify),
        )
        .route("/api/schema", get(api_meta::schema))
        .route("/api/nav", get(api_meta::nav))
        .route("/api/tools", get(api_meta::list_tools))
        .route("/api/tools/invoke", post(api_meta::invoke_tool_api))
        .route(
            "/api/sessions",
            get(api_sessions::list_sessions).post(api_sessions::upsert_session),
        )
        .route(
            "/api/sessions/{category}/{name}/{id}",
            get(api_sessions::get_session).put(api_sessions::put_session),
        )
        .route("/api/chat/stream", post(api_chat::chat_stream))
        .route("/healthz", get(|| async { "ok" }))
        .nest_service("/static", ServeDir::new(static_dir))
        .with_state(Arc::new(state))
}

/// Default state for the running binary / integration tests.
pub fn default_state() -> AppState {
    AppState {
        agents_dir: agents_dir(),
        registry_path: registry_path(),
        sessions_dir: sessions_dir(),
        crate_root: crate_root(),
        repo_root: repo_root(),
        static_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static"),
    }
}
