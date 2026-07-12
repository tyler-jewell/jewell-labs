use rust_agent::model_research::{run_model_research, ModelResearchRequest, BENCHMARK_SOURCES};
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
}

#[test]
fn eligible_or_reject_with_evidence() {
    let d = tempdir().unwrap();
    let weights = d.path().join("cand.gguf");
    std::fs::write(&weights, b"gguf").unwrap();
    let base_w = d.path().join("base.gguf");
    std::fs::write(&base_w, b"gguf").unwrap();
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
            candidate_path: Some(weights.display().to_string()),
            offline: true,
            apply: false,
        },
    );
    assert!(!rep.sources.is_empty());
    assert!(rep.decision == "eligible" || rep.decision == "reject");
}
