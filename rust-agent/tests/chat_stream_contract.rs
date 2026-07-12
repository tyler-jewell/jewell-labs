//! Chat stream contract: host must extract tool calls the weak model actually emits
//! and surface tool_call/tool_result SSE events — not leave raw JSON as the only "answer".
//!
//! This is the regression for the UI showing:
//!   ASSISTANT  {"name":"list_tools","arguments":{}}

use rust_agent::{extract_tool_call, invoke_tool, ToolContext, CORE_AGENT_ID};
use rust_agent::{agents_dir, crate_root, registry_path, repo_root, sessions_dir};

fn orch_ctx() -> ToolContext {
    let agents = agents_dir();
    let doc = rust_agent::load_agent(&agents, CORE_AGENT_ID).unwrap();
    ToolContext::from_paths(
        agents,
        registry_path(),
        sessions_dir(),
        crate_root(),
        repo_root(),
        Some(CORE_AGENT_ID.into()),
        doc.frontmatter.tools.clone(),
    )
}

/// Simulated model completions observed in production (0.6B).
fn weak_model_tool_emissions() -> Vec<&'static str> {
    vec![
        "```tool\n{\"name\":\"list_tools\",\"arguments\":{}}\n```",
        "```nohighlight\n{\"name\":\"list_tools\",\"arguments\":{}}\n```",
        "```json\n{\"name\":\"list_tools\",\"arguments\":{}}\n```",
        "{\"name\":\"list_tools\",\"arguments\":{}}",
        // with prose prefix (still extractable)
        "Sure.\n```nohighlight\n{\"name\":\"list_tools\",\"arguments\":{}}\n```\n",
    ]
}

#[test]
fn host_extracts_every_weak_model_tool_shape() {
    for sample in weak_model_tool_emissions() {
        let call = extract_tool_call(sample)
            .unwrap_or_else(|| panic!("failed to extract from: {sample:?}"));
        assert_eq!(call.name, "list_tools", "sample={sample:?}");
    }
}

#[test]
fn extracted_list_tools_invokes_successfully() {
    let ctx = orch_ctx();
    for sample in weak_model_tool_emissions() {
        let call = extract_tool_call(sample).unwrap();
        let result = invoke_tool(&ctx, &call.name, &call.arguments);
        assert!(
            result.ok,
            "invoke failed for sample={sample:?}: {:?}",
            result.result
        );
        assert!(
            result.result.get("tools").and_then(|t| t.as_array()).map(|a| !a.is_empty())
                == Some(true),
            "expected tools array: {:?}",
            result.result
        );
    }
}

#[test]
fn raw_tool_json_must_not_be_treated_as_final_answer_shape() {
    // If extraction fails, the UI shows this string as the assistant message (the bug).
    // Guard: extraction must succeed so the host can continue the tool loop.
    let displayed_bug = r#"{"name":"list_tools","arguments":{}}"#;
    assert!(
        extract_tool_call(displayed_bug).is_some(),
        "this exact UI-visible string must be recognized as a tool call, not a final answer"
    );
}
