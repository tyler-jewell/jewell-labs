//! Hydrate catalog items from **online** public datasets only.
//!
//! Repo sources ship `remote.toml` pointers (URLs + official ids/file names).
//! Task bodies are never stored as hard-coded items.jsonl.

use super::types::{CatalogItem, GradeSpec, SourceMeta};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Parse remote.toml simple key=value, including multi-line `[ "a", "b" ]` arrays.
fn parse_simple_toml(text: &str) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    let mut lines = text.lines().peekable();
    while let Some(raw) = lines.next() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || !line.contains('=') {
            continue;
        }
        let (k, v0) = line.split_once('=').unwrap();
        let key = k.trim().to_string();
        let mut v = v0.trim().to_string();
        // Multi-line array: files = [\n  "a",\n]
        if v.starts_with('[') && !v.contains(']') {
            let mut buf = v;
            while let Some(more) = lines.next() {
                buf.push(' ');
                buf.push_str(more.trim());
                if more.contains(']') {
                    break;
                }
            }
            v = buf;
        }
        m.insert(key, v);
    }
    m
}

fn parse_list_field(v: &str) -> Vec<String> {
    let v = v.trim();
    if v.starts_with('[') && v.ends_with(']') {
        v[1..v.len() - 1]
            .split(',')
            .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else if v.is_empty() {
        vec![]
    } else {
        vec![v.trim_matches('"').to_string()]
    }
}

fn parse_usize(v: &str, default: usize) -> usize {
    v.trim()
        .trim_matches('"')
        .parse()
        .unwrap_or(default)
}

fn cache_dir(source: &SourceMeta) -> PathBuf {
    source
        .path
        .parent() // sources/
        .and_then(|p| p.parent()) // catalog/
        .map(|c| c.join(".cache").join(&source.id))
        .unwrap_or_else(|| PathBuf::from("evals/catalog/.cache").join(&source.id))
}

fn http_get(url: &str) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent("jewell-labs-eval-catalog/1.0 (+https://github.com/jewell-labs)")
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(url).send().map_err(|e| format!("GET {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GET {url}: HTTP {}", resp.status()));
    }
    resp.text().map_err(|e| e.to_string())
}

fn cached_get(source: &SourceMeta, cache_key: &str, url: &str) -> Result<String, String> {
    let dir = cache_dir(source);
    let _ = fs::create_dir_all(&dir);
    let path = dir.join(cache_key.replace('/', "__"));
    if path.is_file() {
        return fs::read_to_string(&path).map_err(|e| e.to_string());
    }
    let body = http_get(url)?;
    if let Ok(mut f) = fs::File::create(&path) {
        let _ = f.write_all(body.as_bytes());
    }
    Ok(body)
}

/// Load items for a source that declares `remote.toml` (required).
pub fn load_remote_items(source: &SourceMeta) -> Result<Vec<CatalogItem>, String> {
    let remote_path = source.path.join("remote.toml");
    if !remote_path.is_file() {
        return Err(format!(
            "source {}: missing remote.toml (online dataset pointer required; hard-coded items.jsonl is not allowed)",
            source.id
        ));
    }
    let text = fs::read_to_string(&remote_path).map_err(|e| e.to_string())?;
    let m = parse_simple_toml(&text);
    let kind = m
        .get("remote_kind")
        .map(|s| s.trim_matches('"').to_string())
        .unwrap_or_default();
    match kind.as_str() {
        "bfcl_github" => load_bfcl(source, &m),
        "terminal_bench_github" => load_terminal_bench(source, &m),
        "swe_bench_github_pr" => load_swe_bench(source, &m),
        other => Err(format!(
            "source {}: unknown remote_kind {other:?} in remote.toml",
            source.id
        )),
    }
}

// ─── BFCL ───────────────────────────────────────────────────────────────────

