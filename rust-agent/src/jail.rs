//! Pre-open path jail for agent-writable surfaces.
//!
//! Allowed roots only; rejects abs/`..`/`\0`, symlinks, and hardlinks (nlink>1).
//! Never writes then "checks later" — resolve before open.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum JailError {
    #[error("absolute path rejected: {0}")]
    Absolute(String),
    #[error("null byte in path: {0}")]
    NullByte(String),
    #[error("path traversal rejected: {0}")]
    Traversal(String),
    #[error("path outside allowed roots: {0}")]
    OutsideRoot(String),
    #[error("symlink refused: {0}")]
    Symlink(String),
    #[error("hardlink refused (nlink>1): {0}")]
    Hardlink(String),
    #[error("core agent write locked: {0}")]
    CoreLocked(String),
    #[error("io: {0}")]
    Io(String),
}

impl From<std::io::Error> for JailError {
    fn from(e: std::io::Error) -> Self {
        JailError::Io(e.to_string())
    }
}

/// Relative write roots under a workspace (e.g. `agents`, `evals/datasets`).
#[derive(Debug, Clone)]
pub struct WriteJail {
    pub workspace: PathBuf,
    /// Relative root segments under workspace, e.g. `["agents"]`.
    pub allowed_roots: Vec<PathBuf>,
    /// Agent ids that may not be overwritten (default: core/orchestrator).
    pub locked_agent_ids: Vec<String>,
}

impl WriteJail {
    /// Jail whose workspace *is* the agents directory; rel paths are `cat/name.md`.
    pub fn agents_dir(agents_dir: impl Into<PathBuf>) -> Self {
        Self {
            workspace: agents_dir.into(),
            // empty root = any relative path under workspace (still no ..)
            allowed_roots: vec![PathBuf::new()],
            locked_agent_ids: vec![crate::agents::CORE_AGENT_ID.to_string()],
        }
    }

    /// Crate/workspace root with explicit subtrees (agents/, evals/datasets/).
    pub fn workspace_roots(workspace: impl Into<PathBuf>, roots: &[&str]) -> Self {
        Self {
            workspace: workspace.into(),
            allowed_roots: roots.iter().map(PathBuf::from).collect(),
            locked_agent_ids: vec![crate::agents::CORE_AGENT_ID.to_string()],
        }
    }

    /// Validate `rel` and return absolute path under an allowed root (not yet created).
    pub fn resolve_for_write(&self, rel: &str) -> Result<PathBuf, JailError> {
        if rel.contains('\0') {
            return Err(JailError::NullByte(rel.into()));
        }
        let p = Path::new(rel);
        if p.is_absolute() {
            return Err(JailError::Absolute(rel.into()));
        }
        let mut clean = PathBuf::new();
        for c in p.components() {
            match c {
                Component::Normal(s) => clean.push(s),
                Component::CurDir => {}
                Component::ParentDir => return Err(JailError::Traversal(rel.into())),
                Component::RootDir | Component::Prefix(_) => {
                    return Err(JailError::Absolute(rel.into()))
                }
            }
        }
        if clean.as_os_str().is_empty() {
            return Err(JailError::Traversal(rel.into()));
        }
        let under_root = if self.allowed_roots.is_empty()
            || self
                .allowed_roots
                .iter()
                .all(|r| r.as_os_str().is_empty())
        {
            true // whole workspace (still no ..)
        } else {
            self.allowed_roots
                .iter()
                .any(|root| clean == *root || clean.starts_with(root))
        };
        if !under_root {
            return Err(JailError::OutsideRoot(rel.into()));
        }
        // Lexical only — never canonicalize through a symlink at the target.
        let abs = self.workspace.join(&clean);
        // Ensure workspace itself is real when it exists.
        if self.workspace.is_dir() {
            let ws = fs::canonicalize(&self.workspace).map_err(JailError::from)?;
            // Parent chain must not leave workspace via existing symlinks.
            let mut check = abs.clone();
            while check != self.workspace && check.pop() {
                if check.exists() {
                    let meta = fs::symlink_metadata(&check)?;
                    if meta.file_type().is_symlink() {
                        return Err(JailError::Symlink(rel.into()));
                    }
                    let canon = fs::canonicalize(&check)?;
                    if !canon.starts_with(&ws) {
                        return Err(JailError::OutsideRoot(rel.into()));
                    }
                }
            }
        }
        Ok(abs)
    }

