//! Model registry loader (`models/registry.yaml`).

use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone, Deserialize)]
pub struct ModelRegistry {
    pub models: HashMap<String, ModelSpec>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelSpec {
    pub path: String,
    #[serde(default)]
    pub alias: Option<String>,
    #[serde(default)]
    pub defaults: ModelDefaults,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ModelDefaults {
    #[serde(default)]
    pub ctx: Option<u32>,
    #[serde(default)]
    pub reasoning: Option<String>,
}

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("yaml: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("unknown model key '{0}'")]
    UnknownModel(String),
}

impl ModelRegistry {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, RegistryError> {
        let text = std::fs::read_to_string(path)?;
        let reg: ModelRegistry = serde_yaml::from_str(&text)?;
        Ok(reg)
    }

    pub fn resolve(&self, key: &str) -> Result<ResolvedModel, RegistryError> {
        let spec = self
            .models
            .get(key)
            .ok_or_else(|| RegistryError::UnknownModel(key.to_string()))?;
        let path = expand_home(&spec.path);
        Ok(ResolvedModel {
            key: key.to_string(),
            path,
            alias: spec.alias.clone().unwrap_or_else(|| key.to_string()),
            defaults: spec.defaults.clone(),
        })
    }

    pub fn contains(&self, key: &str) -> bool {
        self.models.contains_key(key)
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedModel {
    pub key: String,
    pub path: PathBuf,
    pub alias: String,
    pub defaults: ModelDefaults,
}

fn expand_home(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn load_and_resolve() {
        let mut f = NamedTempFile::new().unwrap();
        write!(
            f,
            r#"
models:
  demo:
    path: /tmp/demo.gguf
    alias: demo-alias
    defaults:
      ctx: 2048
      reasoning: "off"
"#
        )
        .unwrap();
        let reg = ModelRegistry::load(f.path()).unwrap();
        let m = reg.resolve("demo").unwrap();
        assert_eq!(m.alias, "demo-alias");
        assert_eq!(m.path, PathBuf::from("/tmp/demo.gguf"));
        assert_eq!(m.defaults.ctx, Some(2048));
    }

    #[test]
    fn unknown_model_errors() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "models: {{}}\n").unwrap();
        let reg = ModelRegistry::load(f.path()).unwrap();
        assert!(matches!(
            reg.resolve("nope"),
            Err(RegistryError::UnknownModel(_))
        ));
    }
}
