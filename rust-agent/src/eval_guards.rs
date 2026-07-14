//! Safety guards for eval-only chat fields (`eval_fs_root`, `eval_base_url`, `eval_model`).
//!
//! Catalog runners may remount the agent sandbox and pin a local llama-server.
//! Public clients must not point FS tools or inference at arbitrary paths/hosts.

use std::fs;
use std::path::{Component, Path, PathBuf};

/// True when process opted into eval pin fields (optional extra gate).
/// Path/URL hard limits always apply even when this is true.
pub fn eval_pins_env_enabled() -> bool {
    match std::env::var("JEWELL_ALLOW_EVAL_PINS") {
        Ok(v) => {
            let v = v.trim();
            v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
        }
        Err(_) => false,
    }
}

/// Require JEWELL_ALLOW_EVAL_PINS=1 when any eval pin field is set.
pub fn require_eval_pins_enabled(has_any_pin: bool) -> Result<(), String> {
    if has_any_pin && !eval_pins_env_enabled() {
        return Err(
            "eval_base_url / eval_model / eval_fs_root / eval_temperature require \
             JEWELL_ALLOW_EVAL_PINS=1 (catalog runner sets this; public chat ignores eval pins)"
                .into(),
        );
    }
    Ok(())
}

/// Only loopback OpenAI-compatible bases (with optional `/v1`).
///
/// Host must be **exactly** `127.0.0.1`, `localhost`, or `::1` after URL parse —
/// not a `starts_with` prefix (rejects `127.0.0.1.evil.com`, `localhost.attacker.com`).
pub fn sanitize_eval_base_url(raw: Option<&str>) -> Result<Option<String>, String> {
    let Some(u) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let host = parse_http_url_host(u)?;
    let host_lc = host.to_ascii_lowercase();
    let ok = host_lc == "127.0.0.1" || host_lc == "localhost" || host_lc == "::1";
    if !ok {
        return Err(format!(
            "eval_base_url host must be exactly 127.0.0.1, localhost, or ::1 (got {host:?})"
        ));
    }
    Ok(Some(u.trim_end_matches('/').to_string()))
}

/// Extract host from `http(s)://[user@]host[:port][/path]`. Rejects userinfo.
fn parse_http_url_host(u: &str) -> Result<String, String> {
    let lower = u.to_ascii_lowercase();
    let rest = if let Some(r) = lower.strip_prefix("https://") {
        r
    } else if let Some(r) = lower.strip_prefix("http://") {
        r
    } else {
        return Err("eval_base_url must be http:// or https://".into());
    };
    if rest.is_empty() {
        return Err("eval_base_url missing host".into());
    }
    // Authority ends at path / query / fragment
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(rest);
    if authority.is_empty() {
        return Err("eval_base_url missing host".into());
    }
    // userinfo@host is not allowed
    if authority.contains('@') {
        return Err("eval_base_url must not contain userinfo".into());
    }
    // IPv6: [::1] or [::1]:port
    let host = if let Some(inner) = authority.strip_prefix('[') {
        let end = inner
            .find(']')
            .ok_or_else(|| "eval_base_url invalid IPv6 host".to_string())?;
        inner[..end].to_string()
    } else {
        // hostname or IPv4 — strip :port (last colon only for IPv4/name)
        match authority.rfind(':') {
            Some(i) if authority[i + 1..].chars().all(|c| c.is_ascii_digit()) => {
                authority[..i].to_string()
            }
            _ => authority.to_string(),
        }
    };
    if host.is_empty() {
        return Err("eval_base_url missing host".into());
    }
    Ok(host)
}

pub fn sanitize_eval_model(raw: Option<&str>) -> Result<Option<String>, String> {
    let Some(m) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    if m.len() > 256 || m.contains('\0') || m.contains('\n') {
        return Err("eval_model rejected".into());
    }
    Ok(Some(m.to_string()))
}

/// Eval temperature in `[0, 2]`. `None` means leave agent sampling alone.
pub fn sanitize_eval_temperature(raw: Option<f64>) -> Result<Option<f64>, String> {
    let Some(t) = raw else {
        return Ok(None);
    };
    if !t.is_finite() || !(0.0..=2.0).contains(&t) {
        return Err(format!(
            "eval_temperature must be a finite number in [0, 2] (got {t})"
        ));
    }
    Ok(Some(t))
}

