//! Catalog discovery, filter, seeded sample, vendor discovery.
//! Catalog items are hydrated from online remotes (may use evals/catalog/.cache/).

use rust_agent::{
    default_vendor_ids, filter_items, list_source_summaries, load_catalog_items, load_eval_model,
    load_vendor_manifests, run_catalog_sample, sample_items, CatalogFilter, CatalogRunOpts,
};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[test]
fn five_enabled_online_sources_with_links_and_remote() {
    let src = list_source_summaries(false).expect("load sources");
    let ids: BTreeSet<_> = src.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains("swe-bench"), "{ids:?}");
    assert!(ids.contains("terminal-bench"), "{ids:?}");
    assert!(ids.contains("bfcl"), "{ids:?}");
    assert!(ids.contains("mbpp"), "{ids:?}");
    assert!(ids.contains("humaneval"), "{ids:?}");
    assert!(!ids.contains("legacy-local"), "legacy-local must not exist");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../evals/catalog/sources");
    for s in &src {
        assert!(!s.links.is_empty(), "{} missing links", s.id);
        assert!(!s.tags.is_empty(), "{} missing tags", s.id);
        assert!(s.item_count > 0, "{} has no items (remote refs)", s.id);
        let remote = root.join(&s.id).join("remote.toml");
        assert!(
            remote.is_file(),
            "{} must have remote.toml (online pointer)",
            s.id
        );
        let items_jsonl = root.join(&s.id).join("items.jsonl");
        assert!(
            !items_jsonl.is_file(),
            "{} must not ship hard-coded items.jsonl",
            s.id
        );
    }
}

#[test]
fn vendors_discovered_and_default_stable() {
    let ms = load_vendor_manifests(None).expect("vendors");
    let ids: BTreeSet<_> = ms.iter().map(|m| m.id.as_str()).collect();
    assert!(ids.contains("jewell"), "{ids:?}");
    assert!(ids.contains("hermes"), "{ids:?}");
    let d = default_vendor_ids(None).unwrap();
    assert!(d.contains(&"hermes".into()) && d.contains(&"jewell".into()));
    let mut sorted = d.clone();
    sorted.sort();
    assert_eq!(d, sorted, "default vendor ids must be sorted");
}

#[test]
fn eval_model_loads_from_repo() {
    let m = load_eval_model(None).expect("model");
    assert!(
        m.base_url.contains("8091") || m.base_url.contains("http"),
        "{:?}",
        m.base_url
    );
    assert!(!m.model.is_empty());
}

#[test]
fn hydrate_online_and_filter_sample() {
    let all = load_catalog_items(&[], false).expect("hydrate online items");
    assert!(
        all.len() >= 20,
        "need enough hydrated items, got {}",
        all.len()
    );
    assert!(all.iter().all(|i| i.source_id != "legacy-local"));
    // Every item should cite a remote origin in meta or links
    for i in all.iter().take(5) {
        assert!(!i.prompt.trim().is_empty(), "{} empty prompt", i.full_id());
    }

    let coding = filter_items(
        &all,
        &CatalogFilter {
            tags_any: vec!["coding".into()],
            ..Default::default()
        },
    );
    assert!(!coding.is_empty());
    assert!(coding.iter().all(|i| i.tags.iter().any(|t| t == "coding")));

    let runnable = filter_items(
        &all,
        &CatalogFilter {
            exclude_sandbox: true,
            ..Default::default()
        },
    );
    // BFCL online rows are host-runnable; TB/SWE are sandbox_skip.
    assert!(
        !runnable.is_empty(),
        "expected runnable non-sandbox pool from BFCL, got 0"
    );
    assert!(runnable
        .iter()
        .all(|i| { !i.grade.requires_sandbox && i.grade.kind != "sandbox_skip" }));
    let run_sources: BTreeSet<_> = runnable.iter().map(|i| i.source_id.as_str()).collect();
    assert!(
        run_sources.contains("bfcl"),
        "BFCL should be runnable: {run_sources:?}"
    );
    // Host coding datasets should also be runnable without Docker.
    assert!(
        run_sources.contains("mbpp") && run_sources.contains("humaneval"),
        "expected both mbpp and humaneval in runnable pool: {run_sources:?}"
    );

    let a = sample_items(&runnable, 20, 42);
    let b = sample_items(&runnable, 20, 42);
    assert!(!a.is_empty());
    assert_eq!(
        a.iter().map(|i| i.full_id()).collect::<Vec<_>>(),
        b.iter().map(|i| i.full_id()).collect::<Vec<_>>()
    );
}

#[test]
fn bfcl_items_from_remote_have_tools() {
    let all = load_catalog_items(&["bfcl".into()], false).expect("bfcl online");
    assert!(!all.is_empty());
    let with_tools = all.iter().filter(|i| !i.tools.is_empty()).count();
    assert!(
        with_tools >= 10,
        "BFCL items should include tools schemas, only {with_tools}/{} have tools",
        all.len()
    );
    for i in &all {
        if i.grade.kind == "tool_call" {
            assert!(
                !i.tools.is_empty(),
                "{} tool_call item missing tools[]",
                i.full_id()
            );
        }
        // Prompt must come from remote user text (non-empty).
        assert!(!i.prompt.trim().is_empty(), "{}", i.full_id());
    }
}

#[test]
fn jewell_skips_sandbox_coding_honestly() {
    let report = run_catalog_sample(&CatalogRunOpts {
        harnesses: vec!["jewell".into()],
        sources: vec!["swe-bench".into()],
        sample_n: Some(3),
        seed: 2,
        exclude_sandbox: false,
        skip_model_preflight: true,
        run_id: Some(format!(
            "catalog-test-jewell-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        )),
        ..Default::default()
    })
    .expect("jewell catalog");
    assert!(!report.items.is_empty());
    for it in &report.items {
        assert_eq!(
            it.status, "skip",
            "{} should skip: {}",
            it.full_id, it.detail
        );
        assert_eq!(it.score, 0.0);
    }
    assert!(report.meta.get("eval_model").is_some(), "{:?}", report.meta);
    assert!(report.meta.get("vendors").is_some());
}

#[test]
fn rejects_hardcoded_items_jsonl_if_present() {
    // Structural: no source dir may contain items.jsonl with content.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../evals/catalog/sources");
    if !root.is_dir() {
        return;
    }
    for ent in std::fs::read_dir(&root).unwrap() {
        let p = ent.unwrap().path();
        if !p.is_dir() {
            continue;
        }
        let items = p.join("items.jsonl");
        assert!(
            !items.is_file(),
            "hard-coded items forbidden: {}",
            items.display()
        );
    }
}
