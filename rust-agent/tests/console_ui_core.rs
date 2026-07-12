//! Host tests drive the **same** console_core parsers used by the WASM UI.

use console_core::{feed_sse_buffer, parse_sse_data_line, render_markdown, SseEvent};
use rust_agent::parse_sse_data_line as host_reexport;

#[test]
fn stream_parser_matches_ui_crate() {
    let line = r#"data: {"delta":"hello"}"#;
    assert_eq!(
        parse_sse_data_line(line),
        SseEvent::Delta("hello".into())
    );
    // rust_agent re-exports the same function the WASM client uses
    assert_eq!(
        host_reexport(line),
        SseEvent::Delta("hello".into())
    );
    assert!(matches!(
        parse_sse_data_line(r#"data: {"tool_call":{"name":"list_tools","arguments":{}}}"#),
        SseEvent::ToolCall(_)
    ));
    // multi-line buffer path used by streaming client
    let (ev, rest) = feed_sse_buffer(
        "data: {\"delta\":\"a\"}\ndata: {\"tool_result\":{\"name\":\"list_tools\",\"ok\":true}}\npartial",
    );
    assert_eq!(ev.len(), 2);
    assert!(matches!(ev[0], SseEvent::Delta(_)));
    assert!(matches!(ev[1], SseEvent::ToolResult(_)));
    assert_eq!(rest, "partial");
}

#[test]
fn markdown_renders_lists_for_assistant_presentation() {
    let html = render_markdown("**Tools**\n\n- list_tools\n- app_status\n");
    assert!(html.contains("<strong>Tools</strong>"));
    assert!(html.contains("<li>list_tools</li>"));
}
