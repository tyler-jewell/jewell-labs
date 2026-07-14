//! Real-path tests for presence API and console event normalization.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use rust_agent::server::{build_router, default_state};
use rust_agent::{
    fold_run_events, parse_sse_data_line, run_event_from_sse, PresenceBoard, RunKind, SseEvent,
};
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
async fn presence_api_lists_agents_with_idle_or_busy() {
    let state = default_state();
    // Mark core orchestrator busy before mounting.
    state.presence.set_busy("core/orchestrator", Some("sess-1"));
    let app = build_router(state);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/presence")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    let agents = v["agents"].as_array().expect("agents array");
    assert!(!agents.is_empty(), "must list agents");
    let orch = agents
        .iter()
        .find(|a| a["agent_id"] == "core/orchestrator")
        .expect("orchestrator row");
    assert_eq!(orch["status"], "busy");
    assert_eq!(orch["session_id"], "sess-1");
    // Groups present for project path grouping
    assert!(v["groups"]
        .as_array()
        .map(|g| !g.is_empty())
        .unwrap_or(false));
}

#[tokio::test]
async fn agent_page_renders_presence_chrome_and_run_events() {
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
    let body =
        String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    assert!(body.contains("viewport"), "viewport meta");
    assert!(body.contains("presence") || body.contains("data-presence"));
    assert!(body.contains("run-events") || body.contains("id=\"run-events\""));
    assert!(body.contains("name=\"viewport\"") || body.contains("width=device-width"));
}

#[test]
fn board_and_sse_normalization_shipped_path() {
    let board = PresenceBoard::new();
    board.set_busy("system/learner", None);
    assert_eq!(board.snapshot()[0].status.as_str(), "busy");
    board.set_idle("system/learner");
    assert_eq!(board.snapshot()[0].status.as_str(), "idle");

    // Drive real parse_sse_data_line → run_event_from_sse (same as WASM client).
    let tool = parse_sse_data_line(r#"data: {"tool_call":{"name":"list_tools","arguments":{}}}"#);
    let re = run_event_from_sse(&tool).expect("tool event");
    assert_eq!(re.kind, RunKind::ToolCall);

    let stream = [
        r#"data: {"status":"busy"}"#,
        r#"data: {"delta":"hi"}"#,
        r#"data: {"delta":" there"}"#,
        r#"data: {"tool_result":{"name":"list_tools","ok":true,"result":{}}}"#,
        r#"data: {"error":"nope"}"#,
    ];
    let events: Vec<SseEvent> = stream.iter().map(|l| parse_sse_data_line(l)).collect();
    let folded = fold_run_events(&events);
    assert!(
        folded.iter().any(|e| e.kind == RunKind::Status),
        "status event"
    );
    assert!(
        folded.iter().any(|e| e.kind == RunKind::Model),
        "model event"
    );
    assert!(
        folded.iter().any(|e| e.kind == RunKind::ToolResult),
        "tool result"
    );
    assert!(
        folded.iter().any(|e| e.kind == RunKind::Error),
        "error event"
    );
}
