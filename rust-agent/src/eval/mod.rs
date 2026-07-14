//! Agent evals: structural tool_plan (CI gate) + multi-vendor catalog + LLM track.
//!
//! Host-runnable for any agent via [`agent_eval::run_agent_eval`] with depth ≤ 1.
//! Multi-vendor catalog: discover `evals/vendors/*/vendor.toml`, pin shared `evals/model.toml`.

mod agent_eval;
mod cases;
pub mod catalog;
pub mod compare;
mod ground_truth;
mod llm_case;
mod runner;
mod scoring;
mod store;
mod team;
pub mod vendors;

pub use agent_eval::{
    dataset_path_for, eval_all_agents, get_eval_depth, parse_dataset, require_green_eval,
    run_agent_eval, set_eval_depth, DatasetCase, EvalError,
};

/// Host probe: with depth already ≥1, run_agent_eval must return DepthExceeded.
pub fn probe_eval_depth_exceeded(agent_id: &str) -> bool {
    let prev = get_eval_depth();
    set_eval_depth(1);
    let r = run_agent_eval(agent_id, "tool_plan");
    set_eval_depth(prev);
    matches!(r, Err(EvalError::DepthExceeded))
}
pub use cases::{
    core_plan_require_tools, full_introspection_plan, orchestrator_ctx, run_tool_plan_case,
};
pub use catalog::{
    filter_items, list_source_summaries, load_catalog_items, run_catalog_sample, sample_items,
    CatalogFilter, CatalogItem, CatalogRunOpts, CatalogRunReport, SourceSummary,
};
pub use compare::{
    known_harnesses, run_compare, run_dry_all, CompareItem, CompareOpts, CompareReport,
};
pub use ground_truth::{CaseResult, EvalReport, EvalSummary, FactResult, GroundTruth};
pub use llm_case::run_llm_case;
pub use runner::{run_full_eval, write_report};
pub use scoring::score_introspection;
pub use store::{evals_runs_dir, list_eval_runs, load_eval_run, write_eval_report, EvalRunSummary};
pub use team::run_team_collaboration_case;
pub use vendors::{
    default_vendor_ids, load_eval_model, load_vendor_manifests, EvalModel, VendorManifest,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::CORE_AGENT_TOOLS;

    #[test]
    fn tool_plan_complete_introspection() {
        let (ctx, doc) = orchestrator_ctx();
        assert_eq!(doc.id, crate::agents::CORE_AGENT_ID);
        assert_eq!(doc.frontmatter.role, "orchestrator");
        let gt = GroundTruth::collect(&ctx);
        assert!(gt.agent_ids.contains(crate::agents::CORE_AGENT_ID));
        // list_tools is allowlist-filtered → orch sees CORE tools only
        assert_eq!(gt.tool_count, CORE_AGENT_TOOLS.len());
        let case = run_tool_plan_case(&ctx, &gt);
        assert!(
            case.correct,
            "tool plan must fully introspect; facts={:?}",
            case.facts.iter().filter(|f| !f.correct).collect::<Vec<_>>()
        );
        let unique: std::collections::BTreeSet<_> = case.tools_called.iter().cloned().collect();
        assert!(unique.iter().all(|n| n != "run_eval"));
        let required: std::collections::BTreeSet<_> = core_plan_require_tools()
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        assert!(
            required.is_subset(&unique),
            "missing {:?}",
            required.difference(&unique)
        );
    }

    #[test]
    fn team_collaboration_green() {
        let case = run_team_collaboration_case();
        assert!(
            case.correct,
            "team gate failed: {:?}",
            case.facts.iter().filter(|f| !f.correct).collect::<Vec<_>>()
        );
    }
}
