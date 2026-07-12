use rust_agent::{learn_improve, LearnRequest};
use std::fs;
use tempfile::tempdir;

fn sample(name: &str, body: &str) -> String {
    format!(
        "---\nschema_version: 1\nname: {name}\ndescription: d\ndefault_model: qwen3-0.6b\nrole: agent\ntools: [\"list_tools\"]\n---\n\n{body}\n"
    )
}

#[test]
fn dry_run_leaves_disk_unchanged() {
    let d = tempdir().unwrap();
    fs::create_dir_all(d.path().join("lab")).unwrap();
    let base = sample("drybot", "BASELINE MUST_KEEP");
    fs::write(d.path().join("lab/drybot.md"), &base).unwrap();
    let rep = learn_improve(
        d.path(),
        &LearnRequest {
            targets: vec!["lab/drybot".into()],
            patches: vec![sample("drybot", "BASELINE MUST_KEEP\nextra")],
            dry_run: true,
            max_diff_lines: 40,
            require_substrings: vec!["MUST_KEEP".into()],
        },
    )
    .unwrap();
    assert_eq!(rep.results[0].verdict, "would_keep");
    assert!(!rep.results[0].disk_changed);
    assert_eq!(fs::read_to_string(d.path().join("lab/drybot.md")).unwrap(), base);
}

#[test]
fn bad_patch_reverts_good_keeps() {
    let d = tempdir().unwrap();
    fs::create_dir_all(d.path().join("lab")).unwrap();
    let base = sample("keepbot", "MUST_A MUST_B");
    fs::write(d.path().join("lab/keepbot.md"), &base).unwrap();
    let req = |patch: String| LearnRequest {
        targets: vec!["lab/keepbot".into()],
        patches: vec![patch],
        dry_run: false,
        max_diff_lines: 40,
        require_substrings: vec!["MUST_A".into(), "MUST_B".into()],
    };
    assert_eq!(
        learn_improve(d.path(), &req(sample("keepbot", "nothing")))
            .unwrap()
            .results[0]
            .verdict,
        "revert"
    );
    assert_eq!(
        learn_improve(
            d.path(),
            &req(sample("keepbot", "MUST_A MUST_B\nextra guidance"))
        )
        .unwrap()
        .results[0]
        .verdict,
        "keep"
    );
}

#[test]
fn max_diff_and_core_lock() {
    let d = tempdir().unwrap();
    fs::create_dir_all(d.path().join("lab")).unwrap();
    fs::write(d.path().join("lab/diffbot.md"), sample("diffbot", "x")).unwrap();
    let big = sample("diffbot", &(0..50).map(|i| format!("l{i}\n")).collect::<String>());
    assert_eq!(
        learn_improve(
            d.path(),
            &LearnRequest {
                targets: vec!["lab/diffbot".into()],
                patches: vec![big],
                dry_run: false,
                max_diff_lines: 5,
                require_substrings: vec![],
            },
        )
        .unwrap()
        .results[0]
        .verdict,
        "rejected"
    );
    assert_eq!(
        learn_improve(
            d.path(),
            &LearnRequest {
                targets: vec!["core/orchestrator".into()],
                patches: vec![sample("orchestrator", "x")],
                dry_run: false,
                max_diff_lines: 40,
                require_substrings: vec![],
            },
        )
        .unwrap()
        .results[0]
        .verdict,
        "rejected"
    );
}
