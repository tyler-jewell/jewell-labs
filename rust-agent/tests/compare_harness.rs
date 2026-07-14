//! Multi-harness compare: dry gold + capability skip honesty (shipped entry path).

use rust_agent::{run_compare, CompareOpts};
use std::collections::BTreeMap;

#[test]
fn dry_all_tasks_pass_via_run_compare() {
    let report = run_compare(&CompareOpts {
        harnesses: vec!["dry".into()],
        tasks_filter: "all".into(),
        run_id: Some(format!(
            "compare-test-dry-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        )),
        ..Default::default()
    })
    .expect("dry compare");

    assert_eq!(report.kind, "compare");
    assert!(!report.items.is_empty());
    for it in &report.items {
        assert_eq!(it.status, "pass", "{} detail={}", it.task_id, it.detail);
        assert!((it.score - 1.0).abs() < 1e-9, "{} score={}", it.task_id, it.score);
    }
    let dry = report.summary.harnesses.get("dry").expect("dry summary");
    assert!((dry.avg_score - 1.0).abs() < 1e-9);
    assert!(dry.solid_base <= dry.avg_score + 1e-9);
    assert_eq!(dry.n_skip, 0);
}

#[test]
fn jewell_attempts_coding_task_not_capability_skip() {
    // Jewell now provides write_file/terminal/coding via agent fs_* tools.
    // Legacy coding tasks may still fail grading, but must not capability-skip.
    let report = run_compare(&CompareOpts {
        harnesses: vec!["jewell".into()],
        tasks_filter: "coding_fix_bug".into(),
        run_id: Some(format!(
            "compare-test-jewell-attempt-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        )),
        ..Default::default()
    })
    .expect("jewell coding");

    assert_eq!(report.items.len(), 1);
    let it = &report.items[0];
    assert_ne!(
        it.status, "skip",
        "must attempt coding tasks (got skip): detail={} caps={:?}",
        it.detail, it.capabilities_missing
    );
    assert!(
        it.capabilities_missing.is_empty(),
        "unexpected capability skip: {:?}",
        it.capabilities_missing
    );
    assert!(
        matches!(it.status.as_str(), "pass" | "fail" | "error"),
        "status={}",
        it.status
    );
}

#[test]
fn hermes_skips_host_eval_capability() {
    let report = run_compare(&CompareOpts {
        harnesses: vec!["hermes".into()],
        tasks_filter: "agent_os_introspection".into(),
        run_id: Some(format!(
            "compare-test-hermes-skip-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        )),
        ..Default::default()
    })
    .expect("hermes host");

    assert_eq!(report.items.len(), 1);
    let it = &report.items[0];
    assert_eq!(it.status, "skip");
    assert_eq!(it.score, 0.0);
    assert!(
        it.detail.contains("host_eval") || it.capabilities_missing.iter().any(|c| c == "host_eval"),
        "detail={} caps={:?}",
        it.detail,
        it.capabilities_missing
    );
}

#[test]
fn solid_base_never_exceeds_avg() {
    use rust_agent::eval::compare::{solid_base, summarize_items, CompareItem};

    let items = vec![
        CompareItem {
            harness: "h".into(),
            task_id: "t".into(),
            track: "coding".into(),
            run_index: 1,
            status: "pass".into(),
            correct: true,
            score: 1.0,
            t_ms: 1,
            detail: String::new(),
            metrics: BTreeMap::new(),
            workspace: String::new(),
            capabilities_missing: vec![],
        },
        CompareItem {
            harness: "h".into(),
            task_id: "t".into(),
            track: "coding".into(),
            run_index: 2,
            status: "fail".into(),
            correct: false,
            score: 0.0,
            t_ms: 1,
            detail: String::new(),
            metrics: BTreeMap::new(),
            workspace: String::new(),
            capabilities_missing: vec![],
        },
    ];
    assert_eq!(solid_base(&[1.0, 0.0]), 0.0);
    let s = summarize_items(&items);
    let h = s.harnesses.get("h").unwrap();
    assert!(h.solid_base <= h.avg_score + 1e-9);
}
