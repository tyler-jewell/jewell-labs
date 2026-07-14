//! Tool-call protocol: system prompt appendix and extraction from model text.

use super::allowlist::filter_tools_for_agent;
use super::types::ToolCall;
use serde_json::{json, Value};

pub fn tools_system_appendix(allowed: &[String]) -> String {
    let tools = filter_tools_for_agent(allowed);
    if tools.is_empty() {
        return "## Tools\n\nNo tools are enabled for this agent (frontmatter `tools:` is empty).\n"
            .into();
    }
    // Mechanical only: call format + schema list. Behavioral policy lives in agent body.
    let mut lines = vec![
        "## Built-in tools (host-enforced)".to_string(),
        "You may only call tools listed below.".to_string(),
        "To call a tool, emit EXACTLY one fenced block and nothing else in that turn:".to_string(),
        "```tool".to_string(),
        r#"{"name":"list_tools","arguments":{}}"#.to_string(),
        "```".to_string(),
        "Then wait for tool_result.".to_string(),
        "".to_string(),
        "### Allowed tools".to_string(),
    ];
    for t in &tools {
        lines.push(format!(
            "- **{}** (`{}`): {}",
            t.name, t.category, t.description
        ));
    }
    lines.join("\n")
}

pub fn system_with_tools(agent_body: &str, allowed: &[String]) -> String {
    format!(
        "{}\n\n{}",
        agent_body.trim(),
        tools_system_appendix(allowed)
    )
}

/// Pull a tool call from model text. Accepts:
/// - ```tool / ```json / ```nohighlight / bare ``` fences with JSON body
/// - bare JSON object with name + arguments
/// - legacy `tool_call NAME {...}` line
pub fn extract_tool_call(text: &str) -> Option<ToolCall> {
    // Prefer fenced blocks (any language tag).
    let mut search = text;
    while let Some(start) = search.find("```") {
        let after_open = &search[start + 3..];
        // skip optional language tag line
        let after_tag = if let Some(nl) = after_open.find('\n') {
            &after_open[nl + 1..]
        } else {
            after_open
        };
        if let Some(end) = after_tag.find("```") {
            let body = after_tag[..end].trim();
            if let Some(call) = parse_tool_json(body) {
                return Some(call);
            }
            search = &after_tag[end + 3..];
        } else {
            break;
        }
    }

    // Whole-string JSON
    if let Some(call) = parse_tool_json(text.trim()) {
        return Some(call);
    }

    // First {...} substring that parses as a tool call
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end > start {
                if let Some(call) = parse_tool_json(&text[start..=end]) {
                    return Some(call);
                }
            }
        }
    }

    for line in text.lines() {
        let line = line.trim();
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("tool_call ") {
            let orig = &line["tool_call ".len()..];
            let mut parts = orig.splitn(2, char::is_whitespace);
            if let Some(name) = parts.next() {
                let args = parts.next().unwrap_or("{}");
                let arguments = serde_json::from_str(args).unwrap_or(json!({}));
                return Some(ToolCall {
                    name: name.trim().to_string(),
                    arguments,
                });
            }
        }
    }
    None
}

fn parse_tool_json(body: &str) -> Option<ToolCall> {
    if let Ok(call) = serde_json::from_str::<ToolCall>(body) {
        if !call.name.is_empty() {
            return Some(call);
        }
    }
    let v: Value = serde_json::from_str(body).ok()?;
    let name = v
        .get("name")
        .or_else(|| v.get("tool"))
        .and_then(|x| x.as_str())?;
    if name.is_empty() {
        return None;
    }
    let arguments = v.get("arguments").cloned().unwrap_or(json!({}));
    Some(ToolCall {
        name: name.to_string(),
        arguments,
    })
}

#[cfg(test)]
mod extract_tests {
    use super::*;

    #[test]
    fn extracts_tool_fence() {
        let t = "```tool\n{\"name\":\"list_tools\",\"arguments\":{}}\n```";
        assert_eq!(extract_tool_call(t).unwrap().name, "list_tools");
    }

    #[test]
    fn extracts_nohighlight_fence_like_qwen() {
        // Live 0.6B often emits ```nohighlight instead of ```tool
        let t = "```nohighlight\n{\"name\":\"list_tools\",\"arguments\":{}}\n```";
        let call = extract_tool_call(t).expect("must extract nohighlight tool JSON");
        assert_eq!(call.name, "list_tools");
    }

    #[test]
    fn extracts_bare_json() {
        let t = r#"{"name":"list_tools","arguments":{}}"#;
        assert_eq!(extract_tool_call(t).unwrap().name, "list_tools");
    }

    #[test]
    fn rejects_non_tool_json() {
        assert!(extract_tool_call(r#"{"foo":1}"#).is_none());
        assert!(extract_tool_call("hello there").is_none());
    }

    #[test]
    fn tools_appendix_is_mechanical_not_policy() {
        // Behavioral policy must live in agent body, not the host tool appendix.
        let appendix = tools_system_appendix(&["list_tools".into(), "fs_write".into()]);
        assert!(
            appendix.contains("```tool"),
            "appendix must teach the call fence format: {appendix}"
        );
        assert!(
            appendix.contains("list_tools") && appendix.contains("fs_write"),
            "appendix must list allowed tool names: {appendix}"
        );
        for banned in [
            "do not invent",
            "call list_tools first",
            "answer the user in clear markdown",
            "call the appropriate tool",
            "rather than only describing",
            "fs_write) rather",
        ] {
            assert!(
                !appendix.to_ascii_lowercase().contains(banned),
                "tools appendix must not contain policy phrase {banned:?}: {appendix}"
            );
        }
    }

    #[test]
    fn system_with_tools_keeps_agent_body_as_policy_ssot() {
        let body = "You are the core orchestrator.\nAfter each tool_result, keep calling tools until done.";
        let sys = system_with_tools(body, &["list_tools".into()]);
        assert!(sys.starts_with("You are the core orchestrator"));
        assert!(sys.contains("## Built-in tools"));
        assert!(sys.contains("keep calling tools until done"));
    }
}
