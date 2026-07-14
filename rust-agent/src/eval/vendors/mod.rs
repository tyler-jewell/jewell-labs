//! Multi-vendor eval drivers: discover from `evals/vendors/*/vendor.toml`,
//! pin every vendor to the shared [`EvalModel`].

mod cli;
mod jewell;
mod load;
mod model;

pub use load::{default_vendor_ids, load_vendor_manifests, vendors_dir, VendorManifest};
pub use model::{load_eval_model, preflight_eval_model, EvalModel};

use crate::eval::compare::AdapterResult;
use cli::CliVendor;
use jewell::JewellVendor;
use std::path::{Path, PathBuf};

/// Per-item run context.
pub struct VendorContext<'a> {
    pub model: &'a EvalModel,
    pub workspace: &'a Path,
    pub instruction: &'a str,
    pub timeout_s: u64,
    /// Item capability requirements (for skip gating).
    pub required_capabilities: &'a [String],
    /// Catalog track (e.g. `tool_call`, `terminal`) for track-aware CLI args.
    pub track: &'a str,
}

pub enum VendorDriver {
    Cli(CliVendor),
    Jewell(JewellVendor),
}

impl VendorDriver {
    pub fn id(&self) -> &str {
        match self {
            Self::Cli(v) => &v.id,
            Self::Jewell(v) => &v.id,
        }
    }

    pub fn provides(&self) -> &[String] {
        match self {
            Self::Cli(v) => &v.capabilities,
            Self::Jewell(v) => &v.capabilities,
        }
    }

    pub fn resolved_endpoint(&self) -> &str {
        match self {
            Self::Cli(v) => &v.resolved_endpoint,
            Self::Jewell(v) => &v.resolved_endpoint,
        }
    }

    pub fn resolved_model(&self) -> &str {
        match self {
            Self::Cli(v) => &v.resolved_model,
            Self::Jewell(v) => &v.resolved_model,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Cli(_) => "cli",
            Self::Jewell(_) => "native_jewell",
        }
    }

    pub fn pin_model(&mut self, model: &EvalModel, pin_dir: &Path) -> Result<(), String> {
        match self {
            Self::Cli(v) => v.pin_model(model, pin_dir),
            Self::Jewell(v) => {
                v.pin_model(model);
                Ok(())
            }
        }
    }

    pub fn run(&self, ctx: &VendorContext<'_>) -> AdapterResult {
        match self {
            Self::Cli(v) => v.run(ctx),
            Self::Jewell(v) => v.run(ctx),
        }
    }
}

/// Build a driver from a loaded manifest.
pub fn driver_from_manifest(m: &VendorManifest) -> Result<VendorDriver, String> {
    match m.kind.as_str() {
        "cli" => {
            let spec = m
                .cli
                .clone()
                .ok_or_else(|| format!("vendor {}: missing [cli]", m.id))?;
            Ok(VendorDriver::Cli(CliVendor::from_manifest(
                &m.id,
                m.capabilities.clone(),
                spec,
            )))
        }
        "native_jewell" => {
            let spec = m
                .native_jewell
                .clone()
                .ok_or_else(|| format!("vendor {}: missing [native_jewell]", m.id))?;
            Ok(VendorDriver::Jewell(JewellVendor::from_manifest(
                &m.id,
                m.capabilities.clone(),
                spec,
            )))
        }
        other => Err(format!("vendor {}: unsupported kind {other}", m.id)),
    }
}

/// Resolve harness filter against discovered vendors.
///
/// - `filter` empty → all enabled vendors  
/// - otherwise only matching enabled ids (error if unknown)
pub fn resolve_vendors(
    filter: &[String],
    pin_dir: &Path,
    model: &EvalModel,
) -> Result<Vec<VendorDriver>, String> {
    let manifests = load_vendor_manifests(None)?;
    let enabled: Vec<_> = manifests.into_iter().filter(|m| m.enabled).collect();
    let selected: Vec<_> = if filter.is_empty() {
        enabled
    } else {
        let mut out = Vec::new();
        for id in filter {
            let id = id.trim();
            if id.is_empty() {
                continue;
            }
            match enabled.iter().find(|m| m.id == id) {
                Some(m) => out.push(m.clone()),
                None => {
                    let known: Vec<_> = enabled.iter().map(|m| m.id.as_str()).collect();
                    return Err(format!(
                        "unknown or disabled vendor {id:?}; enabled={known:?}"
                    ));
                }
            }
        }
        out
    };
    if selected.is_empty() {
        return Err("no vendors selected (check evals/vendors/*/vendor.toml enabled=true)".into());
    }

    let mut drivers = Vec::new();
    for m in &selected {
        let mut d = driver_from_manifest(m)?;
        d.pin_model(model, pin_dir)?;
        // Fairness: every vendor must report the same endpoint after pin.
        if d.resolved_endpoint() != model.openai_base_url() {
            return Err(format!(
                "vendor {} resolved endpoint {} != pinned {}",
                d.id(),
                d.resolved_endpoint(),
                model.openai_base_url()
            ));
        }
        drivers.push(d);
    }
    Ok(drivers)
}

pub fn vendor_meta_json(drivers: &[VendorDriver], model: &EvalModel) -> serde_json::Value {
    serde_json::json!({
        "eval_model": {
            "base_url": model.openai_base_url(),
            "model": model.model,
            "label": model.display_label(),
            "path": model.path,
        },
        "vendors": drivers.iter().map(|d| serde_json::json!({
            "id": d.id(),
            "kind": d.kind(),
            "endpoint": d.resolved_endpoint(),
            "model": d.resolved_model(),
            "capabilities": d.provides(),
        })).collect::<Vec<_>>(),
    })
}

/// Pin directory under a catalog run folder.
pub fn pin_dir_for_run(run_dir: &Path) -> PathBuf {
    run_dir.join("_vendor_pin")
}
