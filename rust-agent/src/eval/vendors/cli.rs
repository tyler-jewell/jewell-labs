//! Generic CLI vendor driven by vendor.toml `[cli]`.

use super::model::EvalModel;
use super::VendorContext;
use crate::eval::compare::AdapterResult;
use crate::paths::repo_root;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CliSpec {
    #[serde(default)]
    pub bin: Option<String>,
    #[serde(default)]
    pub bin_candidates: Vec<String>,
    /// Args with `{prompt}` and `{model}` placeholders.
    #[serde(default)]
    pub args: Vec<String>,
    /// `workspace` | absolute template
    #[serde(default = "default_cwd")]
    pub cwd: String,
    #[serde(default)]
    pub home_env: Option<String>,
    /// Repo-relative path copied to a run-local home; config rewritten for model pin.
    #[serde(default)]
    pub home_template: Option<String>,
    /// Extra fixed env vars (values may be repo-relative paths).
    #[serde(default)]
    pub extra_env: BTreeMap<String, String>,
    #[serde(default)]
    pub env_model_base_url: Option<String>,
    #[serde(default)]
    pub env_model_name: Option<String>,
    /// Optional args appended after model pin, e.g. `["-m", "{model}"]`.
    #[serde(default)]
    pub model_flag: Vec<String>,
    /// Per-track CLI args override (e.g. BFCL `tool_call` → empty toolset format mode).
    /// Falls back to `args` when track is missing.
    #[serde(default)]
    pub args_by_track: BTreeMap<String, Vec<String>>,
    /// Optional prompt wiring template. Placeholders: `{prompt}`, `{workspace}`, `{model}`.
    /// Default: `{prompt}` (shared catalog instruction only — no coach text).
    /// Must not be used to inject gold or grade hints; keep templates mechanical.
    #[serde(default)]
    pub prompt_template: Option<String>,
    /// Optional sampling temperature written into pinned Hermes-style config.yaml.
    #[serde(default)]
    pub temperature: Option<f64>,
}

fn default_cwd() -> String {
    "workspace".into()
}

#[derive(Debug, Clone)]
pub struct CliVendor {
    pub id: String,
    pub capabilities: Vec<String>,
    pub spec: CliSpec,
    /// Run-local home path after pin_model (if any).
    pub pinned_home: Option<PathBuf>,
    pub resolved_endpoint: String,
    pub resolved_model: String,
}

impl CliVendor {
    pub fn from_manifest(id: &str, capabilities: Vec<String>, spec: CliSpec) -> Self {
        Self {
            id: id.into(),
            capabilities,
            spec,
            pinned_home: None,
            resolved_endpoint: String::new(),
            resolved_model: String::new(),
        }
    }

    pub fn pin_model(&mut self, model: &EvalModel, pin_dir: &Path) -> Result<(), String> {
        self.resolved_endpoint = model.openai_base_url();
        self.resolved_model = model.model.clone();

        if let Some(template_rel) = &self.spec.home_template {
            let repo = repo_root();
            let src = resolve_repo_path(&repo, template_rel);
            if !src.is_dir() {
                return Err(format!(
                    "vendor {}: home_template not a directory: {}",
                    self.id,
                    src.display()
                ));
            }
            let dest = pin_dir.join(&self.id).join("home");
            if dest.exists() {
                let _ = fs::remove_dir_all(&dest);
            }
            copy_dir_minimal(&src, &dest)?;
            write_pinned_config(&dest, model, self.spec.temperature)?;
            self.pinned_home = Some(dest);
        }
        Ok(())
    }