    pub fn assert_not_core_lock(&self, agent_ref: &str) -> Result<(), JailError> {
        if self.locked_agent_ids.iter().any(|id| id == agent_ref) {
            return Err(JailError::CoreLocked(agent_ref.into()));
        }
        Ok(())
    }

    /// Create a new file (fail if exists as symlink). Atomic-ish replace for regular files.
    pub fn write_file(&self, rel: &str, bytes: &[u8]) -> Result<PathBuf, JailError> {
        let path = self.resolve_for_write(rel)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
            // Refuse if any parent component is a symlink
            self.refuse_symlink_chain(parent)?;
        }
        if path.exists() {
            let meta = fs::symlink_metadata(&path)?;
            if meta.file_type().is_symlink() {
                return Err(JailError::Symlink(rel.into()));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if meta.nlink() > 1 {
                    return Err(JailError::Hardlink(rel.into()));
                }
            }
            // Replace regular file in place
            let mut f = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&path)?;
            f.write_all(bytes)?;
            f.sync_all()?;
        } else {
            // create new — do not follow symlinks
            let mut f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            f.write_all(bytes)?;
            f.sync_all()?;
        }
        // Post-check: still a regular file under workspace
        let meta = fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() {
            let _ = fs::remove_file(&path);
            return Err(JailError::Symlink(rel.into()));
        }
        Ok(path)
    }

    fn refuse_symlink_chain(&self, dir: &Path) -> Result<(), JailError> {
        let mut cur = dir.to_path_buf();
        loop {
            if cur == self.workspace {
                break;
            }
            if cur.exists() {
                let meta = fs::symlink_metadata(&cur)?;
                if meta.file_type().is_symlink() {
                    return Err(JailError::Symlink(cur.display().to_string()));
                }
            }
            if !cur.pop() {
                break;
            }
        }
        Ok(())
    }
}

/// Map agent id `cat/name` → relative path under agents dir: `cat/name.md`.
pub fn agent_file_rel(agent_ref: &str) -> Result<String, JailError> {
    let parts: Vec<&str> = agent_ref.split('/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(JailError::Traversal(agent_ref.into()));
    }
    if parts
        .iter()
        .any(|p| *p == ".." || p.contains('\0') || p.contains('\\') || p.contains('/'))
    {
        return Err(JailError::Traversal(agent_ref.into()));
    }
    Ok(format!("{}/{}.md", parts[0], parts[1]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    #[test]
    fn allow_agent_deny_src_and_traversal() {
        let d = tempdir().unwrap();
        let jail = WriteJail::workspace_roots(d.path(), &["agents"]);
        assert!(jail.resolve_for_write("agents/lab/bot.md").is_ok());
        assert!(matches!(
            jail.resolve_for_write("src/main.rs"),
            Err(JailError::OutsideRoot(_))
        ));
        assert!(matches!(
            jail.resolve_for_write("agents/../src/evil.rs"),
            Err(JailError::Traversal(_))
        ));
        assert!(matches!(
            jail.resolve_for_write("/etc/passwd"),
            Err(JailError::Absolute(_))
        ));
        assert!(matches!(
            jail.resolve_for_write("agents/core/../../Cargo.toml"),
            Err(JailError::Traversal(_))
        ));
    }

    #[test]
    fn write_ok_and_core_lock() {
        let d = tempdir().unwrap();
        let jail = WriteJail::agents_dir(d.path());
        jail.assert_not_core_lock("lab/bot").unwrap();
        assert!(matches!(
            jail.assert_not_core_lock("core/orchestrator"),
            Err(JailError::CoreLocked(_))
        ));
        let p = jail.write_file("lab/bot.md", b"hello").unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), "hello");
    }

    #[test]
    fn symlink_target_refused() {
        let d = tempdir().unwrap();
        let outside = d.path().join("outside.txt");
        fs::write(&outside, b"SAFE").unwrap();
        fs::create_dir_all(d.path().join("lab")).unwrap();
        let link = d.path().join("lab/bot.md");
        symlink(&outside, &link).unwrap();
        let jail = WriteJail::agents_dir(d.path());
        let err = jail.write_file("lab/bot.md", b"PWN").unwrap_err();
        assert!(matches!(err, JailError::Symlink(_)), "{err:?}");
        assert_eq!(fs::read_to_string(&outside).unwrap(), "SAFE");
    }

    #[test]
    fn agent_file_rel_ok() {
        assert_eq!(
            agent_file_rel("tutoring/math-tutor").unwrap(),
            "tutoring/math-tutor.md"
        );
        assert!(agent_file_rel("../x").is_err());
        assert!(agent_file_rel("only").is_err());
    }
}
