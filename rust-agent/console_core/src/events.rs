//! Normalize chat SSE payloads into a typed run event stream for the console.

use crate::sse::SseEvent;
use serde_json::Value;

/// Distinct event kinds shown in the run event list (not a raw log dump).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunKind {
    Model,
    ToolCall,
    ToolResult,
    Error,
    Status,
}

impl RunKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Model => "model",
            Self::ToolCall => "tool_call",
            Self::ToolResult => "tool_result",
            Self::Error => "error",
            Self::Status => "status",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunEvent {
    pub kind: RunKind,
    pub label: String,
    pub detail: String,
}

fn truncate(s: &str, max: usize) -> String {
    let mut t = s.chars().take(max).collect::<String>();
    if s.chars().count() > max {
        t.push('…');
    }
    t
}

fn tool_name(v: &Value) -> String {
    v.get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("tool")
        .to_string()
}

/// Map one SSE event to a display event. Deltas collapse to a short model chip
/// only when non-empty; empty deltas are skipped.
pub fn run_event_from_sse(ev: &SseEvent) -> Option<RunEvent> {
    match ev {
        SseEvent::Skip | SseEvent::Done => None,
        SseEvent::Error(e) => Some(RunEvent {
            kind: RunKind::Error,
            label: "error".into(),
            detail: e.clone(),
        }),
        SseEvent::Delta(d) if d.is_empty() => None,
        SseEvent::Delta(d) => Some(RunEvent {
            kind: RunKind::Model,
            label: "model".into(),
            detail: truncate(d, 80),
        }),
        SseEvent::ToolCall(v) => {
            let name = tool_name(v);
            Some(RunEvent {
                kind: RunKind::ToolCall,
                label: format!("→ {name}"),
                detail: truncate(&v.to_string(), 120),
            })
        }
        SseEvent::ToolResult(v) => {
            let name = tool_name(v);
            let ok = v.get("ok").and_then(|o| o.as_bool()).unwrap_or(false);
            Some(RunEvent {
                kind: RunKind::ToolResult,
                label: format!("← {name} {}", if ok { "ok" } else { "err" }),
                detail: truncate(&v.to_string(), 120),
            })
        }
        SseEvent::Other(v) => {
            if let Some(st) = v.get("status").and_then(|s| s.as_str()) {
                return Some(RunEvent {
                    kind: RunKind::Status,
                    label: "status".into(),
                    detail: st.to_string(),
                });
            }
            None
        }
    }
}

/// Fold a sequence of SSE events into a run log, coalescing consecutive model
/// deltas into a single Model event (keeps the list readable).
pub fn fold_run_events(events: &[SseEvent]) -> Vec<RunEvent> {
    let mut out = Vec::new();
    let mut model_buf = String::new();
    let flush_model = |buf: &mut String, out: &mut Vec<RunEvent>| {
        if !buf.is_empty() {
            out.push(RunEvent {
                kind: RunKind::Model,
                label: "model".into(),
                detail: truncate(buf, 120),
            });
            buf.clear();
        }
    };
    for ev in events {
        match ev {
            SseEvent::Delta(d) if !d.is_empty() => model_buf.push_str(d),
            other => {
                flush_model(&mut model_buf, &mut out);
                if let Some(r) = run_event_from_sse(other) {
                    out.push(r);
                }
            }
        }
    }
    flush_model(&mut model_buf, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_sse_data_line;

    #[test]
    fn tool_model_error_kinds() {
        let tc = parse_sse_data_line(r#"data: {"tool_call":{"name":"list_tools","arguments":{}}}"#);
        let r = run_event_from_sse(&tc).unwrap();
        assert_eq!(r.kind, RunKind::ToolCall);
        assert!(r.label.contains("list_tools"));

        let d = parse_sse_data_line(r#"data: {"delta":"hello world"}"#);
        let r = run_event_from_sse(&d).unwrap();
        assert_eq!(r.kind, RunKind::Model);

        let e = parse_sse_data_line(r#"data: {"error":"boom"}"#);
        assert_eq!(run_event_from_sse(&e).unwrap().kind, RunKind::Error);
    }

    #[test]
    fn fold_coalesces_model_deltas() {
        let lines = [
            r#"data: {"delta":"a"}"#,
            r#"data: {"delta":"b"}"#,
            r#"data: {"tool_call":{"name":"x","arguments":{}}}"#,
            r#"data: {"delta":"c"}"#,
        ];
        let events: Vec<_> = lines.iter().map(|l| parse_sse_data_line(l)).collect();
        let folded = fold_run_events(&events);
        assert_eq!(folded.len(), 3);
        assert_eq!(folded[0].kind, RunKind::Model);
        assert_eq!(folded[0].detail, "ab");
        assert_eq!(folded[1].kind, RunKind::ToolCall);
        assert_eq!(folded[2].kind, RunKind::Model);
        assert_eq!(folded[2].detail, "c");
    }

    #[test]
    fn status_from_other() {
        let ev = parse_sse_data_line(r#"data: {"status":"thinking"}"#);
        let r = run_event_from_sse(&ev).unwrap();
        assert_eq!(r.kind, RunKind::Status);
        assert_eq!(r.detail, "thinking");
    }
}
