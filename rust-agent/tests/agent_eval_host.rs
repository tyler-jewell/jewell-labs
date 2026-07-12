use rust_agent::eval::{parse_dataset, run_agent_eval};
use rust_agent::CORE_AGENT_ID;

#[test]
fn parse_rejects_empty_require() {
    assert!(parse_dataset("## case: bad\ntrack: tool_plan\nrequire_tools: []\n").is_err());
}

#[test]
fn parse_ok_and_run_core() {
    let c = parse_dataset("## case: s\ntrack: tool_plan\nrequire_tools: [list_tools]\n").unwrap();
    assert_eq!(c[0].require_tools, vec!["list_tools"]);
    let r = run_agent_eval(CORE_AGENT_ID, "tool_plan").expect("core eval");
    assert!(r.summary.tool_plan_accuracy >= 1.0, "{:?}", r.summary);
}

#[test]
fn specialty_eval_runs() {
    let r = run_agent_eval("tutoring/math-tutor", "tool_plan").expect("tutor");
    assert!(r.summary.total >= 1);
}