    pub fn run(&self, ctx: &VendorContext<'_>) -> AdapterResult {
        let bin = match resolve_bin(&self.spec) {
            Some(b) => b,
            None => {
                return AdapterResult::skip(
                    format!(
                        "vendor {}: binary not found (candidates: {:?})",
                        self.id, self.spec.bin_candidates
                    ),
                    vec![format!("{}_cli", self.id)],
                );
            }
        };

        // Fairness: pass the shared catalog instruction only.
        // Optional `prompt_template` in vendor.toml may substitute {prompt}/{workspace}
        // for CLI wiring (cwd path) — no coaching text is hard-coded here.
        let prompt = expand_prompt_template(
            self.spec.prompt_template.as_deref(),
            ctx.instruction.trim(),
            &ctx.workspace.display().to_string(),
            &ctx.model.model,
        );

        let arg_template =
            select_args_template(&self.spec.args, &self.spec.args_by_track, ctx.track);
        let mut args: Vec<String> = arg_template
            .iter()
            .map(|a| expand(a, &prompt, &ctx.model.model))
            .collect();
        for a in &self.spec.model_flag {
            args.push(expand(a, &prompt, &ctx.model.model));
        }

        let mut cmd = Command::new(&bin);
        cmd.args(&args);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        if self.spec.cwd == "workspace" {
            cmd.current_dir(ctx.workspace);
        }

        let repo = repo_root();
        for (k, v) in &self.spec.extra_env {
            let val = resolve_repo_path(&repo, v);
            cmd.env(k, val);
        }
        if let Some(home_env) = &self.spec.home_env {
            if let Some(home) = &self.pinned_home {
                cmd.env(home_env, home);
            } else if let Some(t) = &self.spec.home_template {
                cmd.env(home_env, resolve_repo_path(&repo, t));
            }
        }
        if let Some(k) = &self.spec.env_model_base_url {
            cmd.env(k, ctx.model.openai_base_url());
        }
        if let Some(k) = &self.spec.env_model_name {
            cmd.env(k, &ctx.model.model);
        }
        // Common OpenAI-compatible env keys for future CLI vendors
        cmd.env("OPENAI_BASE_URL", ctx.model.openai_base_url());
        cmd.env("OPENAI_API_BASE", ctx.model.openai_base_url());
        cmd.env("OPENAI_API_KEY", "local-not-required");

        let timeout_s = ctx.timeout_s.max(30).min(600);
        let t0 = Instant::now();
        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => return AdapterResult::error(format!("{} spawn: {e}", self.id)),
        };
        let pid = child.id();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(child.wait_with_output());
        });
        match rx.recv_timeout(Duration::from_secs(timeout_s)) {
            Ok(Ok(o)) => {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                let t_ms = t0.elapsed().as_millis() as u64;
                if !o.status.success() {
                    return AdapterResult {
                        status: "error".into(),
                        stdout,
                        stderr,
                        t_ms,
                        detail: format!("{} exit {:?}", self.id, o.status.code()),
                        capabilities_missing: vec![],
                        exit_code: o.status.code(),
                    };
                }
                AdapterResult {
                    status: "ok".into(),
                    stdout,
                    stderr,
                    t_ms,
                    detail: format!(
                        "{} completed (model={} @ {})",
                        self.id, self.resolved_model, self.resolved_endpoint
                    ),
                    capabilities_missing: vec![],
                    exit_code: Some(0),
                }
            }
            Ok(Err(e)) => AdapterResult::error(format!("{} wait: {e}", self.id)),
            Err(_timeout) => {
                // Kill hung CLI tree (Hermes spawns nested python).
                let _ = Command::new("kill")
                    .args(["-TERM", &pid.to_string()])
                    .status();
                thread::sleep(Duration::from_millis(400));
                let _ = Command::new("kill")
                    .args(["-KILL", &pid.to_string()])
                    .status();
                let _ = Command::new("pkill")
                    .args(["-P", &pid.to_string()])
                    .status();
                AdapterResult {
                    status: "error".into(),
                    stdout: String::new(),
                    stderr: format!("timeout after {timeout_s}s"),
                    t_ms: t0.elapsed().as_millis() as u64,
                    detail: format!("{} timeout after {timeout_s}s", self.id),
                    capabilities_missing: vec![],
                    exit_code: None,
                }
            }
        }
    }
}

fn expand(s: &str, prompt: &str, model: &str) -> String {
    s.replace("{prompt}", prompt).replace("{model}", model)
}

fn expand_prompt_template(
    template: Option<&str>,
    instruction: &str,
    workspace: &str,
    model: &str,
) -> String {
    let t = template.unwrap_or("{prompt}");
    t.replace("{prompt}", instruction)
        .replace("{workspace}", workspace)
        .replace("{model}", model)
}

