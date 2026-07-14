//! Legacy local Harbor compare harness: synthetic tasks were removed.
//! Public evals live in online catalog (`eval_catalog`). These tests keep
//! pure scoring helpers and assert the local tasks dir no longer ships smokes.

use rust_agent::eval::compare::{solid_base, summarize_items, CompareItem};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[test]
fn local_compare_tasks_dir_has_no_hardcoded_evals() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../evals/harness_compare/tasks");
    if !root.is_dir() {
        return;
    }
    for ent in std::fs::read_dir(&root).unwrap() {
        let p = ent.unwrap().path();
        if p.is_dir() {
            // No task packs with instruction.md (hard-coded local evals).
            assert!(
                !p.join("instruction.md").is_file(),
                "hard-coded local eval task forbidden: {}",
                p.display()
            );
            assert!(
                !p.join("task.toml").is_file(),
                "hard-coded local eval task.toml forbidden: {}",
                p.display()
            );
        }
    }
}

#[test]
fn solid_base_never_exceeds_avg() {
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
