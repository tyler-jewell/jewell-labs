use rust_agent::eval::{parse_dataset, run_agent_eval};
use rust_agent::{CORE_AGENT_ID, LEARNER_ID};

#[test]
fn parse_requires_nonempty_require_tools() {
    let bad = "## case: x\ntrack: tool_plan\nprompt: p\nrequire_tools: []\n";
    assert!(parse_dataset(bad).is_err());
}

#[test]
fn core_and_learner_tool_plan_green() {
    let r = run_agent_eval(CORE_AGENT_ID, "tool_plan").expect("core eval");
    assert!(r.summary.tool_plan_accuracy >= 1.0, "core {:?}", r.summary);
    let r = run_agent_eval(LEARNER_ID, "tool_plan").expect("learner");
    assert!(
        r.summary.tool_plan_accuracy >= 1.0,
        "learner {:?}",
        r.summary
    );
}
