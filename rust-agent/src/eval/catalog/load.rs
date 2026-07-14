//! Discover sources under evals/catalog/sources/ and hydrate items from online remotes.

use super::remote::{assert_no_hardcoded_items, load_remote_items};
use super::types::{CatalogItem, SourceMeta, SourceSummary};
use crate::paths::repo_root;
use std::fs;
use std::path::{Path, PathBuf};

pub fn catalog_root(repo: &Path) -> PathBuf {
    repo.join("evals").join("catalog")
}

pub fn sources_dir(repo: &Path) -> PathBuf {
    catalog_root(repo).join("sources")
}

/// Tiny TOML subset for source.toml, including multi-line string arrays.
fn parse_simple_toml(text: &str) -> std::collections::BTreeMap<String, String> {
    let mut m = std::collections::BTreeMap::new();
    let mut lines = text.lines().peekable();
    while let Some(raw) = lines.next() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || !line.contains('=') {
            continue;
        }
        let (k, v0) = line.split_once('=').unwrap();
        let key = k.trim().to_string();
        let mut v = v0.trim().to_string();
        if v.starts_with('[') && !v.contains(']') {
            let mut buf = v;
            while let Some(more) = lines.next() {
                buf.push(' ');
                buf.push_str(more.trim());
                if more.contains(']') {
                    break;
                }
            }
            v = buf;
        }
        m.insert(key, v);
    }
    m
}

fn parse_list_field(v: &str) -> Vec<String> {
    let v = v.trim();
    if v.starts_with('[') && v.ends_with(']') {
        v[1..v.len() - 1]
            .split(',')
            .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        vec![v.trim_matches('"').to_string()]
    }
}

fn parse_bool(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes")
}

pub fn load_source_meta(dir: &Path) -> std::io::Result<SourceMeta> {
    let toml_path = dir.join("source.toml");
    let text = fs::read_to_string(&toml_path)?;
    let m = parse_simple_toml(&text);
    let id = m
        .get("id")
        .cloned()
        .unwrap_or_else(|| {
            dir.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string()
        })
        .trim_matches('"')
        .to_string();
    let name = m
        .get("name")
        .map(|s| s.trim_matches('"').to_string())
        .unwrap_or_else(|| id.clone());
    let description = m
        .get("description")
        .map(|s| s.trim_matches('"').to_string())
        .unwrap_or_default();
    let links = m
        .get("links")
        .map(|s| parse_list_field(s))
        .unwrap_or_default();
    let license = m
        .get("license")
        .map(|s| s.trim_matches('"').to_string())
        .unwrap_or_default();
    let version = m
        .get("version")
        .map(|s| s.trim_matches('"').to_string())
        .unwrap_or_default();
    let tags = m
        .get("tags")
        .map(|s| parse_list_field(s))
        .unwrap_or_default();
    let enabled = m
        .get("enabled")
        .map(|s| parse_bool(s))
        .unwrap_or(true);
    let notes = m
        .get("notes")
        .map(|s| s.trim_matches('"').to_string())
        .unwrap_or_default();
    Ok(SourceMeta {
        id,
        name,
        description,
        links,
        license,
        version,
        tags,
        enabled,
        notes,
        item_count: 0,
        path: dir.to_path_buf(),
    })
}

/// Hydrate catalog items from the source's online remote (remote.toml).
/// Hard-coded `items.jsonl` task bodies are rejected.
pub fn load_items(source: &SourceMeta) -> std::io::Result<Vec<CatalogItem>> {
    assert_no_hardcoded_items(&source.path).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e)
    })?;
    if !source.path.join("remote.toml").is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "source {}: remote.toml required (online dataset pointer only)",
                source.id
            ),
        ));
    }
    load_remote_items(source).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
}

/// Load all sources (enabled and disabled). Item counts use remote.toml list sizes when
/// network hydrate is not yet run (cheap discovery); prefer load_items for full bodies.
pub fn load_all_sources(repo: Option<&Path>) -> std::io::Result<Vec<SourceMeta>> {
    let root = repo.map(Path::to_path_buf).unwrap_or_else(repo_root);
    let dir = sources_dir(&root);
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    let mut entries: Vec<_> = fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.join("source.toml").is_file())
        .collect();
    entries.sort();
    let mut sources = Vec::new();
    for p in entries {
        let mut meta = load_source_meta(&p)?;
        // Prefer cheap remote ref count for discovery; fall back to hydrate length.
        meta.item_count = remote_ref_count(&meta).unwrap_or(0);
        if meta.item_count == 0 {
            // try hydrate once for accurate count (may use cache)
            if let Ok(items) = load_items(&meta) {
                meta.item_count = items.len();
            }
        }
        sources.push(meta);
    }
    Ok(sources)
}

fn remote_ref_count(source: &SourceMeta) -> Option<usize> {
    let text = fs::read_to_string(source.path.join("remote.toml")).ok()?;
    let m = parse_simple_toml(&text);
    let kind = m.get("remote_kind")?.trim_matches('"');
    match kind {
        "bfcl_github" => {
            let files = m.get("files").map(|s| parse_list_field(s))?;
            let max_per = m
                .get("max_per_file")
                .and_then(|s| s.trim().trim_matches('"').parse().ok())
                .unwrap_or(20usize);
            // upper bound until hydrated
            Some(files.len().saturating_mul(max_per))
        }
        "terminal_bench_github" => {
            let tasks = m.get("tasks").map(|s| parse_list_field(s))?;
            Some(tasks.len())
        }
        "swe_bench_github_pr" => {
            let ids = m.get("instance_ids").map(|s| parse_list_field(s))?;
            Some(ids.len())
        }
        _ => None,
    }
}

pub fn list_source_summaries(include_disabled: bool) -> std::io::Result<Vec<SourceSummary>> {
    Ok(load_all_sources(None)?
        .into_iter()
        .filter(|s| include_disabled || s.enabled)
        .map(|s| SourceSummary {
            id: s.id,
            name: s.name,
            links: s.links,
            tags: s.tags,
            version: s.version,
            license: s.license,
            enabled: s.enabled,
            item_count: s.item_count,
            notes: s.notes,
        })
        .collect())
}

/// Load all items from enabled sources (or filtered source ids) via online remotes.
pub fn load_catalog_items(
    source_ids: &[String],
    include_disabled: bool,
) -> std::io::Result<Vec<CatalogItem>> {
    let sources = load_all_sources(None)?;
    let mut items = Vec::new();
    for s in sources {
        if !include_disabled && !s.enabled {
            continue;
        }
        if !source_ids.is_empty() && !source_ids.iter().any(|id| id == &s.id) {
            continue;
        }
        items.extend(load_items(&s)?);
    }
    Ok(items)
}
