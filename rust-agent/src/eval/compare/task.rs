//! Harbor-lite task discovery and workspace materialization.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    pub path: PathBuf,
    pub track: String,
    pub timeout_s: u64,
    pub capabilities: Vec<String>,
    pub n_runs_default: u32,
    pub title: String,
}

impl Task {
    pub fn instruction_path(&self) -> PathBuf {
        self.path.join("instruction.md")
    }

    pub fn workspace_src(&self) -> PathBuf {
        self.path.join("workspace")
    }

    pub fn workspace_gold(&self) -> PathBuf {
        self.path.join("workspace_gold")
    }

    pub fn instruction(&self) -> std::io::Result<String> {
        fs::read_to_string(self.instruction_path())
    }

    /// Copy seed workspace into `dest/workspace`, return workspace path.
    pub fn materialize_workspace(&self, dest: &Path) -> std::io::Result<PathBuf> {
        fs::create_dir_all(dest)?;
        let ws = dest.join("workspace");
        if ws.exists() {
            let _ = fs::remove_dir_all(&ws);
        }
        let src = self.workspace_src();
        if src.is_dir() {
            copy_dir_all(&src, &ws)?;
        } else {
            fs::create_dir_all(&ws)?;
        }
        Ok(ws)
    }
}

/// Tiny TOML subset: `key = value` (str/int/float/bool/list-of-str).
pub fn parse_simple_toml(text: &str) -> BTreeMap<String, TomlVal> {
    let mut data = BTreeMap::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || !line.contains('=') {
            continue;
        }
        let (key, val) = line.split_once('=').unwrap();
        let key = key.trim().to_string();
        let val = val.trim();
        data.insert(key, parse_toml_val(val));
    }
    data
}

#[derive(Debug, Clone)]
pub enum TomlVal {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    List(Vec<String>),
}

fn parse_toml_val(val: &str) -> TomlVal {
    if val.starts_with('[') && val.ends_with(']') {
        let inner = &val[1..val.len() - 1];
        let mut items = Vec::new();
        for part in inner.split(',') {
            let p = part.trim().trim_matches('"').trim_matches('\'').to_string();
            if !p.is_empty() {
                items.push(p);
            }
        }
        return TomlVal::List(items);
    }
    if (val.starts_with('"') && val.ends_with('"')) || (val.starts_with('\'') && val.ends_with('\''))
    {
        return TomlVal::Str(val[1..val.len() - 1].to_string());
    }
    match val.to_ascii_lowercase().as_str() {
        "true" => return TomlVal::Bool(true),
        "false" => return TomlVal::Bool(false),
        _ => {}
    }
    if let Ok(i) = val.parse::<i64>() {
        return TomlVal::Int(i);
    }
    if let Ok(f) = val.parse::<f64>() {
        return TomlVal::Float(f);
    }
    TomlVal::Str(val.to_string())
}

impl TomlVal {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(i) => Some(*i),
            _ => None,
        }
    }
    pub fn as_list(&self) -> Option<&[String]> {
        match self {
            Self::List(v) => Some(v),
            _ => None,
        }
    }
}

pub fn load_task(task_dir: &Path) -> std::io::Result<Task> {
    let toml_path = task_dir.join("task.toml");
    if !toml_path.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("missing task.toml in {}", task_dir.display()),
        ));
    }
    let meta = parse_simple_toml(&fs::read_to_string(toml_path)?);
    let id = meta
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| task_dir.file_name().and_then(|n| n.to_str()).unwrap_or("task"))
        .to_string();
    let track = meta
        .get("track")
        .and_then(|v| v.as_str())
        .unwrap_or("coding")
        .to_string();
    let timeout_s = meta
        .get("timeout_s")
        .and_then(|v| v.as_i64())
        .unwrap_or(300)
        .max(1) as u64;
    let capabilities = meta
        .get("capabilities")
        .and_then(|v| v.as_list())
        .map(|l| l.to_vec())
        .unwrap_or_default();
    let n_runs_default = meta
        .get("n_runs_default")
        .and_then(|v| v.as_i64())
        .unwrap_or(1)
        .max(1) as u32;
    let title = meta
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or(&id)
        .to_string();
    Ok(Task {
        id,
        path: task_dir.to_path_buf(),
        track,
        timeout_s,
        capabilities,
        n_runs_default,
        title,
    })
}

/// `filter_spec`: `all` | comma tracks | comma task ids.
pub fn discover_tasks(tasks_root: &Path, filter_spec: &str) -> std::io::Result<Vec<Task>> {
    if !tasks_root.is_dir() {
        return Ok(vec![]);
    }
    let mut tasks = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(tasks_root)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    entries.sort();
    for child in entries {
        let name = child.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('_') {
            continue;
        }
        if !child.join("task.toml").is_file() {
            continue;
        }
        tasks.push(load_task(&child)?);
    }

    let spec = filter_spec.trim();
    if spec.is_empty() || spec == "all" {
        return Ok(tasks);
    }
    let parts: Vec<&str> = spec.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    let all_tracks: std::collections::BTreeSet<_> =
        tasks.iter().map(|t| t.track.as_str()).collect();
    let tracks: std::collections::BTreeSet<_> = parts
        .iter()
        .copied()
        .filter(|p| all_tracks.contains(p))
        .collect();
    let ids: std::collections::BTreeSet<_> = parts
        .iter()
        .copied()
        .filter(|p| !tracks.contains(p))
        .collect();

    let mut out: Vec<Task> = tasks
        .iter()
        .filter(|t| tracks.contains(t.track.as_str()) || ids.contains(t.id.as_str()))
        .cloned()
        .collect();
    if out.is_empty() {
        out = tasks
            .into_iter()
            .filter(|t| parts.iter().any(|p| t.id.contains(p) || t.id.starts_with(p)))
            .collect();
    }
    Ok(out)
}

pub fn compare_tasks_dir(repo_root: &Path) -> PathBuf {
    repo_root
        .join("evals")
        .join("harness_compare")
        .join("tasks")
}

pub fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), to)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::repo_root;

    #[test]
    fn parse_task_toml_sample() {
        let text = r#"
id = "coding_fix_bug"
track = "coding"
timeout_s = 240
capabilities = ["terminal", "write_file", "coding"]
n_runs_default = 1
"#;
        let m = parse_simple_toml(text);
        assert_eq!(m.get("id").and_then(|v| v.as_str()), Some("coding_fix_bug"));
        assert_eq!(m.get("timeout_s").and_then(|v| v.as_i64()), Some(240));
        assert_eq!(
            m.get("capabilities").and_then(|v| v.as_list()).map(|l| l.len()),
            Some(3)
        );
    }

    #[test]
    fn discover_local_tasks_empty_after_online_only() {
        // Hard-coded Harbor smokes removed; public evals use online catalog remotes.
        let root = repo_root();
        let dir = compare_tasks_dir(&root);
        if !dir.is_dir() {
            return;
        }
        let all = discover_tasks(&dir, "all").expect("discover");
        assert!(
            all.is_empty(),
            "local compare tasks must be empty (use eval_catalog online sources); got {}",
            all.len()
        );
    }
}
