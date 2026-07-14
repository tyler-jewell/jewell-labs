//! Filter catalog items by source / tags / track.

use super::types::{CatalogFilter, CatalogItem};

pub fn filter_items(items: &[CatalogItem], f: &CatalogFilter) -> Vec<CatalogItem> {
    items
        .iter()
        .filter(|it| matches_filter(it, f))
        .cloned()
        .collect()
}

pub fn matches_filter(it: &CatalogItem, f: &CatalogFilter) -> bool {
    if !f.sources.is_empty() && !f.sources.iter().any(|s| s == &it.source_id) {
        return false;
    }
    if !f.tracks.is_empty() && !f.tracks.iter().any(|t| t == &it.track) {
        return false;
    }
    if !f.tags_all.is_empty() && !f.tags_all.iter().all(|t| it.tags.iter().any(|x| x == t)) {
        return false;
    }
    if !f.tags_any.is_empty() && !f.tags_any.iter().any(|t| it.tags.iter().any(|x| x == t)) {
        return false;
    }
    if f.exclude_sandbox && (it.grade.requires_sandbox || it.grade.kind == "sandbox_skip") {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::catalog::types::GradeSpec;

    fn item(source: &str, id: &str, tags: &[&str], track: &str) -> CatalogItem {
        CatalogItem {
            id: id.into(),
            source_id: source.into(),
            tags: tags.iter().map(|s| (*s).into()).collect(),
            track: track.into(),
            capabilities: vec![],
            prompt: "p".into(),
            grade: GradeSpec {
                kind: "exact".into(),
                expected: Some("1".into()),
                expected_contains: None,
                tool_name: None,
                answer_file: None,
                dry_pass: true,
                skip_reason: None,
                requires_sandbox: false,
            },
            tools: vec![],
            workspace_files: Default::default(),
            links: vec![],
            meta: serde_json::Value::Null,
        }
    }

    #[test]
    fn filter_by_source_and_tag() {
        let items = vec![
            item("swe-bench", "a", &["coding", "autonomy"], "coding"),
            item("bfcl", "b", &["tool_call"], "tool_call"),
            item("terminal-bench", "c", &["terminal", "autonomy"], "terminal"),
        ];
        let f = CatalogFilter {
            sources: vec!["swe-bench".into()],
            ..Default::default()
        };
        assert_eq!(filter_items(&items, &f).len(), 1);
        let f2 = CatalogFilter {
            tags_any: vec!["tool_call".into()],
            ..Default::default()
        };
        assert_eq!(filter_items(&items, &f2)[0].id, "b");
        let f3 = CatalogFilter {
            tags_all: vec!["coding".into(), "autonomy".into()],
            ..Default::default()
        };
        assert_eq!(filter_items(&items, &f3).len(), 1);
    }
}
