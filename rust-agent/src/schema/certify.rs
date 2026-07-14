//! Frontmatter parsing and certification.

use super::types::{AgentFrontmatter, CertificationResult, SchemaError, LATEST_SCHEMA_VERSION};
use std::collections::BTreeMap;

/// Split markdown into (yaml_text, body).
pub fn split_frontmatter(markdown: &str) -> Result<(&str, &str), SchemaError> {
    let text = markdown.trim_start_matches('\u{feff}');
    if !text.starts_with("---") {
        return Err(SchemaError::MissingFrontmatter);
    }
    let rest = &text[3..];
    let rest = rest.strip_prefix('\n').unwrap_or(rest);
    if let Some(idx) = rest.find("\n---") {
        let yaml = &rest[..idx];
        let after = &rest[idx + 4..];
        let body = after.strip_prefix('\n').unwrap_or(after);
        Ok((yaml, body))
    } else {
        Err(SchemaError::MissingFrontmatter)
    }
}

/// Parse frontmatter only (no certification).
pub fn parse_frontmatter(markdown: &str) -> Result<(AgentFrontmatter, String), SchemaError> {
    let (yaml, body) = split_frontmatter(markdown)?;
    let fm: AgentFrontmatter =
        serde_yaml::from_str(yaml).map_err(|e| SchemaError::InvalidYaml(e.to_string()))?;
    Ok((fm, body.to_string()))
}

/// Certify markdown against the latest schema. Always returns a result struct;
/// `ok` is false when errors are present.
pub fn certify_agent_markdown(markdown: &str) -> CertificationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut schema_version = 0u32;

    match parse_frontmatter(markdown) {
        Err(e) => {
            errors.push(e.to_string());
        }
        Ok((fm, body)) => {
            schema_version = fm.schema_version;

            if fm.schema_version == 0 {
                errors.push("schema_version must be >= 1".into());
            }
            if fm.schema_version > LATEST_SCHEMA_VERSION {
                errors.push(format!(
                    "schema_version {} is newer than supported latest {}",
                    fm.schema_version, LATEST_SCHEMA_VERSION
                ));
            }
            if fm.schema_version < LATEST_SCHEMA_VERSION {
                warnings.push(format!(
                    "schema_version {} is older than latest {}; consider migrating",
                    fm.schema_version, LATEST_SCHEMA_VERSION
                ));
            }

            if fm.name.trim().is_empty() {
                errors.push("name is required and must be non-empty".into());
            } else if !is_valid_agent_name(&fm.name) {
                errors.push(
                    "name must match [A-Za-z0-9][A-Za-z0-9_-]* (letters, digits, _ , -)".into(),
                );
            }

            if fm.default_model.trim().is_empty() {
                errors.push("default_model is required and must be non-empty".into());
            }

            let role = fm.role.trim().to_ascii_lowercase();
            if role != "agent" && role != "orchestrator" {
                errors.push("role must be 'agent' or 'orchestrator'".into());
            }

            if body.trim().is_empty() {
                warnings.push("agent body (system prompt) is empty".into());
            }

            if fm.tools.is_empty() {
                warnings.push(
                    "tools is empty — agent cannot call built-in tools; use tools: [\"*\"] or list names"
                        .into(),
                );
            } else {
                for err in crate::tools::validate_tool_allowlist(&fm.tools) {
                    errors.push(err);
                }
            }

            if let Some(s) = &fm.server {
                if let Some(p) = s.port {
                    if p == 0 {
                        errors.push("server.port must be non-zero when set".into());
                    }
                }
                if let Some(ref r) = s.reasoning {
                    let r = r.trim().to_ascii_lowercase();
                    if !matches!(r.as_str(), "on" | "off" | "auto") {
                        errors.push("server.reasoning must be on|off|auto".into());
                    }
                }
            }

            if let Some(s) = &fm.sampling {
                if let Some(t) = s.temperature {
                    if !(0.0..=2.0).contains(&t) {
                        errors.push("sampling.temperature must be between 0 and 2".into());
                    }
                }
                if let Some(m) = s.max_tokens {
                    if m == 0 {
                        errors.push("sampling.max_tokens must be > 0 when set".into());
                    }
                }
            }
        }
    }

    CertificationResult {
        ok: errors.is_empty(),
        schema_version,
        latest_schema_version: LATEST_SCHEMA_VERSION,
        errors,
        warnings,
    }
}

pub fn is_valid_agent_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// JSON Schema-like summary for UI / docs (versioned).
pub fn schema_summary() -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    m.insert(
        "schema_version".into(),
        format!("u32, latest={LATEST_SCHEMA_VERSION}"),
    );
    m.insert(
        "name".into(),
        "string, required, [A-Za-z0-9][A-Za-z0-9_-]*".into(),
    );
    m.insert("description".into(), "string, optional".into());
    m.insert(
        "default_model".into(),
        "string, required, registry key".into(),
    );
    m.insert("role".into(), "agent|orchestrator, default agent".into());
    m.insert(
        "tools".into(),
        "string[], tool allowlist: names, category/*, or *".into(),
    );
    m.insert(
        "server".into(),
        "optional object: host, port, ctx, reasoning".into(),
    );
    m.insert(
        "sampling".into(),
        "optional object: temperature, max_tokens, seed".into(),
    );
    m
}
