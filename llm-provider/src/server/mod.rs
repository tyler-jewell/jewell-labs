//! Router assembly + the `/v1` handlers. Auth is a layer on the `/v1` sub-router, so the
//! handlers never mention it; the mint/login routes stay open.

pub mod auth_mw;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{middleware, Json, Router};
use serde_json::{json, Value};

use crate::app::AppState;
use crate::identity::oauth_google;
use crate::openai::{client_error, error_response};

pub fn router(app: AppState) -> Router {
    let v1 = Router::new()
        .route("/models", get(models_handler))
        .route("/chat/completions", post(chat_handler))
        .layer(middleware::from_fn_with_state(app.clone(), auth_mw::gate));

    Router::new()
        .route("/", get(oauth_google::index))
        .route("/healthz", get(|| async { Json(json!({"ok": true})) }))
        .route("/login", get(oauth_google::login))
        .route("/oauth/callback", get(oauth_google::callback))
        .route("/auth/google", post(oauth_google::google_token))
        .route("/auth/refresh", post(oauth_google::refresh))
        .nest("/v1", v1)
        .with_state(app)
}

async fn models_handler(State(app): State<AppState>) -> Response {
    let data = app.registry.all_models(&app.http).await;
    Json(json!({"object": "list", "data": data})).into_response()
}

async fn chat_handler(State(app): State<AppState>, Json(body): Json<Value>) -> Response {
    let model = body["model"].as_str().unwrap_or("").to_string();
    match app.registry.route(&app.http, &model).await {
        Some(p) => p
            .chat(&app.http, body)
            .await
            .unwrap_or_else(|e| error_response(StatusCode::BAD_GATEWAY, format!("Upstream error: {e}"))),
        None => client_error(
            StatusCode::NOT_FOUND,
            "model_not_found",
            format!("Unknown model '{model}'. See GET /v1/models."),
        ),
    }
}
