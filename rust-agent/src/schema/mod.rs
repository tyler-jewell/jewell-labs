//! Versioned agent frontmatter schema and certification.

mod certify;
mod types;

pub use certify::{
    certify_agent_markdown, is_valid_agent_name, parse_frontmatter, schema_summary,
    split_frontmatter,
};
pub use types::{
    AgentDocument, AgentFrontmatter, CertificationResult, SamplingConfig, SchemaError,
    ServerConfig, LATEST_SCHEMA_VERSION,
};

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"---
schema_version: 1
name: math-tutor
description: A tutor
default_model: qwen3-0.6b
role: agent
tools:
  - list_tools
  - app_status
---

You are a math tutor.
"#;

    #[test]
    fn certify_valid_passes() {
        let r = certify_agent_markdown(VALID);
        assert!(r.ok, "{:?}", r.errors);
        assert!(r.errors.is_empty());
        assert_eq!(r.latest_schema_version, LATEST_SCHEMA_VERSION);
        assert_eq!(r.schema_version, 1);
    }

    #[test]
    fn certify_unknown_tool_fails() {
        let md = r#"---
schema_version: 1
name: broken
description: x
default_model: qwen3-0.6b
tools: [not_a_real_tool]
---

body
"#;
        let r = certify_agent_markdown(md);
        assert!(!r.ok);
        assert!(
            r.errors.iter().any(|e| e.contains("unknown tool")),
            "errors={:?}",
            r.errors
        );
    }

    #[test]
    fn certify_missing_default_model_fails() {
        let md = r#"---
schema_version: 1
name: broken
description: x
tools: ["*"]
---

body
"#;
        let r = certify_agent_markdown(md);
        assert!(!r.ok);
        assert!(
            r.errors.iter().any(|e| e.contains("default_model")),
            "errors={:?}",
            r.errors
        );
    }

    #[test]
    fn certify_missing_frontmatter_fails() {
        let r = certify_agent_markdown("# just markdown\n");
        assert!(!r.ok);
        assert!(!r.errors.is_empty());
        assert!(r.errors[0].contains("frontmatter") || r.errors[0].contains("---"));
    }

    #[test]
    fn certify_empty_name_fails() {
        let md = r#"---
schema_version: 1
name: ""
default_model: qwen3-0.6b
tools: ["*"]
---

body
"#;
        let r = certify_agent_markdown(md);
        assert!(!r.ok);
        assert!(r.errors.iter().any(|e| e.contains("name")));
    }

    #[test]
    fn parse_roundtrip_fields() {
        let (fm, body) = parse_frontmatter(VALID).unwrap();
        assert_eq!(fm.name, "math-tutor");
        assert_eq!(fm.default_model, "qwen3-0.6b");
        assert!(body.contains("math tutor"));
    }

    #[test]
    fn invalid_role_fails() {
        let md = r#"---
schema_version: 1
name: x
default_model: m
role: wizard
tools: ["*"]
---

body
"#;
        let r = certify_agent_markdown(md);
        assert!(!r.ok);
        assert!(r.errors.iter().any(|e| e.contains("role")));
    }
}
