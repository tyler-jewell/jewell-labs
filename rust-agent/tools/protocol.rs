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
    let mut lines = vec![
        "## Built-in tools (Jewell Labs)".to_string(),
        "You may only call tools listed below (from your frontmatter `tools:` allowlist)."
            .to_string(),
        "To call a tool, emit EXACTLY one fenced block:".to_string(),
        "```tool".to_string(),
        r#"{"name":"TOOL_NAME","arguments":{...}}"#.to_string(),
        "```".to_string(),
        "The host returns a tool_result. Prefer tools over guessing.".to_string(),
        "".to_string(),
        "### Allowed tools".to_string(),
    ];
    for t in &tools {
        lines.push(format!(
            "- **{}** (`{}`): {}",
            t.name, t.category, t.description
        ));
    }
    lines.push("".into());
    lines.push("Use `list_tools` (if allowed) for full parameter schemas.".into());
    lines.join("\n")
}

pub fn system_with_tools(agent_body: &str, allowed: &[String]) -> String {
    format!(
        "{}\n\n{}",
        agent_body.trim(),
        tools_system_appendix(allowed)
    )
}

pub fn extract_tool_call(text: &str) -> Option<ToolCall> {
    if let Some(rest) = text.find("```tool") {
        let after = &text[rest + "```tool".len()..];
        let after = after.strip_prefix('\n').unwrap_or(after);
        if let Some(end) = after.find("```") {
            let body = after[..end].trim();
            if let Ok(call) = serde_json::from_str::<ToolCall>(body) {
                if !call.name.is_empty() {
                    return Some(call);
                }
            }
            if let Ok(v) = serde_json::from_str::<Value>(body) {
                if let Some(name) = v
                    .get("tool")
                    .and_then(|x| x.as_str())
                    .or_else(|| v.get("name").and_then(|x| x.as_str()))
                {
                    let arguments = v.get("arguments").cloned().unwrap_or(json!({}));
                    return Some(ToolCall {
                        name: name.to_string(),
                        arguments,
                    });
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
