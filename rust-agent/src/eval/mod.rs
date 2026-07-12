//! Agent evals: structural tool_plan (CI gate) + optional LLM track.
//!
//! Host-runnable for any agent via [`agent_eval::run_agent_eval`] with depth ≤ 1.

mod agent_eval;
mod cases;
mod ground_truth;
mod llm_case;
mod runner;
mod scoring;

pub use agent_eval::{
    dataset_path_for, eval_all_agents, parse_dataset, require_green_eval,
    run_agent_eval, write_eval_report, DatasetCase, EvalError,
};
pub use cases::{full_introspection_plan, orchestrator_ctx, run_tool_plan_case};
pub use ground_truth::{CaseResult, EvalReport, EvalSummary, FactResult, GroundTruth};
pub use llm_case::run_llm_case;
pub use runner::{run_full_eval, write_report};
pub use scoring::score_introspection;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_plan_complete_introspection() {
        let (ctx, doc) = orchestrator_ctx();
        assert_eq!(doc.id, crate::agents::CORE_AGENT_ID);
        assert_eq!(doc.frontmatter.role, "orchestrator");
        let gt = GroundTruth::collect(&ctx);
        assert!(gt.agent_ids.contains(crate::agents::CORE_AGENT_ID));
        assert_eq!(gt.tool_count, crate::agents::CORE_AGENT_TOOLS.len());
        let case = run_tool_plan_case(&ctx, &gt);
        assert!(
            case.correct,
            "tool plan must fully introspect; facts={:?}",
            case.facts.iter().filter(|f| !f.correct).collect::<Vec<_>>()
        );
        let unique: std::collections::BTreeSet<_> =
            case.tools_called.iter().cloned().collect();
        // run_eval intentionally not invoked in core self-plan (depth guard)
        assert_eq!(unique.len(), gt.tool_count - 1);
        assert!(unique.iter().all(|n| n != "run_eval"));
        assert!(gt.tool_names.contains("run_eval"));
    }
}
