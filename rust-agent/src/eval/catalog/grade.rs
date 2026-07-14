//! Grade catalog items (post-hoc). Tool calls require parsed JSON; file tasks
//! grade named workspace outputs — never the raw instruction echo.

use super::types::CatalogItem;
use serde_json::{json, Map, Value};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct CatalogGrade {
    pub correct: bool,
    pub score: f64,
    pub detail: String,
    pub metrics: Map<String, Value>,
}

pub fn grade_catalog_item(item: &CatalogItem, workspace: &Path, harness: &str) -> CatalogGrade {
    let g = &item.grade;
    match g.kind.as_str() {
        "dry_marker" | "dry_pass" => {
            let ok = workspace.join("catalog_dry_ok.txt").is_file() || harness == "dry";
            CatalogGrade {
                correct: ok,
                score: if ok { 1.0 } else { 0.0 },
                detail: if ok {
                    "dry_ok".into()
                } else {
                    "missing dry marker".into()
                },
                metrics: Map::new(),
            }
        }
        "sandbox_skip" => {
            if harness == "dry" && g.dry_pass {
                return CatalogGrade {
                    correct: true,
                    score: 1.0,
                    detail: "dry catalog wiring".into(),
                    metrics: Map::new(),
                };
            }
            CatalogGrade {
                correct: false,
                score: 0.0,
                detail: g
                    .skip_reason
                    .clone()
                    .unwrap_or_else(|| "sandbox required".into()),
                metrics: Map::new(),
            }
        }
        "exact" => {
            let raw = read_grade_text(workspace, g.answer_file.as_deref());
            let text = normalize_vendor_output(&raw);
            let exp = g.expected.as_deref().unwrap_or("");
            if exp.is_empty() {
                return fail("exact: empty expected");
            }
            // Match full text, last non-empty line, or any standalone line (CLI chrome).
            let ok = text.trim() == exp.trim()
                || last_nonempty_line(&text) == exp.trim()
                || text.lines().any(|l| l.trim() == exp.trim());
            if !ok
                && looks_like_prompt_echo(&text, &item.prompt)
                && last_nonempty_line(&text) != exp.trim()
            {
                return fail("exact: answer looks like instruction echo");
            }
            CatalogGrade {
                correct: ok,
                score: if ok { 1.0 } else { 0.0 },
                detail: format!(
                    "got={:?} expected={:?}",
                    clip(&last_nonempty_line(&text), 80),
                    exp
                ),
                metrics: Map::new(),
            }
        }
        "boxed" => {
            let text = read_grade_text(workspace, g.answer_file.as_deref());
            if looks_like_prompt_echo(&text, &item.prompt) {
                return fail("boxed: answer looks like instruction echo");
            }
            let extracted = last_boxed(&text).unwrap_or_else(|| last_int(&text));
            let exp = g.expected.as_deref().unwrap_or("");
            let ok = !exp.is_empty() && extracted.trim() == exp.trim();
            let mut metrics = Map::new();
            metrics.insert("extracted".into(), json!(extracted));
            CatalogGrade {
                correct: ok,
                score: if ok { 1.0 } else { 0.0 },
                detail: format!("extracted={extracted:?}"),
                metrics,
            }
        }
        "contains" => {
            let text = read_grade_text(workspace, g.answer_file.as_deref());
            if text.trim().is_empty() {
                return fail("contains: empty answer file");
            }
            if looks_like_prompt_echo(&text, &item.prompt) {
                return fail("contains: answer looks like instruction echo");
            }
            let needle = g
                .expected_contains
                .as_deref()
                .or(g.expected.as_deref())
                .unwrap_or("");
            // Needle must not appear only because the grader read the prompt.
            if needle.is_empty() {
                return fail("contains: no expected needle");
            }
            // Reject needles that appear in the prompt unless answer_file is used
            // (file content is the graded surface).
            if g.answer_file.is_none() && item.prompt.contains(needle) {
                return fail(format!(
                    "contains: needle {needle:?} appears in prompt; set answer_file"
                ));
            }
            let ok = text.contains(needle);
            CatalogGrade {
                correct: ok,
                score: if ok { 1.0 } else { 0.0 },
                detail: format!("contains {needle:?} in {:?} => {ok}", g.answer_file),
                metrics: Map::new(),
            }
        }
        "tool_call" => grade_tool_call(item, workspace),
        "file_exact" => {
            // Alias for exact with required answer_file
            let Some(rel) = g.answer_file.as_deref() else {
                return fail("file_exact requires grade.answer_file");
            };
            let path = workspace.join(rel);
            if !path.is_file() {
                return fail(format!("file_exact: missing {rel}"));
            }
            let text = fs::read_to_string(&path).unwrap_or_default();
            let exp = g.expected.as_deref().unwrap_or("");
            let ok = text.trim() == exp.trim();
            CatalogGrade {
                correct: ok,
                score: if ok { 1.0 } else { 0.0 },
                detail: format!("file {rel}: got={:?} expected={:?}", clip(&text, 60), exp),
                metrics: Map::new(),
            }
        }
        "file_lines_exact" => {
            let Some(rel) = g.answer_file.as_deref() else {
                return fail("file_lines_exact requires answer_file");
            };
            let path = workspace.join(rel);
            if !path.is_file() {
                return fail(format!("file_lines_exact: missing {rel}"));
            }
            let text = fs::read_to_string(&path).unwrap_or_default();
            let exp = g.expected.as_deref().unwrap_or("");
            let got: Vec<&str> = text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
            let want: Vec<&str> = exp.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
            let ok = got == want;
            CatalogGrade {
                correct: ok,
                score: if ok { 1.0 } else { 0.0 },
                detail: format!("file {rel} lines got={got:?} want={want:?}"),
                metrics: Map::new(),
            }
        }
        other => fail(format!("unknown grade kind {other}")),
    }
}