fn load_bfcl(source: &SourceMeta, m: &BTreeMap<String, String>) -> Result<Vec<CatalogItem>, String> {
    let base = m
        .get("base_url")
        .map(|s| s.trim_matches('"').to_string())
        .ok_or_else(|| "bfcl remote.toml: base_url required".to_string())?;
    let files = m
        .get("files")
        .map(|s| parse_list_field(s))
        .unwrap_or_default();
    if files.is_empty() {
        return Err("bfcl remote.toml: files list empty".into());
    }
    let max_per = m
        .get("max_per_file")
        .map(|s| parse_usize(s, 20))
        .unwrap_or(20);

    let mut items = Vec::new();
    for file in &files {
        let url = format!("{}/{}", base.trim_end_matches('/'), file);
        let body = cached_get(source, file, &url)?;
        let answers = load_bfcl_answers(source, &base, file).unwrap_or_default();
        let is_irrelevance = file.to_ascii_lowercase().contains("irrelevance");
        let mut n = 0usize;
        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let row: Value = serde_json::from_str(line)
                .map_err(|e| format!("bfcl parse {file}: {e}"))?;
            let id = row
                .get("id")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            if id.is_empty() {
                continue;
            }
            let prompt_user = extract_bfcl_user_text(&row);
            let tools = extract_bfcl_tools(&row);
            // Prompt body = remote BFCL user text only. Grading maps online gold → our kinds.
            let grade = if is_irrelevance {
                GradeSpec {
                    kind: "no_tool_call".into(),
                    expected: None,
                    expected_contains: None,
                    tool_name: None,
                    answer_file: None,
                    dry_pass: false,
                    skip_reason: None,
                    requires_sandbox: false,
                }
            } else {
                let (tool_name, contains) = bfcl_gold_hint(&answers, &id, &tools);
                GradeSpec {
                    kind: "tool_call".into(),
                    expected: None,
                    expected_contains: contains,
                    tool_name: Some(tool_name),
                    answer_file: None,
                    dry_pass: false,
                    skip_reason: None,
                    requires_sandbox: false,
                }
            };

            let prompt = prompt_user;

            items.push(CatalogItem {
                id: id.clone(),
                source_id: source.id.clone(),
                tags: source.tags.clone(),
                track: "tool_call".into(),
                capabilities: vec!["tool_call".into()],
                prompt,
                grade,
                tools,
                workspace_files: BTreeMap::new(),
                links: source.links.clone(),
                meta: json!({
                    "remote": url,
                    "file": file,
                    "dataset": "BFCL",
                }),
            });
            n += 1;
            if n >= max_per {
                break;
            }
        }
    }
    if items.is_empty() {
        return Err("bfcl: hydrated 0 items from remote files".into());
    }
    Ok(items)
}

fn load_bfcl_answers(
    source: &SourceMeta,
    base: &str,
    file: &str,
) -> Result<BTreeMap<String, Value>, String> {
    let url = format!(
        "{}/possible_answer/{}",
        base.trim_end_matches('/'),
        file
    );
    let body = match cached_get(source, &format!("possible_answer__{file}"), &url) {
        Ok(b) => b,
        Err(_) => return Ok(BTreeMap::new()),
    };
    let mut map = BTreeMap::new();
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(row) = serde_json::from_str::<Value>(line) {
            if let Some(id) = row.get("id").and_then(|x| x.as_str()) {
                map.insert(id.to_string(), row);
            }
        }
    }
    Ok(map)
}

fn extract_bfcl_user_text(row: &Value) -> String {
    // question: [[{role, content}, ...], ...]  (turns)
    if let Some(q) = row.get("question") {
        if let Some(turns) = q.as_array() {
            for turn in turns {
                if let Some(msgs) = turn.as_array() {
                    for msg in msgs {
                        if msg.get("role").and_then(|r| r.as_str()) == Some("user") {
                            if let Some(c) = msg.get("content").and_then(|c| c.as_str()) {
                                return c.to_string();
                            }
                        }
                    }
                }
            }
        }
    }
    String::new()
}

fn extract_bfcl_tools(row: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    if let Some(funcs) = row.get("function").and_then(|f| f.as_array()) {
        for f in funcs {
            // Convert BFCL function schema → JSON-schema-ish tool object for prompt injection.
            let name = f.get("name").cloned().unwrap_or(json!("unknown"));
            let description = f.get("description").cloned().unwrap_or(json!(""));
            let parameters = f.get("parameters").cloned().unwrap_or(json!({}));
            out.push(json!({
                "name": name,
                "description": description,
                "parameters": parameters,
            }));
        }
    }
    out
}

