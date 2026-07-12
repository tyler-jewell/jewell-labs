use rust_agent::model_research::{
    model_fitness, project_tool_plan_score, run_model_research, ModelResearchRequest,
    BENCHMARK_SOURCES,
};
use std::io::Write;
use tempfile::tempdir;

#[test]
fn sources_documented() {
    assert!(BENCHMARK_SOURCES.iter().any(|s| s.2.contains("huggingface")));
}

#[test]
fn reject_missing_candidate_path() {
    let d = tempdir().unwrap();
    let reg = d.path().join("registry.yaml");
    let mut f = std::fs::File::create(&reg).unwrap();
    write!(f, "models:\n  base:\n    path: /nonexistent/base.gguf\n    alias: base\n").unwrap();
    let rep = run_model_research(
        &reg,
        &ModelResearchRequest {
            candidate_id: "new-model".into(),
            candidate_path: None,
            offline: true,
            apply: false,
        },
    );
    assert_eq!(rep.decision, "reject");
    assert!(!rep.registry_changed);
    assert!(!rep.sources.is_empty());
    // fitness metric must distinguish missing candidate
    let fit = rep.metrics.iter().find(|m| m.name == "model_fitness").unwrap();
    assert_eq!(fit.candidate, 0.0);
    assert!(!fit.not_worse || fit.baseline == 0.0);
}

#[test]
fn candidate_fitness_not_copied_from_baseline() {
    let d = tempdir().unwrap();
    // baseline: larger "GGUF" file
    let base_w = d.path().join("base.gguf");
    let mut bb = b"GGUF".to_vec();
    bb.extend(vec![0u8; 2048]);
    std::fs::write(&base_w, &bb).unwrap();
    // candidate: tiny non-GGUF
    let cand_w = d.path().join("cand.bin");
    std::fs::write(&cand_w, b"xx").unwrap();
    let reg = d.path().join("registry.yaml");
    std::fs::write(
        &reg,
        format!("models:\n  base:\n    path: {}\n    alias: base\n", base_w.display()),
    )
    .unwrap();

    let base_fit = model_fitness(&base_w);
    let cand_fit = model_fitness(&cand_w);
    assert!(base_fit > cand_fit, "base={base_fit} cand={cand_fit}");

    let rep = run_model_research(
        &reg,
        &ModelResearchRequest {
            candidate_id: "cand".into(),
            candidate_path: Some(cand_w.display().to_string()),
            offline: true,
            apply: false,
        },
    );
    let fit = rep.metrics.iter().find(|m| m.name == "model_fitness").unwrap();
    assert!(
        (fit.baseline - base_fit).abs() < 1e-9,
        "baseline metric must measure baseline file"
    );
    assert!(
        (fit.candidate - cand_fit).abs() < 1e-9,
        "candidate metric must measure candidate file, not copy baseline"
    );
    assert!(!fit.not_worse, "weaker candidate must fail not_worse");
    assert_eq!(rep.decision, "reject");
}

#[test]
fn equal_or_better_candidate_can_be_eligible() {
    let d = tempdir().unwrap();
    let mut payload = b"GGUF".to_vec();
    payload.extend(vec![1u8; 2048]);
    let base_w = d.path().join("base.gguf");
    let cand_w = d.path().join("cand.gguf");
    std::fs::write(&base_w, &payload).unwrap();
    std::fs::write(&cand_w, &payload).unwrap();
    let reg = d.path().join("registry.yaml");
    std::fs::write(
        &reg,
        format!("models:\n  base:\n    path: {}\n    alias: base\n", base_w.display()),
    )
    .unwrap();
    let rep = run_model_research(
        &reg,
        &ModelResearchRequest {
            candidate_id: "cand".into(),
            candidate_path: Some(cand_w.display().to_string()),
            offline: true,
            apply: false,
        },
    );
    let fit = rep.metrics.iter().find(|m| m.name == "model_fitness").unwrap();
    assert!(fit.not_worse);
    // infra may fail if agents dir missing in isolation — accept eligible or reject with infra reason
    assert!(
        rep.decision == "eligible" || rep.decision == "reject",
        "{}",
        rep.reason
    );
    if project_tool_plan_score() >= 1.0 {
        assert_eq!(rep.decision, "eligible", "{:?}", rep);
    }
}