fn grade_tool_call(item: &CatalogItem, workspace: &Path) -> CatalogGrade {
    let g = &item.grade;
    let name = g.tool_name.as_deref().unwrap_or("");
    if name.is_empty() {
        return fail("tool_call: missing tool_name");
    }
    let raw = read_grade_text(workspace, g.answer_file.as_deref());
    if raw.trim().is_empty() {
        return fail("tool_call: empty response");
    }
    // Prefer CLI agent box / post-init region; also scan full text as fallback.
    let focused = normalize_vendor_output(&raw);
    let mut calls = extract_tool_calls(&focused);
    if calls.is_empty() {
        calls = extract_tool_calls(&raw);
    }
    // Prompt-echo alone must fail even if it mentions the tool name.
    if calls.is_empty() && looks_like_prompt_echo(&focused, &item.prompt) {
        return fail("tool_call: instruction echo without JSON tool call");
    }
    if calls.is_empty() {
        return fail("tool_call: no parseable JSON tool call with name+arguments");
    }
    // Drop JSON-Schema-shaped false positives from tool docs in the prompt dump.
    let calls: Vec<_> = calls
        .into_iter()
        .filter(|(_, args)| !looks_like_json_schema_args(args))
        .collect();
    if calls.is_empty() {
        return fail("tool_call: only schema-shaped false positives (no real arguments)");
    }
    let extra = g.expected_contains.as_deref().unwrap_or("");
    let mut matched = false;
    let mut detail = String::new();
    for (n, args) in &calls {
        if n != name {
            continue;
        }
        let args_s = args.to_string();
        if !extra.is_empty() && !args_s.contains(extra) {
            detail = format!("name matched but args missing {extra:?}: {args_s}");
            continue;
        }
        matched = true;
        detail = format!("tool_call ok name={n} args={args_s}");
        break;
    }
    if !matched && detail.is_empty() {
        detail = format!(
            "no call named {name:?}; found {:?}",
            calls.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>()
        );
    }
    CatalogGrade {
        correct: matched,
        score: if matched { 1.0 } else { 0.0 },
        detail,
        metrics: Map::new(),
    }
}

/// Extract JSON objects that look like tool calls: {"name": "...", "arguments": ...}
pub fn extract_tool_calls(text: &str) -> Vec<(String, Value)> {
    let repaired = repair_common_tool_json(text);
    let mut out = Vec::new();
    let bytes = repaired.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'{' {
            i += 1;
            continue;
        }
        if let Some(end) = find_json_object_end(&repaired, i) {
            let slice = &repaired[i..=end];
            if let Ok(v) = serde_json::from_str::<Value>(slice) {
                if let Some(name) = v.get("name").and_then(|x| x.as_str()) {
                    // Real tool calls use "arguments"; tool *definitions* use "parameters".
                    if let Some(args) = v.get("arguments").cloned() {
                        out.push((name.to_string(), args));
                    }
                }
            }
            i = end + 1;
        } else {
            i += 1;
        }
    }
    out
}

/// Fix common model/CLI typos that keep tool calls unparseable.
fn repair_common_tool_json(text: &str) -> String {
    text.replace("\"arguments:", "\"arguments\":")
        .replace("'arguments:", "\"arguments\":")
        .replace("\"name: ", "\"name\": ")
}

