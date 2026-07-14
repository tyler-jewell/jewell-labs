//! Shared EvalModel pin for multi-vendor catalog runs.

use crate::paths::repo_root;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalModel {
    /// OpenAI-compatible base, usually with `/v1` (e.g. http://127.0.0.1:8091/v1).
    pub base_url: String,
    /// Model id sent in chat/completions `model` field.
    pub model: String,
    /// Human label for reports.
    #[serde(default)]
    pub label: String,
    /// Optional GGUF / weights path for documentation only.
    #[serde(default)]
    pub path: Option<String>,
}

impl Default for EvalModel {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:8091/v1".into(),
            model: "local".into(),
            label: "default".into(),
            path: None,
        }
    }
}

impl EvalModel {
    /// Base URL without trailing `/v1` — for rust-agent `ChatEndpoint`.
    pub fn chat_host_base(&self) -> String {
        let u = self.base_url.trim_end_matches('/');
        u.strip_suffix("/v1").unwrap_or(u).to_string()
    }

    /// Base URL with `/v1` for OpenAI-compatible clients (Hermes, etc.).
    pub fn openai_base_url(&self) -> String {
        let u = self.base_url.trim_end_matches('/');
        if u.ends_with("/v1") {
            u.to_string()
        } else {
            format!("{u}/v1")
        }
    }

    pub fn display_label(&self) -> &str {
        if self.label.is_empty() {
            &self.model
        } else {
            &self.label
        }
    }
}

pub fn default_model_path(repo: &Path) -> PathBuf {
    repo.join("evals").join("model.toml")
}

pub fn load_eval_model(path: Option<&Path>) -> Result<EvalModel, String> {
    let repo = repo_root();
    let p = path
        .map(Path::to_path_buf)
        .or_else(|| std::env::var_os("EVAL_MODEL_FILE").map(PathBuf::from))
        .unwrap_or_else(|| default_model_path(&repo));

    let mut m = if p.is_file() {
        let text = fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
        toml::from_str::<EvalModel>(&text).map_err(|e| format!("parse {}: {e}", p.display()))?
    } else {
        EvalModel::default()
    };

    if let Ok(u) = std::env::var("EVAL_MODEL_BASE_URL") {
        if !u.trim().is_empty() {
            m.base_url = u;
        }
    }
    if let Ok(id) = std::env::var("EVAL_MODEL") {
        if !id.trim().is_empty() {
            m.model = id;
        }
    }
    if let Ok(l) = std::env::var("EVAL_MODEL_LABEL") {
        if !l.trim().is_empty() {
            m.label = l;
        }
    }
    Ok(m)
}

/// Fail closed if the shared endpoint is not reachable.
pub fn preflight_eval_model(model: &EvalModel) -> Result<(), String> {
    let base = model.openai_base_url();
    let url = format!("{}/models", base.trim_end_matches('/'));
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("http client: {e}"))?;
    let resp = client.get(&url).send().map_err(|e| {
        format!(
            "eval model preflight failed at {url}: {e}\n\
                 Start llama-server on the pinned endpoint, or set EVAL_MODEL_BASE_URL."
        )
    })?;
    if !resp.status().is_success() {
        return Err(format!(
            "eval model preflight HTTP {} at {url}",
            resp.status()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_host_strips_v1() {
        let m = EvalModel {
            base_url: "http://127.0.0.1:8091/v1".into(),
            model: "local".into(),
            label: "q".into(),
            path: None,
        };
        assert_eq!(m.chat_host_base(), "http://127.0.0.1:8091");
        assert_eq!(m.openai_base_url(), "http://127.0.0.1:8091/v1");
    }

    #[test]
    fn parse_default_shape() {
        let t = r#"
base_url = "http://127.0.0.1:8091/v1"
model = "local"
label = "Qwen3-4B"
"#;
        let m: EvalModel = toml::from_str(t).unwrap();
        assert_eq!(m.model, "local");
        assert_eq!(m.display_label(), "Qwen3-4B");
    }
}
