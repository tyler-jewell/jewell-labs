//! Agent tools living at `rust-agent/tools/{category}/{tool-name}.rs`.
//!
//! Both `src/*` and agent markdown files use this registry. Agents only receive
//! tools listed in frontmatter `tools:` (or `*` for all).

pub mod fs;
pub mod introspect;
pub mod mutate;
pub mod sessions;

mod allowlist;
mod context;
mod invoke;
mod protocol;
mod types;

pub use allowlist::{
    filter_tools_for_agent, resolve_allowlist, tool_matches_allowlist, validate_tool_allowlist,
};
pub use context::{sessions_dir, ToolContext};
pub use invoke::{all_tool_names, builtin_tools, invoke_tool, invoke_tool_call};
pub use protocol::{extract_tool_call, system_with_tools, tools_system_appendix};
pub use types::{
    arg_bool, arg_str, arg_u64, ToolCall, ToolError, ToolResult, ToolSpec, MAX_TOOL_ROUNDS,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::{ChatSession, SessionMessage};
    use chrono::Utc;
    use serde_json::json;
    use tempfile::tempdir;

    fn ctx_tmp(dir: &std::path::Path) -> ToolContext {
        let agents = dir.join("agents");
        let sessions = dir.join("sessions");
        let models = dir.join("models");
        std::fs::create_dir_all(agents.join("demo")).unwrap();
        std::fs::create_dir_all(&models).unwrap();
        std::fs::write(
            agents.join("demo").join("bot.md"),
            r#"---
schema_version: 1
name: bot
description: d
default_model: qwen3-0.6b
role: agent
tools: ["*"]
---

hello
"#,
        )
        .unwrap();
        std::fs::write(
            models.join("registry.yaml"),
            "models:\n  qwen3-0.6b:\n    path: /tmp/x.gguf\n    alias: q\n",
        )
        .unwrap();
        ToolContext::from_paths(
            agents,
            models.join("registry.yaml"),
            sessions,
            dir.to_path_buf(),
            dir.to_path_buf(),
            Some("demo/bot".into()),
            vec!["*".into()],
        )
    }

    #[test]
    fn registry_has_categories() {
        let tools = builtin_tools();
        assert!(tools.iter().any(|t| t.category == "introspect"));
        assert!(tools.iter().any(|t| t.category == "sessions"));
        assert_eq!(tools.len(), 19); // + fs_write/fs_read/fs_list
        assert!(tools.iter().any(|t| t.category == "fs"));
    }

    #[test]
    fn agent_fs_write_read_and_escape_denied() {
        let dir = tempdir().unwrap();
        let ctx = ctx_tmp(dir.path());
        // write
        let w = invoke_tool(
            &ctx,
            "fs_write",
            &json!({"path": "notes/hello.txt", "content": "sandbox-ok"}),
        );
        assert!(w.ok, "{:?}", w.result);
        let r = invoke_tool(&ctx, "fs_read", &json!({"path": "notes/hello.txt"}));
        assert!(r.ok, "{:?}", r.result);
        assert_eq!(r.result["content"], "sandbox-ok");
        // on disk under agents/demo/bot/fs/
        let on_disk = dir.path().join("agents/demo/bot/fs/notes/hello.txt");
        assert_eq!(std::fs::read_to_string(&on_disk).unwrap(), "sandbox-ok");
        // escape denied
        let bad = invoke_tool(
            &ctx,
            "fs_write",
            &json!({"path": "../escape.txt", "content": "x"}),
        );
        assert!(!bad.ok, "escape must fail: {:?}", bad.result);
        let abs = invoke_tool(
            &ctx,
            "fs_write",
            &json!({"path": "/tmp/nope.txt", "content": "x"}),
        );
        assert!(!abs.ok, "abs must fail: {:?}", abs.result);
    }

    #[test]
    fn sandbox_override_writes_eval_workspace() {
        let dir = tempdir().unwrap();
        let mut ctx = ctx_tmp(dir.path());
        // Override must live under {repo_root}/evals/runs/ (eval pin policy).
        let ws = dir.path().join("evals/runs/catalog-test/workspace");
        std::fs::create_dir_all(&ws).unwrap();
        ctx.sandbox_override = Some(ws.clone());
        let w = invoke_tool(
            &ctx,
            "fs_write",
            &json!({"path": "hello.txt", "content": "Hello, Terminal-Bench"}),
        );
        assert!(w.ok, "{:?}", w.result);
        assert_eq!(
            std::fs::read_to_string(ws.join("hello.txt"))
                .unwrap()
                .trim(),
            "Hello, Terminal-Bench"
        );
        // Outside evals/runs is denied
        ctx.sandbox_override = Some(dir.path().join("secrets"));
        let bad = invoke_tool(
            &ctx,
            "fs_write",
            &json!({"path": "x.txt", "content": "nope"}),
        );
        assert!(
            !bad.ok,
            "must reject fs outside evals/runs: {:?}",
            bad.result
        );
    }

    #[test]
    fn allowlist_star_and_category() {
        let all = resolve_allowlist(&["*".into()]);
        assert_eq!(all.len(), builtin_tools().len());
        let sess = resolve_allowlist(&["sessions/*".into()]);
        assert!(sess.iter().all(|n| {
            builtin_tools()
                .iter()
                .find(|t| t.name == *n)
                .map(|t| t.category == "sessions")
                .unwrap_or(false)
        }));
        assert!(!sess.is_empty());
    }

    #[test]
    fn deny_when_not_allowed() {
        let dir = tempdir().unwrap();
        let mut ctx = ctx_tmp(dir.path());
        ctx.allowed_tools = vec!["list_agents".into()];
        let denied = invoke_tool(&ctx, "app_status", &json!({}));
        assert!(!denied.ok);
        let ok = invoke_tool(&ctx, "list_agents", &json!({}));
        assert!(ok.ok, "{:?}", ok.result);
    }

    #[test]
    fn extract_and_search() {
        let text = r#"```tool
{"name":"list_agents","arguments":{}}
```"#;
        assert_eq!(extract_tool_call(text).unwrap().name, "list_agents");

        let dir = tempdir().unwrap();
        let ctx = ctx_tmp(dir.path());
        ctx.sessions
            .upsert(ChatSession {
                id: "abc".into(),
                agent_stem: "demo/bot".into(),
                created: Utc::now(),
                updated: Utc::now(),
                title: None,
                messages: vec![SessionMessage {
                    role: "user".into(),
                    content: "widgets please".into(),
                    ts: Some(Utc::now()),
                }],
            })
            .unwrap();
        let search = invoke_tool(&ctx, "list_sessions", &json!({"query": "widgets"}));
        assert!(search.ok, "{:?}", search.result);
        assert_eq!(search.result["mode"], "search");
        assert_eq!(search.result["count"], 1);
    }
}
