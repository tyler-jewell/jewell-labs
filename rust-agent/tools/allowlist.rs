//! Tool allowlist resolution and validation.

use super::invoke::builtin_tools;
use super::types::ToolSpec;

/// Resolve allowlist patterns (`*`, `name`, `category/*`) to concrete tool names.
pub fn resolve_allowlist(patterns: &[String]) -> Vec<String> {
    let specs = builtin_tools();
    if patterns.iter().any(|p| p == "*") {
        return specs.into_iter().map(|t| t.name).collect();
    }
    let mut out = Vec::new();
    for t in &specs {
        if tool_matches_allowlist(patterns, &t.name, &t.category) {
            out.push(t.name.clone());
        }
    }
    out
}

pub fn tool_matches_allowlist(patterns: &[String], name: &str, category: &str) -> bool {
    patterns.iter().any(|p| {
        p == "*"
            || p == name
            || p == &format!("{category}/*")
            || p == &format!("{category}/{name}")
    })
}

pub fn filter_tools_for_agent(patterns: &[String]) -> Vec<ToolSpec> {
    builtin_tools()
        .into_iter()
        .filter(|t| tool_matches_allowlist(patterns, &t.name, &t.category))
        .collect()
}

/// Validate frontmatter tool entries against the registry.
pub fn validate_tool_allowlist(patterns: &[String]) -> Vec<String> {
    let mut errors = Vec::new();
    let specs = builtin_tools();
    let names: Vec<_> = specs.iter().map(|t| t.name.as_str()).collect();
    let cats: Vec<_> = specs.iter().map(|t| t.category.as_str()).collect();

    for p in patterns {
        if p == "*" {
            continue;
        }
        if let Some(cat) = p.strip_suffix("/*") {
            if !cats.iter().any(|c| *c == cat) {
                errors.push(format!("unknown tool category in tools: {p}"));
            }
            continue;
        }
        if p.contains('/') {
            let mut parts = p.splitn(2, '/');
            let cat = parts.next().unwrap_or("");
            let name = parts.next().unwrap_or("");
            if !specs
                .iter()
                .any(|t| t.category == cat && t.name == name)
            {
                errors.push(format!("unknown tool in tools: {p}"));
            }
            continue;
        }
        if !names.iter().any(|n| *n == p.as_str()) {
            errors.push(format!("unknown tool in tools: {p}"));
        }
    }
    errors
}
