//! HTTP integration tests against the real Axum router (no external server).
//! These are the tests that prove "the app works" when green.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use rust_agent::server::{build_router, default_state};
use serde_json::Value;
use tower::ServiceExt;

async fn json_get(path: &str) -> (StatusCode, Value) {
    let app = build_router(default_state());
    let res = app
        .oneshot(
            Request::builder()
                .uri(path)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, v)
}

async fn json_post(path: &str, body: Value) -> (StatusCode, Value) {
    let app = build_router(default_state());
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, v)
}

#[tokio::test]
async fn healthz_ok() {
    let app = build_router(default_state());
    let res = app
        .oneshot(Request::builder().uri("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn home_redirects_to_orchestrator() {
    let app = build_router(default_state());
    let res = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert!(
        res.status().is_redirection() || res.status() == StatusCode::OK,
        "status={}",
        res.status()
    );
    let loc = res
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !loc.is_empty() {
        assert!(
            loc.contains("core/orchestrator"),
            "location={loc}"
        );
    }
}

#[tokio::test]
async fn orchestrator_page_agents_only_sidebar() {
    let app = build_router(default_state());
    let res = app
        .oneshot(
            Request::builder()
                .uri("/agents/core/orchestrator?tab=chat")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = String::from_utf8(
        res.into_body().collect().await.unwrap().to_bytes().to_vec(),
    )
    .unwrap();
    assert!(body.contains("Agents") || body.contains("agents"));
    assert!(body.contains("orchestrator"));
    // tools are metadata chips, not peer sidebar nav
    assert!(
        !body.contains("sidebar-path\">tools/")
            && !body.contains("aria-label=\"Tools\""),
        "tools must not be peer sidebar nav"
    );
    assert!(body.contains("tool-meta") || body.contains("Available tools"));
    assert!(
        body.contains("/static/pkg/boot.js"),
        "must load WASM bootstrap only"
    );
    assert!(!body.contains("/static/app.js"), "product app.js must be gone");
}

#[tokio::test]
async fn wasm_pkg_glue_served() {
    for uri in ["/static/pkg/boot.js", "/static/pkg/console_wasm.js"] {
        let app = build_router(default_state());
        let res = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "{uri}");
        let body = String::from_utf8(
            res.into_body().collect().await.unwrap().to_bytes().to_vec(),
        )
        .unwrap();
        assert!(
            body.contains("GENERATED")
                || body.contains("wasm")
                || body.contains("console_wasm")
                || body.contains("import"),
            "{uri} unexpected content"
        );
    }
}

#[tokio::test]
async fn api_lists_core_orchestrator_and_tools() {
    let (st, agents) = json_get("/api/agents").await;
    assert_eq!(st, StatusCode::OK);
    let list = agents["agents"].as_array().expect("agents array");
    assert!(
        list.iter().any(|a| a["id"] == "core/orchestrator"),
        "missing core orchestrator: {agents}"
    );

    let (st, tools) = json_get("/api/tools").await;
    assert_eq!(st, StatusCode::OK);
    let t = tools["tools"].as_array().expect("tools");
    assert_eq!(t.len(), 15, "lean tool surface expected 15, got {}", t.len());
    assert!(t.iter().any(|x| x["name"] == "list_tools"));
    assert!(t.iter().all(|x| x["name"] != "search_chat_logs"));
}

#[tokio::test]
async fn core_orchestrator_can_invoke_list_tools_via_api() {
    let (st, body) = json_post(
        "/api/tools/invoke",
        serde_json::json!({
            "name": "list_tools",
            "arguments": {},
            "caller_agent": "core/orchestrator"
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    assert_eq!(body["ok"], true, "{body}");
    let tools = body["result"]["tools"].as_array().expect("tools");
    assert_eq!(tools.len(), 15);
}

#[tokio::test]
async fn math_tutor_denied_list_agents() {
    let (st, body) = json_post(
        "/api/tools/invoke",
        serde_json::json!({
            "name": "list_agents",
            "arguments": {},
            "caller_agent": "tutoring/math-tutor"
        }),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert_eq!(body["ok"], false);
}

#[tokio::test]
async fn agent_page_and_schema_tab_render() {
    let app = build_router(default_state());
    let res = app
        .oneshot(
            Request::builder()
                .uri("/agents/core/orchestrator?tab=schema")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = String::from_utf8(
        res.into_body().collect().await.unwrap().to_bytes().to_vec(),
    )
    .unwrap();
    assert!(body.contains("orchestrator") || body.contains("core/orchestrator"));
    assert!(body.contains("src/schema.rs") || body.contains("Schema"));
}

#[tokio::test]
async fn nav_api_lists_src_aligned_tabs() {
    let (st, nav) = json_get("/api/nav").await;
    assert_eq!(st, StatusCode::OK);
    let tabs = nav["agent_tabs"].as_array().unwrap();
    let ids: Vec<_> = tabs.iter().filter_map(|t| t["id"].as_str()).collect();
    assert_eq!(ids, vec!["chat", "sessions", "schema", "registry"]);
}
