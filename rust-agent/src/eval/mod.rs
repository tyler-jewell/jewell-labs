//! Core agent eval: complete app introspection via built-in tools.
//!
//! Two tracks:
//! 1. **tool_plan** — orchestrator allowlist runs every introspect tool; fact-check 100%
//! 2. **llm_agent** — live model drives tool calls; fact-check tool results + answer

mod cases;
mod ground_truth;
mod llm_case;
mod runner;
mod scoring;

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
        assert_eq!(unique.len(), gt.tool_count);
    }
}
