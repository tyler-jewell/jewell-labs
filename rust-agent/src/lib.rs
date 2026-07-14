//! Core library: schema certification, agent listing, registry, chat, tools, UI.

pub mod agent_run;
pub mod agents;
pub mod app;
pub mod chat;
/// Re-export pure UI parsers used by host tests (same as WASM client).
pub use console_core::{
    fold_run_events, parse_sse_data_line, render_markdown, run_event_from_sse, RunEvent, RunKind,
    SseEvent,
};
pub mod eval;
pub mod eval_guards;
pub mod presence;
pub use presence::{PresenceBoard, PresenceRow, PresenceStatus};
pub mod jail;
pub mod learn;
pub mod model_eval;
pub mod model_research;
pub mod nav;
pub mod paths;
pub mod registry;
pub mod schema;
pub mod server;
pub mod sessions;
pub mod tool_fs;

/// Module path alias for eval.
pub use eval as eval_introspection;

/// Tools live at `rust-agent/tools/{category}/{tool-name}.rs` (shared by src + agents).
#[path = "../tools/mod.rs"]
pub mod tools;

pub use agent_run::{run_agent_with_tools, run_tool_plan, AgentRunResult, ToolRound};
pub use agents::{
    agent_id, certify_agent_file, list_agents, load_agent, parse_agent_ref, path_is_under_agents,
    resolve_agent_path, validate_segment, validate_stem, write_agent_file,
    write_agent_file_unlocked, AgentListItem, AgentsError, AGENT_IMPLEMENTOR_ID, CORE_AGENT_ID,
    CORE_AGENT_TOOLS, LEARNER_ID, TEAM_AGENT_IDS, TOOL_IMPLEMENTOR_ID,
};
pub use chat::{complete_chat, stream_chat, ChatEndpoint, ChatError, ChatMessage, ChatRequest};
pub use eval::{
    default_vendor_ids, eval_all_agents, evals_runs_dir, filter_items, known_harnesses,
    list_eval_runs, list_source_summaries, load_catalog_items, load_eval_model, load_eval_run,
    load_vendor_manifests, probe_eval_depth_exceeded, require_green_eval, run_agent_eval,
    run_catalog_sample, run_compare, run_dry_all, run_full_eval, run_team_collaboration_case,
    sample_items, write_eval_report, write_report, CatalogFilter, CatalogItem, CatalogRunOpts,
    CatalogRunReport, CompareItem, CompareOpts, CompareReport, EvalError, EvalModel, EvalReport,
    EvalRunSummary, GroundTruth, SourceSummary, VendorManifest,
};
pub use jail::{agent_file_rel, JailError, WriteJail};
pub use learn::{learn_improve, LearnReport, LearnRequest};
pub use model_eval::{score_inference_target, score_project_eval, ProjectEvalScore};
pub use model_research::{
    model_fitness, project_tool_plan_score, run_model_research, write_research_report,
    ModelResearchReport, ModelResearchRequest,
};
// re-export probe via eval module
pub use nav::{
    agent_tab_def, default_agent_tab, group_by_category, is_valid_agent_tab, normalize_agent_tab,
    AgentTabDef, AGENT_TABS,
};
pub use paths::{agents_dir, crate_root, registry_path, repo_root, sessions_dir, tools_dir};
pub use registry::{ModelRegistry, RegistryError, ResolvedModel};
pub use schema::{
    certify_agent_markdown, parse_frontmatter, schema_summary, AgentDocument, AgentFrontmatter,
    CertificationResult, LATEST_SCHEMA_VERSION,
};
pub use sessions::{
    ChatSession, MessageHit, SessionError, SessionMessage, SessionStore, SessionSummary,
};
pub use tool_fs::{list_agent_local_tools, list_tools_from_fs, ToolFileEntry};
pub use tools::{
    all_tool_names, builtin_tools, extract_tool_call, filter_tools_for_agent, invoke_tool,
    invoke_tool_call, resolve_allowlist, system_with_tools, tools_system_appendix,
    validate_tool_allowlist, ToolCall, ToolContext, ToolResult, ToolSpec, MAX_TOOL_ROUNDS,
};
