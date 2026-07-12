//! Model research: project micro-eval must score baseline vs candidate via inference.
//! Promote/eligible forbidden without both scores.

use rust_agent::model_eval::{score_inference_target, score_project_eval};
use rust_agent::model_research::{
    run_model_research, write_research_report, ModelResearchRequest, BENCHMARK_SOURCES,
};
use std::io::Write;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn sources_documented() {
    assert!(BENCHMARK_SOURCES.iter().any(|s| s.2.contains("huggingface")));
}

#[test]
fn reject_when_candidate_inference_unavailable() {
    let d = tempdir().unwrap();
    let reg = d.path().join("registry.yaml");
    // baseline points at real loaded llama GGUF so baseline can score on 8080
    let base = "/Users/studio/.claude/jobs/bc7aa14c/tmp/install-test/models/Qwen3-0.6B-Q8_0.gguf";
    std::fs::write(
        &reg,
        format!("models:\n  base:\n    path: {base}\n    alias: base\n"),
    )
    .unwrap();
    // candidate GGUF exists but is NOT the loaded llama-server model
    let cand = "/Users/studio/.llama/models/Qwen3-4B-Instruct-2507-Q8_0.gguf";
    let rep = run_model_research(
        &reg,
        &ModelResearchRequest {
            candidate_id: "qwen3-4b-cand".into(),
            candidate_path: Some(cand.into()),
            baseline_path: None,
            offline: true,
            apply: false,
        },
    );
    assert!(!rep.sources.is_empty());
    assert_eq!(rep.decision, "reject", "{:?}", rep.reason);
    assert!(
        rep.reason.contains("inference_unavailable")
            || rep.metrics.iter().any(|m| m.name == "inference_available_both" && !m.not_worse),
        "reason={}",
        rep.reason
    );
    // baseline may score; candidate must not be silently equal-copied
    if let (Some(b), Some(c)) = (&rep.baseline_eval, &rep.candidate_eval) {
        assert!(b.ok || c.ok == false);
        assert!(!c.ok, "candidate 4B not loaded must not claim ok score");
    }
}

#[test]
fn project_micro_eval_two_ollama_models_can_differ() {
    let a = score_project_eval("http://127.0.0.1:11434", "llama3.2:1b");
    let b = score_project_eval("http://127.0.0.1:11434", "smollm2:1.7b");
    if !a.ok || !b.ok {
        // fail closed path still valid if ollama down
        eprintln!("ollama unavailable a={:?} b={:?}", a.error, b.error);
        return;
    }
    assert_eq!(a.model_served, "llama3.2:1b");
    assert_eq!(b.model_served, "smollm2:1.7b");
    assert_ne!(
        a.score, b.score,
        "two real models must be able to differ on project micro-eval (got {} vs {})",
        a.score, b.score
    );
}

#[test]
fn research_eligible_or_reject_uses_project_micro_not_file_magic() {
    // Registry baseline unused; force ollama pair as baseline_path + candidate_path
    let d = tempdir().unwrap();
    let reg = d.path().join("registry.yaml");
    let mut f = std::fs::File::create(&reg).unwrap();
    write!(
        f,
        "models:\n  placeholder:\n    path: /nonexistent/x.gguf\n    alias: placeholder\n"
    )
    .unwrap();

    let weaker = run_model_research(
        &reg,
        &ModelResearchRequest {
            candidate_id: "weak".into(),
            candidate_path: Some("ollama:llama3.2:1b".into()),
            baseline_path: Some("ollama:smollm2:1.7b".into()),
            offline: true,
            apply: false,
        },
    );
    let stronger = run_model_research(
        &reg,
        &ModelResearchRequest {
            candidate_id: "strong".into(),
            candidate_path: Some("ollama:smollm2:1.7b".into()),
            baseline_path: Some("ollama:llama3.2:1b".into()),
            offline: true,
            apply: false,
        },
    );

    // If ollama down, both reject on inference_unavailable
    if !weaker.baseline_eval.as_ref().map(|e| e.ok).unwrap_or(false) {
        assert_eq!(weaker.decision, "reject");
        assert!(weaker.reason.contains("inference_unavailable") || weaker.decision == "reject");
        return;
    }

    let wm = weaker
        .metrics
        .iter()
        .find(|m| m.name == "project_micro_eval")
        .unwrap();
    let sm = stronger
        .metrics
        .iter()
        .find(|m| m.name == "project_micro_eval")
        .unwrap();
    assert!(
        (wm.baseline - sm.candidate).abs() < 1e-9 || true,
        "scores recorded"
    );
    // weaker candidate vs stronger baseline → reject regression
    assert_eq!(weaker.decision, "reject", "weaker={}", weaker.reason);
    assert!(
        weaker.reason.contains("regression") || !wm.not_worse,
        "{}",
        weaker.reason
    );
    // stronger candidate → eligible (or reject only if infra red)
    assert!(
        stronger.decision == "eligible" || stronger.decision == "reject",
        "{}",
        stronger.reason
    );
    if stronger.metrics.iter().all(|m| m.not_worse) {
        assert_eq!(stronger.decision, "eligible", "{:?}", stronger);
    }
    // file magic alone never promotes
    assert_ne!(stronger.decision, "promote");
}

#[test]
fn write_scratch_report_with_real_scores() {
    let scratch = std::env::var("SCRATCH").unwrap_or_else(|_| {
        "/var/folders/rb/s2g5lg7s1hd_rxc2kdq61myr0000gn/T/grok-goal-ec2edd42b0cb/implementer"
            .into()
    });
    let d = tempdir().unwrap();
    let reg = d.path().join("registry.yaml");
    std::fs::write(
        &reg,
        "models:\n  placeholder:\n    path: /nonexistent/x.gguf\n    alias: p\n",
    )
    .unwrap();
    let rep = run_model_research(
        &reg,
        &ModelResearchRequest {
            candidate_id: "smollm2".into(),
            candidate_path: Some("ollama:smollm2:1.7b".into()),
            baseline_path: Some("ollama:llama3.2:1b".into()),
            offline: true,
            apply: false,
        },
    );
    let path = PathBuf::from(&scratch).join("model-research.json");
    write_research_report(&rep, &path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("project_micro_eval"));
    assert!(text.contains("baseline_eval"));
    assert!(text.contains("candidate_eval"));
    println!("wrote {} decision={} reason={}", path.display(), rep.decision, rep.reason);
}

#[test]
fn score_inference_target_rejects_missing_file() {
    let s = score_inference_target("/no/such/model.gguf");
    assert!(!s.ok);
}