fn bfcl_gold_hint(
    answers: &BTreeMap<String, Value>,
    id: &str,
    tools: &[Value],
) -> (String, Option<String>) {
    let default_name = tools
        .first()
        .and_then(|t| t.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("unknown")
        .to_string();

    let Some(ans) = answers.get(id) else {
        return (default_name, None);
    };
    let Some(gt) = ans.get("ground_truth").and_then(|g| g.as_array()) else {
        return (default_name, None);
    };
    let Some(first) = gt.first().and_then(|x| x.as_object()) else {
        return (default_name, None);
    };
    let Some((tool_name, params)) = first.iter().next() else {
        return (default_name, None);
    };
    // Prefer a distinctive required arg value for expected_contains.
    let mut contains = None;
    if let Some(obj) = params.as_object() {
        for (_k, v) in obj {
            if let Some(arr) = v.as_array() {
                for cand in arr {
                    if let Some(s) = cand.as_str() {
                        if !s.is_empty() {
                            contains = Some(s.to_string());
                            break;
                        }
                    } else if let Some(n) = cand.as_i64() {
                        contains = Some(n.to_string());
                        break;
                    } else if let Some(n) = cand.as_f64() {
                        contains = Some(n.to_string());
                        break;
                    }
                }
            }
            if contains.is_some() {
                break;
            }
        }
    }
    (tool_name.clone(), contains)
}

// ─── Terminal-Bench ─────────────────────────────────────────────────────────

fn load_terminal_bench(
    source: &SourceMeta,
    m: &BTreeMap<String, String>,
) -> Result<Vec<CatalogItem>, String> {
    let base = m
        .get("base_url")
        .map(|s| s.trim_matches('"').to_string())
        .ok_or_else(|| "terminal-bench remote.toml: base_url required".to_string())?;
    let tasks = m
        .get("tasks")
        .map(|s| parse_list_field(s))
        .unwrap_or_default();
    if tasks.is_empty() {
        return Err("terminal-bench remote.toml: tasks list empty".into());
    }

    let mut items = Vec::new();
    let mut errors = Vec::new();
    for task in &tasks {
        let url = format!(
            "{}/{}/task.yaml",
            base.trim_end_matches('/'),
            task
        );
        let body = match cached_get(source, &format!("{task}__task.yaml"), &url) {
            Ok(b) => b,
            Err(e) => {
                errors.push(e);
                continue;
            }
        };
        let instruction = extract_yaml_instruction(&body);
        if instruction.trim().is_empty() {
            errors.push(format!("{task}: empty instruction in remote task.yaml"));
            continue;
        }
        items.push(CatalogItem {
            id: task.clone(),
            source_id: source.id.clone(),
            tags: source.tags.clone(),
            track: "terminal".into(),
            capabilities: vec![
                "terminal".into(),
                "write_file".into(),
                "coding".into(),
            ],
            prompt: instruction,
            grade: GradeSpec {
                kind: "sandbox_skip".into(),
                expected: Some("sandbox_wiring_ok".into()),
                expected_contains: None,
                tool_name: None,
                answer_file: None,
                dry_pass: true,
                skip_reason: Some(
                    "Full Terminal-Bench task requires Harbor/Docker (official task.yaml loaded online)"
                        .into(),
                ),
                requires_sandbox: true,
            },
            tools: vec![],
            workspace_files: BTreeMap::new(),
            links: vec![
                format!(
                    "https://github.com/laude-institute/terminal-bench/tree/main/original-tasks/{task}"
                ),
            ],
            meta: json!({
                "remote": url,
                "task_family": task,
                "dataset": "Terminal-Bench",
            }),
        });
    }
    if items.is_empty() {
        return Err(format!(
            "terminal-bench: hydrated 0 tasks; errors: {}",
            errors.join("; ")
        ));
    }
    Ok(items)
}

/// Minimal extract of `instruction: |-` / `instruction: |` block from task.yaml.
fn extract_yaml_instruction(yaml: &str) -> String {
    let mut lines = yaml.lines().peekable();
    while let Some(line) = lines.next() {
        let t = line.trim_start();
        if t.starts_with("instruction:") {
            let rest = t["instruction:".len()..].trim();
            if rest == "|-" || rest == "|" || rest == ">" || rest == ">-" {
                let mut body = Vec::new();
                while let Some(peek) = lines.peek() {
                    // block ends at next top-level key (no leading space) or empty? keep indented lines
                    if peek.starts_with(' ') || peek.starts_with('\t') {
                        let l = lines.next().unwrap();
                        // strip common indent of 2 spaces
                        let stripped = l.strip_prefix("  ").unwrap_or(l);
                        body.push(stripped.to_string());
                    } else if peek.trim().is_empty() {
                        body.push(String::new());
                        lines.next();
                    } else {
                        break;
                    }
                }
                return body.join("\n").trim().to_string();
            }
            if !rest.is_empty() {
                return rest.trim_matches('"').trim_matches('\'').to_string();
            }
        }
    }
    String::new()
}

// ─── SWE-bench ──────────────────────────────────────────────────────────────

fn load_swe_bench(
    source: &SourceMeta,
    m: &BTreeMap<String, String>,
) -> Result<Vec<CatalogItem>, String> {
    let ids = m
        .get("instance_ids")
        .map(|s| parse_list_field(s))
        .unwrap_or_default();
    if ids.is_empty() {
        return Err("swe-bench remote.toml: instance_ids empty".into());
    }
    let dataset_url = m
        .get("dataset_url")
        .map(|s| s.trim_matches('"').to_string())
        .unwrap_or_else(|| "https://huggingface.co/datasets/princeton-nlp/SWE-bench_Lite".into());

    let mut items = Vec::new();
    for id in &ids {
        let (owner, repo, number) = match parse_swe_instance_id(id) {
            Some(t) => t,
            None => continue,
        };
        let api = format!("https://api.github.com/repos/{owner}/{repo}/pulls/{number}");
        let (title, body) = match cached_get(source, &format!("pr__{id}.json"), &api) {
            Ok(raw) => {
                if let Ok(v) = serde_json::from_str::<Value>(&raw) {
                    (
                        v.get("title")
                            .and_then(|t| t.as_str())
                            .unwrap_or(id)
                            .to_string(),
                        v.get("body")
                            .and_then(|b| b.as_str())
                            .unwrap_or("")
                            .to_string(),
                    )
                } else {
                    (id.clone(), String::new())
                }
            }
            Err(_) => (id.clone(), String::new()),
        };

        let mut prompt = format!(
            "SWE-bench Lite instance `{id}` ({owner}/{repo}#{number}).\n\n**{title}**\n\n"
        );
        if !body.trim().is_empty() {
            // cap very long PR bodies for local models
            let clipped: String = body.chars().take(6000).collect();
            prompt.push_str(&clipped);
            if body.chars().count() > 6000 {
                prompt.push_str("\n\n[…truncated…]");
            }
        } else {
            prompt.push_str(&format!(
                "(Problem statement could not be fetched online. See {dataset_url} instance `{id}`.)"
            ));
        }
        prompt.push_str(
            "\n\nWhen a full repo sandbox with unit tests is unavailable, respond with a concise technical analysis only.",
        );

        items.push(CatalogItem {
            id: id.clone(),
            source_id: source.id.clone(),
            tags: source.tags.clone(),
            track: "coding".into(),
            capabilities: vec!["coding".into(), "write_file".into(), "terminal".into()],
            prompt,
            grade: GradeSpec {
                kind: "sandbox_skip".into(),
                expected: Some("sandbox_wiring_ok".into()),
                expected_contains: None,
                tool_name: None,
                answer_file: None,
                dry_pass: true,
                skip_reason: Some(
                    "Full SWE-bench needs Docker + repository checkout + unit tests (swebench.com)"
                        .into(),
                ),
                requires_sandbox: true,
            },
            tools: vec![],
            workspace_files: BTreeMap::new(),
            links: vec![
                dataset_url.clone(),
                format!("https://github.com/{owner}/{repo}/pull/{number}"),
            ],
            meta: json!({
                "instance_id": id,
                "remote": api,
                "dataset": "SWE-bench_Lite",
            }),
        });
    }
    if items.is_empty() {
        return Err("swe-bench: hydrated 0 instances".into());
    }
    Ok(items)
}

/// `owner__repo-number` → (owner, repo, number). Repo may contain hyphens.
fn parse_swe_instance_id(id: &str) -> Option<(String, String, u64)> {
    let (left, num_s) = id.rsplit_once('-')?;
    let number: u64 = num_s.parse().ok()?;
    let (owner, repo) = left.split_once("__")?;
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner.to_string(), repo.to_string(), number))
}

