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
    // Advertise the central-config default so clients never hardcode a model: the local
    // model that is the gateway's first point of contact.
    Json(json!({
        "object": "list",
        "data": data,
        "default_model": app.cfg.default_model,
    }))
    .into_response()
}

async fn chat_handler(State(app): State<AppState>, Json(body): Json<Value>) -> Response {
    // The local go-to model is the first point of contact: a request with no model (or an
    // empty one) is served by the configured default. An explicit model name is honored
    // as-is. There is NO automatic cross-model fallback — escalation is the router agent's
    // decision (it calls a specific model deliberately), so a failure surfaces to the caller
    // to reason about rather than being silently retried on another model.
    let requested = match body["model"].as_str() {
        Some(m) if !m.is_empty() => m.to_string(),
        _ => app.cfg.default_model.clone(),
    };

    match app.registry.route(&app.http, &requested).await {
        Some(p) => {
            let mut attempt = body;
            attempt["model"] = Value::String(requested);
            p.chat(&app.http, attempt)
                .await
                .unwrap_or_else(|e| error_response(StatusCode::BAD_GATEWAY, format!("Upstream error: {e}")))
        }
        None => client_error(
            StatusCode::NOT_FOUND,
            "model_not_found",
            format!("Unknown model '{requested}'. See GET /v1/models."),
        ),
    }
}