/// Select CLI argv template for a catalog track (used by tests + run).
pub fn select_args_template<'a>(
    default_args: &'a [String],
    by_track: &'a BTreeMap<String, Vec<String>>,
    track: &str,
) -> &'a [String] {
    by_track
        .get(track)
        .filter(|v| !v.is_empty())
        .map(|v| v.as_slice())
        .unwrap_or(default_args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_aware_args_prefer_tool_call_override() {
        let default: Vec<String> = vec![
            "chat".into(),
            "-t".into(),
            "terminal,file,code_execution".into(),
        ];
        let mut by = BTreeMap::new();
        by.insert(
            "tool_call".into(),
            vec![
                "chat".into(),
                "-t".into(),
                "none".into(),
                "--ignore-rules".into(),
            ],
        );
        let fmt = select_args_template(&default, &by, "tool_call");
        assert!(fmt.iter().any(|a| a == "none"), "{fmt:?}");
        assert!(fmt.iter().any(|a| a == "--ignore-rules"), "{fmt:?}");
        let term = select_args_template(&default, &by, "terminal");
        assert!(term.iter().any(|a| a.contains("terminal")), "{term:?}");
    }

    #[test]
    fn default_prompt_template_is_instruction_only() {
        let out = expand_prompt_template(None, "Task: hello", "/tmp/ws", "local");
        assert_eq!(out, "Task: hello");
        let out2 = expand_prompt_template(
            Some("cwd={workspace}\n{prompt}"),
            "Task: hello",
            "/tmp/ws",
            "local",
        );
        assert_eq!(out2, "cwd=/tmp/ws\nTask: hello");
        assert!(!out2.to_ascii_lowercase().contains("evaluated"));
    }
}

fn resolve_repo_path(repo: &Path, rel_or_abs: &str) -> PathBuf {
    let p = PathBuf::from(rel_or_abs);
    if p.is_absolute() {
        p
    } else {
        repo.join(p)
    }
}

fn resolve_bin(spec: &CliSpec) -> Option<PathBuf> {
    let repo = repo_root();
    let mut cands = Vec::new();
    if let Some(b) = &spec.bin {
        cands.push(b.clone());
    }
    cands.extend(spec.bin_candidates.iter().cloned());
    for c in cands {
        let p = resolve_repo_path(&repo, &c);
        if p.is_file() {
            return Some(p);
        }
        if let Some(w) = which(&c) {
            return Some(w);
        }
    }
    None
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn copy_dir_minimal(src: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for ent in fs::read_dir(src).map_err(|e| e.to_string())? {
        let ent = ent.map_err(|e| e.to_string())?;
        let from = ent.path();
        let to = dest.join(ent.file_name());
        if from.is_dir() {
            // Skip heavy caches
            let name = ent.file_name().to_string_lossy().to_string();
            if matches!(
                name.as_str(),
                "audio_cache" | "image_cache" | "logs" | "memories" | "lsp"
            ) {
                continue;
            }
            copy_dir_minimal(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|e| format!("copy {}: {e}", from.display()))?;
        }
    }
    Ok(())
}

/// Write Hermes-style config.yaml pinned to EvalModel (works for hermes home_template).
fn write_pinned_config(
    home: &Path,
    model: &EvalModel,
    temperature: Option<f64>,
) -> Result<(), String> {
    let temp_line = temperature
        .map(|t| format!("  temperature: {t}\n"))
        .unwrap_or_default();
    let cfg = format!(
        r#"model:
  default: "{label}"
  provider: "custom"
  base_url: "{base}"
  api_key: "local-not-required"
  context_length: 65536
  max_tokens: 1024
{temp}auxiliary:
  compression:
    provider: "main"
    context_length: 65536
terminal:
  backend: local
agent:
  api_max_retries: 1
"#,
        label = if model.path.as_ref().map(|s| !s.is_empty()).unwrap_or(false) {
            model.path.as_deref().unwrap_or(&model.model)
        } else {
            model.display_label()
        },
        base = model.openai_base_url(),
        temp = temp_line,
    );
    fs::write(home.join("config.yaml"), cfg).map_err(|e| format!("write config.yaml: {e}"))
}
