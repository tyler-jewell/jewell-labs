//! Discover vendors from `evals/vendors/*/vendor.toml`.

use super::cli::CliSpec;
use super::jewell::NativeJewellSpec;
use crate::paths::repo_root;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct VendorManifest {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// `cli` | `native_jewell`
    pub kind: String,
    /// Repo-relative SSoT path for this vendor's harness config (document + enforce).
    #[serde(default)]
    pub config_ssot: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub cli: Option<CliSpec>,
    #[serde(default)]
    pub native_jewell: Option<NativeJewellSpec>,
    /// Filled at load time.
    #[serde(skip)]
    pub dir: PathBuf,
}

fn default_true() -> bool {
    true
}

pub fn vendors_dir(repo: &Path) -> PathBuf {
    repo.join("evals").join("vendors")
}

/// Load all vendor manifests (enabled and disabled), sorted by id.
pub fn load_vendor_manifests(root: Option<&Path>) -> Result<Vec<VendorManifest>, String> {
    let repo = root.map(Path::to_path_buf).unwrap_or_else(repo_root);
    let dir = vendors_dir(&repo);
    if !dir.is_dir() {
        return Err(format!(
            "vendors dir missing: {} — create evals/vendors/<id>/vendor.toml",
            dir.display()
        ));
    }
    let mut out = Vec::new();
    let rd = fs::read_dir(&dir).map_err(|e| format!("read {}: {e}", dir.display()))?;
    for ent in rd.flatten() {
        let p = ent.path();
        if !p.is_dir() {
            continue;
        }
        let toml_path = p.join("vendor.toml");
        if !toml_path.is_file() {
            continue;
        }
        let text = fs::read_to_string(&toml_path)
            .map_err(|e| format!("read {}: {e}", toml_path.display()))?;
        let mut m: VendorManifest =
            toml::from_str(&text).map_err(|e| format!("parse {}: {e}", toml_path.display()))?;
        if m.id.is_empty() {
            m.id = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .into();
        }
        if m.name.is_empty() {
            m.name = m.id.clone();
        }
        m.dir = p;
        validate_manifest(&m)?;
        out.push(m);
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

fn validate_manifest(m: &VendorManifest) -> Result<(), String> {
    match m.kind.as_str() {
        "cli" => {
            if m.cli.is_none() {
                return Err(format!("vendor {}: kind=cli requires [cli] table", m.id));
            }
            if let Some(cli) = &m.cli {
                if let Some(t) = &cli.prompt_template {
                    reject_coach_text(t, &format!("vendor {} prompt_template", m.id))?;
                }
            }
        }
        "native_jewell" => {
            if m.native_jewell.is_none() {
                return Err(format!(
                    "vendor {}: kind=native_jewell requires [native_jewell] table",
                    m.id
                ));
            }
        }
        other => {
            return Err(format!(
                "vendor {}: unknown kind {other:?} (supported: cli, native_jewell)",
                m.id
            ));
        }
    }
    // Require explicit SSoT pointer so configs are discoverable.
    let expected = format!("evals/vendors/{}/vendor.toml", m.id);
    match &m.config_ssot {
        Some(p) if p == &expected || p.ends_with(&format!("vendors/{}/vendor.toml", m.id)) => {}
        Some(p) => {
            return Err(format!(
                "vendor {}: config_ssot={p:?} must point at {expected}",
                m.id
            ));
        }
        None => {
            return Err(format!(
                "vendor {}: missing config_ssot = \"{expected}\"",
                m.id
            ));
        }
    }
    Ok(())
}

/// Poka-yoke: vendor config templates must not smuggle coach / gold text.
fn reject_coach_text(s: &str, where_: &str) -> Result<(), String> {
    let lower = s.to_ascii_lowercase();
    for banned in [
        "format-only",
        "do not greet",
        "never an unedited",
        "double-check content",
        "final correct",
        "do not execute",
        "fictional",
        "keep required prefixes",
        "err- on log",
        "no-tool",
        "you are under automated evaluation",
        "you are being evaluated",
    ] {
        if lower.contains(banned) {
            return Err(format!(
                "{where_}: banned coach phrase {banned:?} — put task text only in catalog items"
            ));
        }
    }
    Ok(())
}

/// Enabled vendor ids in stable order (default harness list).
pub fn default_vendor_ids(root: Option<&Path>) -> Result<Vec<String>, String> {
    Ok(load_vendor_manifests(root)?
        .into_iter()
        .filter(|m| m.enabled)
        .map(|m| m.id)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_jewell_and_hermes() {
        let ms = load_vendor_manifests(None).expect("load vendors");
        let ids: Vec<_> = ms.iter().map(|m| m.id.as_str()).collect();
        assert!(ids.contains(&"jewell"), "{ids:?}");
        assert!(ids.contains(&"hermes"), "{ids:?}");
        let enabled = default_vendor_ids(None).unwrap();
        assert!(enabled.contains(&"jewell".into()));
        assert!(enabled.contains(&"hermes".into()));
        // stable sort
        let mut sorted = enabled.clone();
        sorted.sort();
        assert_eq!(enabled, sorted);
        for m in &ms {
            assert!(
                m.config_ssot.is_some(),
                "vendor {} must declare config_ssot",
                m.id
            );
        }
    }

    #[test]
    fn vendor_adapters_have_no_coach_wrappers() {
        // Poka-yoke: fail CI if coach strings reappear in adapter sources.
        let root = crate::paths::repo_root();
        let paths = [
            root.join("rust-agent/src/eval/vendors/jewell.rs"),
            root.join("rust-agent/src/eval/vendors/cli.rs"),
            root.join("rust-agent/src/eval/compare/adapters/jewell.rs"),
            root.join("rust-agent/src/eval/compare/adapters/hermes.rs"),
        ];
        let banned = [
            "You are under automated evaluation",
            "You are being evaluated",
            "FORMAT-ONLY",
            "never an unedited copy",
            "Double-check content",
            "Do not greet",
            "keep required prefixes like ERR-",
        ];
        for p in paths {
            let text =
                fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
            for b in banned {
                assert!(
                    !text.contains(b),
                    "{} must not contain coach phrase {b:?}",
                    p.display()
                );
            }
        }
    }
}