/// Hermes / CLI agents wrap the answer in a box after "Initializing agent...".
pub fn normalize_vendor_output(text: &str) -> String {
    let t = text.trim();
    // Prefer region after Hermes init banner
    let after = if let Some(idx) = t.rfind("Initializing agent") {
        &t[idx..]
    } else if let Some(idx) = t.rfind("╭─") {
        &t[idx..]
    } else {
        t
    };
    // Collect all Hermes-style boxes (parallel tool calls often use multiple boxes).
    let mut boxes: Vec<String> = Vec::new();
    let mut in_box = false;
    let mut box_lines: Vec<String> = Vec::new();
    for line in after.lines() {
        let s = line.trim_end();
        if s.starts_with('╭') {
            if in_box && !box_lines.is_empty() {
                boxes.push(box_lines.join(" "));
            }
            in_box = true;
            box_lines.clear();
            continue;
        }
        if s.starts_with('╰') {
            if in_box && !box_lines.is_empty() {
                boxes.push(box_lines.join(" "));
            }
            in_box = false;
            box_lines.clear();
            continue;
        }
        if in_box {
            let inner = s.trim();
            if inner.is_empty() || inner.starts_with('┊') {
                continue;
            }
            // Join wrapped JSON lines with space (Hermes wraps long tool JSON).
            box_lines.push(inner.to_string());
        }
    }
    if in_box && !box_lines.is_empty() {
        boxes.push(box_lines.join(" "));
    }
    if !boxes.is_empty() {
        return boxes.join("\n");
    }
    after.to_string()
}

fn last_nonempty_line(text: &str) -> String {
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .last()
        .unwrap_or("")
        .to_string()
}

fn looks_like_json_schema_args(args: &Value) -> bool {
    // Tool docs in the prompt dump look like {"properties":{...},"type":"object","required":[...]}
    args.get("properties").is_some()
}

