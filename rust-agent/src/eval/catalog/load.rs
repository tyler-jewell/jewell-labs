//! Discover sources under evals/catalog/sources/.

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

/// Tiny TOML subset for source.toml (same rules as compare task.toml).
fn parse_simple_toml(text: &str) -> std::collections::BTreeMap<String, String> {
    let mut m = std::collections::BTreeMap::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || !line.contains('=') {
            continue;
        }
        let (k, v) = line.split_once('=').unwrap();
        m.insert(k.trim().to_string(), v.trim().to_string());
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

pub fn load_items(source: &SourceMeta) -> std::io::Result<Vec<CatalogItem>> {
    let path = source.path.join("items.jsonl");
    if !path.is_file() {
        return Ok(vec![]);
    }
    let text = fs::read_to_string(path)?;
    let mut out = Vec::new();
    for (lineno, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut item: CatalogItem = serde_json::from_str(line).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("{}:{}: {e}", source.id, lineno + 1),
            )
        })?;
        if item.source_id.is_empty() {
            item.source_id = source.id.clone();
        }
        // inherit source tags if item tags empty
        if item.tags.is_empty() {
            item.tags = source.tags.clone();
        }
        out.push(item);
    }
    Ok(out)
}

/// Load all sources (enabled and disabled).
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
        let items = load_items(&meta)?;
        meta.item_count = items.len();
        sources.push(meta);
    }
    Ok(sources)
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

/// Load all items from enabled sources (or filtered source ids).
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