/// Reject accidental hard-coded item files next to remote sources.
pub fn assert_no_hardcoded_items(source_dir: &Path) -> Result<(), String> {
    let banned = source_dir.join("items.jsonl");
    if banned.is_file() {
        // Allow empty / comment-only files? No — any items.jsonl is forbidden.
        let mut f = fs::File::open(&banned).map_err(|e| e.to_string())?;
        let mut buf = String::new();
        f.read_to_string(&mut buf).map_err(|e| e.to_string())?;
        if buf.lines().any(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#')
        }) {
            return Err(format!(
                "{}: hard-coded items.jsonl is forbidden; use remote.toml + online datasets only",
                banned.display()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_swe_instance_ids() {
        let (o, r, n) = parse_swe_instance_id("django__django-11099").unwrap();
        assert_eq!((o.as_str(), r.as_str(), n), ("django", "django", 11099));
        let (o, r, n) = parse_swe_instance_id("scikit-learn__scikit-learn-13439").unwrap();
        assert_eq!(o, "scikit-learn");
        assert_eq!(r, "scikit-learn");
        assert_eq!(n, 13439);
    }

    #[test]
    fn extracts_yaml_instruction_block() {
        let y = r#"# canary
instruction: |-
  Create a file called /app/hello.txt.
  Write "Hello, world!" to it.
author_name: Alex
difficulty: easy
"#;
        let inst = extract_yaml_instruction(y);
        assert!(inst.contains("hello.txt"), "{inst}");
        assert!(inst.contains("Hello, world!"), "{inst}");
    }

    #[test]
    fn parse_multiline_toml_arrays() {
        let t = r#"
files = [
  "BFCL_v4_simple_python.json",
  "BFCL_v4_irrelevance.json",
]
max_per_file = 20
"#;
        let m = parse_simple_toml(t);
        let files = parse_list_field(m.get("files").unwrap());
        assert_eq!(files.len(), 2, "{files:?}");
        assert_eq!(files[0], "BFCL_v4_simple_python.json");
    }
}