fn find_json_object_end(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if start >= bytes.len() || bytes[start] != b'{' {
        return None;
    }
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escape = false;
    for (idx, &b) in bytes.iter().enumerate().skip(start) {
        if in_str {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    None
}

/// True if `text` is mostly the evaluation prompt / Hermes "Query:" dump.
pub fn looks_like_prompt_echo(text: &str, prompt: &str) -> bool {
    let t = text.trim();
    if t.is_empty() {
        return false;
    }
    if t.starts_with("Query:") || t.starts_with("You are being evaluated on an isolated workspace")
    {
        return true;
    }
    // Substantial overlap with the instruction body
    let p = prompt.trim();
    if p.len() >= 40 && t.contains(p) {
        return true;
    }
    // First 80 chars of prompt appear and answer is long dump
    if p.len() >= 40 {
        let head: String = p.chars().take(80).collect();
        if t.contains(&head) && t.len() > p.len() / 2 {
            return true;
        }
    }
    false
}

fn read_grade_text(ws: &Path, answer_file: Option<&str>) -> String {
    if let Some(rel) = answer_file {
        let p = ws.join(rel);
        if p.is_file() {
            return fs::read_to_string(p).unwrap_or_default();
        }
        return String::new();
    }
    for name in ["agent_answer.txt", "agent_transcript.txt", "solution.txt"] {
        let p = ws.join(name);
        if p.is_file() {
            return fs::read_to_string(p).unwrap_or_default();
        }
    }
    String::new()
}

fn fail(detail: impl Into<String>) -> CatalogGrade {
    CatalogGrade {
        correct: false,
        score: 0.0,
        detail: detail.into(),
        metrics: Map::new(),
    }
}

fn clip(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn last_boxed(text: &str) -> Option<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("\\boxed{") {
        let after = &rest[i + "\\boxed{".len()..];
        if let Some(end) = after.find('}') {
            let inner = after[..end].trim();
            if !inner.is_empty() {
                found.push(inner.to_string());
            }
            rest = &after[end + 1..];
        } else {
            break;
        }
    }
    found.pop()
}

fn last_int(text: &str) -> String {
    let nums: Vec<&str> = text
        .split(|c: char| !c.is_ascii_digit() && c != '-')
        .filter(|s| !s.is_empty() && s.chars().any(|c| c.is_ascii_digit()))
        .collect();
    nums.last().map(|s| s.to_string()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::catalog::types::GradeSpec;
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    fn item(kind: &str, prompt: &str) -> CatalogItem {
        CatalogItem {
            id: "t".into(),
            source_id: "s".into(),
            tags: vec![],
            track: "tool_call".into(),
            capabilities: vec![],
            prompt: prompt.into(),
            grade: GradeSpec {
                kind: kind.into(),
                expected: None,
                expected_contains: Some("San Francisco".into()),
                tool_name: Some("get_weather".into()),
                dry_pass: false,
                skip_reason: None,
                requires_sandbox: false,
                answer_file: None,
            },
            tools: vec![],
            workspace_files: BTreeMap::new(),
            links: vec![],
            meta: Value::Null,
        }
    }

    #[test]
    fn tool_call_prompt_echo_fails() {
        let d = tempdir().unwrap();
        let prompt = "Call get_weather for city San Francisco.\nRespond with JSON tool call.";
        let it = item("tool_call", prompt);
        fs::write(
            d.path().join("agent_answer.txt"),
            format!("Query: {prompt}\nget_weather San Francisco"),
        )
        .unwrap();
        let g = grade_catalog_item(&it, d.path(), "hermes");
        assert!(!g.correct, "echo should fail: {}", g.detail);
    }

    #[test]
    fn tool_call_valid_json_passes() {
        let d = tempdir().unwrap();
        let prompt = "Call get_weather for city San Francisco.";
        let it = item("tool_call", prompt);
        fs::write(
            d.path().join("agent_answer.txt"),
            r#"Here: {"name":"get_weather","arguments":{"city":"San Francisco"}}"#,
        )
        .unwrap();
        let g = grade_catalog_item(&it, d.path(), "hermes");
        assert!(g.correct, "{}", g.detail);
    }

    #[test]
    fn tool_call_wrong_name_fails() {
        let d = tempdir().unwrap();
        let it = item("tool_call", "Call get_weather for city San Francisco.");
        fs::write(
            d.path().join("agent_answer.txt"),
            r#"{"name":"send_email","arguments":{"city":"San Francisco"}}"#,
        )
        .unwrap();
        let g = grade_catalog_item(&it, d.path(), "hermes");
        assert!(!g.correct, "{}", g.detail);
    }

    #[test]
    fn file_exact_hello_txt_not_answer_txt() {
        let d = tempdir().unwrap();
        let mut it = item("file_exact", "Create hello.txt with Hello, Terminal-Bench");
        it.grade.kind = "file_exact".into();
        it.grade.expected = Some("Hello, Terminal-Bench".into());
        it.grade.answer_file = Some("hello.txt".into());
        it.grade.tool_name = None;
        it.grade.expected_contains = None;
        // Wrong surface: only agent_answer
        fs::write(d.path().join("agent_answer.txt"), "Hello, Terminal-Bench").unwrap();
        let g = grade_catalog_item(&it, d.path(), "hermes");
        assert!(!g.correct, "must require hello.txt: {}", g.detail);
        fs::write(d.path().join("hello.txt"), "Hello, Terminal-Bench").unwrap();
        let g2 = grade_catalog_item(&it, d.path(), "hermes");
        assert!(g2.correct, "{}", g2.detail);
    }

    #[test]
    fn contains_rejects_needle_in_prompt_without_answer_file() {
        let d = tempdir().unwrap();
        let mut it = item("contains", "Filter data.csv where score>=50");
        it.grade.kind = "contains".into();
        it.grade.expected_contains = Some("score".into());
        it.grade.tool_name = None;
        fs::write(d.path().join("agent_answer.txt"), "Filter data.csv where score>=50").unwrap();
        let g = grade_catalog_item(&it, d.path(), "hermes");
        assert!(!g.correct, "{}", g.detail);
    }

    #[test]
    fn extract_tool_calls_parses_nested() {
        let t = r#"blah {"name":"run_sql","arguments":{"query":"SELECT 1"}} done"#;
        let c = extract_tool_calls(t);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].0, "run_sql");
    }

    #[test]
    fn hermes_box_no_tool_passes_exact() {
        let d = tempdir().unwrap();
        let prompt = "User says only: Thanks!";
        let mut it = item("exact", prompt);
        it.grade.kind = "exact".into();
        it.grade.expected = Some("NO_TOOL".into());
        it.grade.tool_name = None;
        it.grade.expected_contains = None;
        let chrome = r#"Query: You are being evaluated
## Task
User says only: Thanks!
Initializing agent...
╭─ ⚕ Hermes ───────────────────────────────────────────────────────────────────╮
    NO_TOOL
╰──────────────────────────────────────────────────────────────────────────────╯
Session: 123
"#;
        fs::write(d.path().join("agent_answer.txt"), chrome).unwrap();
        let g = grade_catalog_item(&it, d.path(), "hermes");
        assert!(g.correct, "{}", g.detail);
    }

    #[test]
    fn repairs_missing_quote_after_arguments() {
        let t = r#"{"name": "calculate", "arguments: {"expression": "2+2*3"}}"#;
        let c = extract_tool_calls(t);
        assert_eq!(c.len(), 1, "{c:?}");
        assert_eq!(c[0].0, "calculate");
        assert!(c[0].1.to_string().contains("2+2*3"), "{:?}", c[0].1);
    }

    #[test]
    fn ignores_parameters_key_tool_definitions() {
        let t = r#"{"name":"calculate","parameters":{"type":"object","properties":{"expression":{"type":"string"}}}}"#;
        let c = extract_tool_calls(t);
        assert!(c.is_empty(), "definitions must not count as calls: {c:?}");
    }
}
