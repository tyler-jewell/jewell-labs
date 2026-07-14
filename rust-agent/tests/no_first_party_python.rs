//! Structural gate: Jewell agent/evals must not ship first-party Python sources.
//!
//! Allowed exception: hermes vendor under `evals/harness_compare/vendor/` (gitignored
//! third-party install). Subject workspaces may still contain `solution.py` as
//! graded artifacts; those are not tracked first-party harness modules.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Paths that must never contain tracked first-party `.py` (relative to monorepo root).
const FORBIDDEN_PREFIXES: &[&str] = &[
    "tools/",
    "rust-agent/",
    "evals/catalog/",
    "evals/datasets/",
    "evals/vendors/",
    "evals/harness_compare/lib/",
    "evals/harness_compare/run_compare.py",
    "evals/harness_compare/review_sample.py",
];

fn monorepo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("monorepo root")
}

fn is_hermes_vendor(rel: &str) -> bool {
    rel.starts_with("evals/harness_compare/vendor/")
}

fn is_forbidden_first_party(rel: &str) -> bool {
    if is_hermes_vendor(rel) {
        return false;
    }
    FORBIDDEN_PREFIXES.iter().any(|p| {
        if p.ends_with('/') {
            rel.starts_with(p)
        } else {
            rel == *p || rel.starts_with(&format!("{p}/"))
        }
    }) || {
        // Any tracked .py under harness_compare outside vendor is first-party.
        rel.starts_with("evals/harness_compare/") && rel.ends_with(".py") && !is_hermes_vendor(rel)
    }
}

/// Drive the real gate: `git ls-files '*.py'` from monorepo root.
#[test]
fn no_first_party_python_sources_tracked() {
    let root = monorepo_root();
    let out = Command::new("git")
        .args(["ls-files", "*.py", "*.pyi", "*.pyx"])
        .current_dir(&root)
        .output()
        .expect("git ls-files");
    assert!(
        out.status.success(),
        "git ls-files failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    let mut offenders = Vec::new();
    for line in text.lines() {
        let rel = line.trim();
        if rel.is_empty() {
            continue;
        }
        if is_forbidden_first_party(rel) {
            offenders.push(rel.to_string());
        } else if !is_hermes_vendor(rel) {
            // Any non-hermes tracked Python is also forbidden (future paths).
            offenders.push(rel.to_string());
        }
    }
    assert!(
        offenders.is_empty(),
        "first-party Python is forbidden in Jewell agent/evals (hermes vendor only). \
         Offenders:\n  {}\nSee AGENTS.md § No Python in Jewell agent or Jewell evals.",
        offenders.join("\n  ")
    );
}

/// Disk scan of known first-party trees (catches untracked local .py left behind).
#[test]
fn no_first_party_python_on_disk_in_core_trees() {
    let root = monorepo_root();
    let mut offenders = Vec::new();
    for rel in [
        "tools",
        "rust-agent",
        "evals/catalog",
        "evals/datasets",
        "evals/vendors",
        "evals/harness_compare",
    ] {
        let dir = root.join(rel);
        if !dir.is_dir() {
            continue;
        }
        walk_py(&dir, &root, &mut offenders);
    }
    assert!(
        offenders.is_empty(),
        "first-party Python files found on disk (hermes vendor excluded):\n  {}",
        offenders.join("\n  ")
    );
}

fn walk_py(dir: &Path, root: &Path, offenders: &mut Vec<String>) {
    let rd = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    for ent in rd.flatten() {
        let p = ent.path();
        let name = ent.file_name();
        let name = name.to_string_lossy();
        if name == "vendor" && p.parent().is_some_and(|par| par.ends_with("harness_compare")) {
            // Do not descend into hermes vendor tree.
            continue;
        }
        if name == "target" || name == ".cache" || name == "__pycache__" || name == "runs" {
            continue;
        }
        if p.is_dir() {
            walk_py(&p, root, offenders);
            continue;
        }
        if p.extension().and_then(|e| e.to_str()) == Some("py") {
            let rel = p
                .strip_prefix(root)
                .map(|r| r.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| p.display().to_string());
            if !is_hermes_vendor(&rel) {
                offenders.push(rel);
            }
        }
    }
}

#[test]
fn agents_md_bans_first_party_python() {
    let root = monorepo_root();
    let text = std::fs::read_to_string(root.join("AGENTS.md")).expect("AGENTS.md");
    let lower = text.to_lowercase();
    assert!(
        lower.contains("no python in jewell")
            || lower.contains("do **not** add, restore, or introduce first-party **python**")
            || lower.contains("do not introduce first-party python"),
        "AGENTS.md must state an explicit ban on first-party Python in Jewell agent/evals"
    );
    assert!(
        lower.contains("hermes") && (lower.contains("exception") || lower.contains("vendor")),
        "AGENTS.md ban must allow hermes vendor as the exception"
    );
    // Canonical runbook must not teach reintroducing first-party python3 llama-eval.
    assert!(
        !text.contains("python3 llama-eval.py"),
        "AGENTS.md must not prescribe python3 llama-eval.py as the default batch path"
    );
    assert!(
        !text.contains("python3 run_compare.py"),
        "AGENTS.md must not prescribe python3 run_compare.py"
    );
}
