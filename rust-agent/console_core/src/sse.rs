//! SSE line parser for `/api/chat/stream` — same contract as the former static/js/sse.js.

use serde_json::Value;

/// One decoded SSE `data:` payload from the chat stream.
#[derive(Debug, Clone, PartialEq)]
pub enum SseEvent {
    Skip,
    Done,
    Error(String),
    Delta(String),
    ToolCall(Value),
    ToolResult(Value),
    Other(Value),
}

/// Parse one SSE line. Drives both unit tests and the WASM chat client.
pub fn parse_sse_data_line(line: &str) -> SseEvent {
    let trimmed = line.trim();
    if !trimmed.starts_with("data:") {
        return SseEvent::Skip;
    }
    let data = trimmed[5..].trim();
    if data == "[DONE]" {
        return SseEvent::Done;
    }
    let Ok(obj) = serde_json::from_str::<Value>(data) else {
        return SseEvent::Skip;
    };
    if let Some(err) = obj.get("error") {
        return SseEvent::Error(
            err.as_str()
                .map(|s| s.to_string())
                .unwrap_or_else(|| err.to_string()),
        );
    }
    if let Some(tc) = obj.get("tool_call") {
        return SseEvent::ToolCall(tc.clone());
    }
    if let Some(tr) = obj.get("tool_result") {
        return SseEvent::ToolResult(tr.clone());
    }
    if let Some(d) = obj.get("delta") {
        return SseEvent::Delta(
            d.as_str()
                .map(|s| s.to_string())
                .unwrap_or_else(|| d.to_string()),
        );
    }
    SseEvent::Other(obj)
}

/// Feed a multi-line SSE buffer; returns events + remainder (incomplete last line).
/// Used by the WASM client and host stream tests.
#[allow(dead_code)] // also exercised in unit tests via feed_buffer_splits_lines
pub fn feed_sse_buffer(buf: &str) -> (Vec<SseEvent>, String) {
    let mut events = Vec::new();
    let mut rest = String::new();
    let mut parts = buf.split('\n').collect::<Vec<_>>();
    if !buf.ends_with('\n') {
        if let Some(last) = parts.pop() {
            rest = last.to_string();
        }
    }
    for line in parts {
        let ev = parse_sse_data_line(line);
        if !matches!(ev, SseEvent::Skip) {
            events.push(ev);
        }
    }
    (events, rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delta_done_skip() {
        assert_eq!(
            parse_sse_data_line(r#"data: {"delta":"hi"}"#),
            SseEvent::Delta("hi".into())
        );
        assert_eq!(parse_sse_data_line("data: [DONE]"), SseEvent::Done);
        assert_eq!(parse_sse_data_line(": keepalive"), SseEvent::Skip);
        assert_eq!(parse_sse_data_line(""), SseEvent::Skip);
    }

    #[test]
    fn tool_events_and_error() {
        let tc = parse_sse_data_line(r#"data: {"tool_call":{"name":"list_tools","arguments":{}}}"#);
        match tc {
            SseEvent::ToolCall(v) => assert_eq!(v["name"], "list_tools"),
            _ => panic!("{tc:?}"),
        }
        let tr = parse_sse_data_line(
            r#"data: {"tool_result":{"name":"list_tools","ok":true,"result":{}}}"#,
        );
        assert!(matches!(tr, SseEvent::ToolResult(_)));
        assert_eq!(
            parse_sse_data_line(r#"data: {"error":"boom"}"#),
            SseEvent::Error("boom".into())
        );
    }

    #[test]
    fn feed_buffer_splits_lines() {
        let (ev, rest) =
            feed_sse_buffer("data: {\"delta\":\"a\"}\ndata: {\"delta\":\"b\"}\npartial");
        assert_eq!(ev.len(), 2);
        assert_eq!(rest, "partial");
    }
}