/// Absolute path under `{repo_root}/evals/runs/` only.
pub fn sanitize_eval_fs_root(
    raw: Option<&str>,
    repo_root: &Path,
) -> Result<Option<PathBuf>, String> {
    let Some(s) = raw.map(str::trim).filter(|x| !x.is_empty()) else {
        return Ok(None);
    };
    let path = PathBuf::from(s);
    if !path.is_absolute() {
        return Err("eval_fs_root must be an absolute path".into());
    }
    if s.contains('\0') {
        return Err("eval_fs_root null byte".into());
    }
    let allowed = repo_root.join("evals").join("runs");
    // Lexical: path must stay under allowed after cleaning .. components
    let cleaned = clean_path(&path)?;
    let allowed_clean = clean_path(&allowed)?;
    if !cleaned.starts_with(&allowed_clean) {
        return Err(format!(
            "eval_fs_root must be under {} (got {})",
            allowed.display(),
            path.display()
        ));
    }
    // If path exists, canonicalize and re-check (symlink escape)
    if cleaned.exists() {
        let canon = fs::canonicalize(&cleaned).map_err(|e| e.to_string())?;
        let allowed_canon = if allowed.exists() {
            fs::canonicalize(&allowed).map_err(|e| e.to_string())?
        } else {
            allowed_clean.clone()
        };
        if !canon.starts_with(&allowed_canon) {
            return Err(format!(
                "eval_fs_root escapes evals/runs after resolve: {}",
                canon.display()
            ));
        }
        return Ok(Some(canon));
    }
    // Non-existent: ensure we can create under allowed; refuse if any parent is a symlink
    if let Some(parent) = cleaned.parent() {
        let mut cur = parent.to_path_buf();
        while cur.starts_with(&allowed_clean) {
            if cur.exists() {
                let meta = fs::symlink_metadata(&cur).map_err(|e| e.to_string())?;
                if meta.file_type().is_symlink() {
                    return Err("eval_fs_root parent is a symlink".into());
                }
            }
            if cur == allowed_clean {
                break;
            }
            if !cur.pop() {
                break;
            }
        }
    }
    Ok(Some(cleaned))
}

fn clean_path(p: &Path) -> Result<PathBuf, String> {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::Prefix(pref) => out.push(pref.as_os_str()),
            Component::RootDir => out.push(Component::RootDir.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    return Err("path traversal in eval_fs_root".into());
                }
            }
            Component::Normal(s) => out.push(s),
        }
    }
    Ok(out)
}

/// Validate override used by fs tools (defense in depth).
pub fn assert_sandbox_override_allowed(
    ovr: &Path,
    repo_root: &Path,
) -> Result<(), String> {
    let s = ovr.to_string_lossy();
    sanitize_eval_fs_root(Some(s.as_ref()), repo_root)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn loopback_base_ok_remote_rejected() {
        assert!(sanitize_eval_base_url(Some("http://127.0.0.1:8091")).unwrap().is_some());
        assert!(sanitize_eval_base_url(Some("http://localhost:8091/v1")).unwrap().is_some());
        assert!(sanitize_eval_base_url(Some("http://[::1]:8091/v1")).unwrap().is_some());
        assert!(sanitize_eval_base_url(Some("http://evil.com")).is_err());
        assert!(sanitize_eval_base_url(Some("http://user@127.0.0.1")).is_err());
        // DNS lookalikes must not pass (prefix starts_with was wrong)
        assert!(
            sanitize_eval_base_url(Some("http://127.0.0.1.evil.com")).is_err(),
            "127.0.0.1.evil.com must be rejected"
        );
        assert!(
            sanitize_eval_base_url(Some("http://localhost.attacker.com")).is_err(),
            "localhost.attacker.com must be rejected"
        );
        assert!(
            sanitize_eval_base_url(Some("http://127.0.0.1.nip.io:8091")).is_err()
        );
        assert!(sanitize_eval_base_url(Some("https://127.0.0.1.example")).is_err());
    }

    #[test]
    fn parse_host_exact() {
        assert_eq!(parse_http_url_host("http://127.0.0.1:8091/v1").unwrap(), "127.0.0.1");
        assert_eq!(parse_http_url_host("http://localhost/v1").unwrap(), "localhost");
        assert_eq!(parse_http_url_host("http://[::1]:8080").unwrap(), "::1");
        assert_eq!(
            parse_http_url_host("http://127.0.0.1.evil.com").unwrap(),
            "127.0.0.1.evil.com"
        );
    }

    #[test]
    fn eval_temperature_bounds() {
        assert_eq!(sanitize_eval_temperature(None).unwrap(), None);
        assert_eq!(sanitize_eval_temperature(Some(0.0)).unwrap(), Some(0.0));
        assert_eq!(sanitize_eval_temperature(Some(1.5)).unwrap(), Some(1.5));
        assert!(sanitize_eval_temperature(Some(-0.1)).is_err());
        assert!(sanitize_eval_temperature(Some(2.01)).is_err());
        assert!(sanitize_eval_temperature(Some(f64::NAN)).is_err());
    }

    #[test]
    fn fs_root_only_under_evals_runs() {
        let d = tempdir().unwrap();
        let repo = d.path();
        let runs = repo.join("evals/runs/catalog-x/workspace");
        fs::create_dir_all(&runs).unwrap();
        let ok = sanitize_eval_fs_root(Some(runs.to_str().unwrap()), repo).unwrap();
        assert!(ok.unwrap().ends_with("workspace"));
        let home = repo.join("secrets");
        fs::create_dir_all(&home).unwrap();
        assert!(sanitize_eval_fs_root(Some(home.to_str().unwrap()), repo).is_err());
        assert!(sanitize_eval_fs_root(Some("/etc/passwd"), repo).is_err());
    }
}
