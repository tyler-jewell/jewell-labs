//! Grade catalog items (post-hoc). Tool calls require parsed JSON; file tasks
//! grade named workspace outputs — never the raw instruction echo.

use super::types::CatalogItem;
use serde_json::{json, Map, Value};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Wall-clock budget for local python3 unit grades (agent code can hang).
const PYTHON_GRADE_TIMEOUT_SECS: u64 = 20;

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
        // BFCL irrelevance: model must not emit a tool/function call.
        "no_tool_call" => {
            let text = read_grade_text(workspace, g.answer_file.as_deref());
            let calls = extract_tool_calls(&text);
            let ok = calls.is_empty();
            CatalogGrade {
                correct: ok,
                score: if ok { 1.0 } else { 0.0 },
                detail: if ok {
                    "no_tool_call ok".into()
                } else {
                    format!(
                        "expected no tool call, found {:?}",
                        calls.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>()
                    )
                },
                metrics: Map::new(),
            }
        }
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
            let got: Vec<&str> = text
                .lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty())
                .collect();
            let want: Vec<&str> = exp
                .lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty())
                .collect();
            let ok = got == want;
            CatalogGrade {
                correct: ok,
                score: if ok { 1.0 } else { 0.0 },
                detail: format!("file {rel} lines got={got:?} want={want:?}"),
                metrics: Map::new(),
            }
        }
        // Coding: agent writes solution.py; prefer local python3 unit tests, else structural smoke.
        "python_workspace" | "solution_file" => grade_solution_file(item, workspace),
        other => fail(format!("unknown grade kind {other}")),
    }
}

/// Grade coding solution files written by the agent.
///
/// 1. Structural smoke (file exists, entry defined, not stub/seed/echo).
/// 2. When `meta.python_grade` has tests, run them with local `python3`.
fn grade_solution_file(item: &CatalogItem, workspace: &Path) -> CatalogGrade {
    let g = &item.grade;
    let rel = g.answer_file.as_deref().unwrap_or("solution.py");
    let path = workspace.join(rel);
    if !path.is_file() {
        return fail(format!(
            "solution_file: missing {rel} (agent must write the solution file to the workspace)"
        ));
    }
    let code = fs::read_to_string(&path).unwrap_or_default();
    if code.trim().is_empty() {
        return fail(format!("solution_file: {rel} is empty"));
    }

    let entry = required_entry_point(item);
    if entry.is_empty() {
        return fail("solution_file: missing entry_point / function name in grade meta");
    }

    if !defines_python_fn(&code, &entry) {
        return fail(format!("solution_file: no `def {entry}` in {rel}"));
    }

    // Seeded workspace content must not score as a completed solution.
    if let Some(seed) = item.workspace_files.get(rel) {
        if is_seed_unchanged(&code, seed) {
            return fail(format!(
                "solution_file: {rel} still matches seeded stub (agent did not implement)"
            ));
        }
    }

    if is_unimplemented_stub(&code, &entry) {
        return fail(format!(
            "solution_file: `{entry}` still looks like an unimplemented stub"
        ));
    }

    // Reject pure prompt-echo / starter paste (no line-count gate — HE stubs are long).
    if looks_like_prompt_echo(&code, &item.prompt) {
        return fail("solution_file: solution looks like prompt/stub echo only");
    }

    let mut metrics = Map::new();
    metrics.insert("entry_point".into(), json!(entry));
    metrics.insert("bytes".into(), json!(code.len()));

    // Prefer real unit-test execution when payload is present.
    match try_python_unit_grade(item, &path, &entry) {
        PythonGradeResult::Ran(g) => {
            for (k, v) in g.metrics {
                metrics.insert(k, v);
            }
            metrics.insert("grader".into(), json!("python3"));
            CatalogGrade {
                correct: g.correct,
                score: g.score,
                detail: g.detail,
                metrics,
            }
        }
        PythonGradeResult::Unavailable(reason) => {
            metrics.insert("python3".into(), json!(reason));
            // Without executable tests, structural smoke never awards full credit.
            metrics.insert("grader".into(), json!("structural_smoke"));
            CatalogGrade {
                correct: false,
                score: 0.5,
                detail: format!(
                    "structural smoke only (def {entry} present; python3 grade unavailable: {reason})"
                ),
                metrics,
            }
        }
        PythonGradeResult::NoTests => {
            metrics.insert("grader".into(), json!("structural_smoke"));
            CatalogGrade {
                correct: false,
                score: 0.5,
                detail: format!(
                    "structural smoke only (def {entry} present; no python_grade tests in meta)"
                ),
                metrics,
            }
        }
    }
}

enum PythonGradeResult {
    Ran(CatalogGrade),
    NoTests,
    Unavailable(String),
}

/// Run MBPP asserts or HumanEval `check()` via local python3.
fn try_python_unit_grade(
    item: &CatalogItem,
    solution_path: &Path,
    entry: &str,
) -> PythonGradeResult {
    let mode = item
        .meta
        .pointer("/python_grade/mode")
        .and_then(|t| t.as_str())
        .unwrap_or("");

    let has_mbpp = item
        .meta
        .pointer("/python_grade/test_list")
        .and_then(|t| t.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    let has_he = item
        .meta
        .pointer("/python_grade/test")
        .and_then(|t| t.as_str())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    if !has_mbpp && !has_he {
        return PythonGradeResult::NoTests;
    }

    let sol = solution_path
        .to_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| solution_path.display().to_string());

    let py = if mode == "humaneval" || (has_he && !has_mbpp) {
        let test = item
            .meta
            .pointer("/python_grade/test")
            .and_then(|t| t.as_str())
            .unwrap_or("");
        humaneval_grade_script(&sol, entry, test)
    } else {
        let setup = item
            .meta
            .pointer("/python_grade/test_setup_code")
            .and_then(|t| t.as_str())
            .unwrap_or("");
        let asserts: Vec<String> = item
            .meta
            .pointer("/python_grade/test_list")
            .and_then(|t| t.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        mbpp_grade_script(&sol, setup, &asserts)
    };

    let out = match run_python3_timed(&py, PYTHON_GRADE_TIMEOUT_SECS) {
        Ok(o) => o,
        Err(PythonRunError::Spawn(e)) => {
            return PythonGradeResult::Unavailable(format!("spawn python3: {e}"));
        }
        Err(PythonRunError::Timeout) => {
            return PythonGradeResult::Ran(CatalogGrade {
                correct: false,
                score: 0.0,
                detail: format!("python3 grade timeout after {PYTHON_GRADE_TIMEOUT_SECS}s"),
                metrics: {
                    let mut m = Map::new();
                    m.insert("timeout_s".into(), json!(PYTHON_GRADE_TIMEOUT_SECS));
                    m
                },
            });
        }
        Err(PythonRunError::Wait(e)) => {
            return PythonGradeResult::Unavailable(format!("python3 wait: {e}"));
        }
    };

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    // Prefer last JSON line from stdout.
    for line in stdout.lines().rev() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            let correct = v.get("correct").and_then(|x| x.as_bool()).unwrap_or(false);
            let score = v
                .get("score")
                .and_then(|x| x.as_f64())
                .unwrap_or(if correct { 1.0 } else { 0.0 });
            let detail = v
                .get("detail")
                .and_then(|x| x.as_str())
                .unwrap_or("python3 grade")
                .to_string();
            let mut metrics = Map::new();
            if let Some(m) = v.get("metrics").and_then(|x| x.as_object()) {
                for (k, val) in m {
                    metrics.insert(k.clone(), val.clone());
                }
            }
            if !out.status.success() && correct {
                // Don't trust a "correct" if process failed hard.
                return PythonGradeResult::Ran(CatalogGrade {
                    correct: false,
                    score: 0.0,
                    detail: format!("python3 grade process failed: {stderr}"),
                    metrics,
                });
            }
            return PythonGradeResult::Ran(CatalogGrade {
                correct,
                score,
                detail,
                metrics,
            });
        }
    }

    if !out.status.success() {
        return PythonGradeResult::Ran(CatalogGrade {
            correct: false,
            score: 0.0,
            detail: format!(
                "python3 grade failed (exit={:?}): {}",
                out.status.code(),
                clip(&stderr, 200)
            ),
            metrics: Map::new(),
        });
    }

    // Process ran but harness produced no parseable grade JSON — hard fail, not structural smoke.
    PythonGradeResult::Ran(CatalogGrade {
        correct: false,
        score: 0.0,
        detail: format!(
            "python3 grade produced no JSON (exit=0); stderr={}",
            clip(&stderr, 120)
        ),
        metrics: Map::new(),
    })
}

enum PythonRunError {
    Spawn(String),
    Wait(String),
    Timeout,
}

/// Spawn `python3 -c` and kill it if it exceeds `timeout_secs`.
fn run_python3_timed(
    script: &str,
    timeout_secs: u64,
) -> Result<std::process::Output, PythonRunError> {
    let child = Command::new("python3")
        .arg("-c")
        .arg(script)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| PythonRunError::Spawn(e.to_string()))?;
    let pid = child.id();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    match rx.recv_timeout(Duration::from_secs(timeout_secs)) {
        Ok(Ok(out)) => Ok(out),
        Ok(Err(e)) => Err(PythonRunError::Wait(e.to_string())),
        Err(_) => {
            // Timed out: kill the process (and best-effort process group via pkill).
            let _ = Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .status();
            thread::sleep(Duration::from_millis(200));
            let _ = Command::new("kill")
                .args(["-KILL", &pid.to_string()])
                .status();
            // Drain wait so we don't leave zombies.
            let _ = rx.recv_timeout(Duration::from_secs(2));
            Err(PythonRunError::Timeout)
        }
    }
}

fn mbpp_grade_script(solution_path: &str, setup: &str, asserts: &[String]) -> String {
    let asserts_json = serde_json::to_string(asserts).unwrap_or_else(|_| "[]".into());
    // Exec source text directly (no importlib) so same-path overwrites never hit stale __pycache__.
    // signal.alarm is a secondary timeout (Unix); Rust also kills the subprocess.
    format!(
        r#"
import json, sys
try:
    import signal
    signal.alarm({timeout})
except Exception:
    pass
path = {path:?}
setup = {setup:?}
asserts = json.loads({asserts_json:?})
ns = {{"__name__": "candidate_solution", "__file__": path}}
try:
    if setup.strip():
        exec(setup, ns, ns)
    src = open(path, "r", encoding="utf-8").read()
    exec(compile(src, path, "exec"), ns, ns)
except Exception as e:
    print(json.dumps({{"correct": False, "score": 0.0, "detail": f"load/setup error: {{type(e).__name__}}: {{e}}", "metrics": {{"phase": "load"}}}}))
    sys.exit(0)
ok = 0
failed = None
for i, line in enumerate(asserts):
    try:
        exec(line, ns, ns)
        ok += 1
    except Exception as e:
        failed = f"assert[{{i}}] {{line[:80]}}: {{type(e).__name__}}: {{e}}"
        break
total = max(len(asserts), 1)
score = ok / total
print(json.dumps({{
    "correct": ok == len(asserts) and len(asserts) > 0,
    "score": score if len(asserts) > 0 else 0.0,
    "detail": "all asserts passed" if failed is None else failed,
    "metrics": {{"asserts_ok": ok, "asserts_total": len(asserts), "mode": "mbpp"}},
}}))
"#,
        path = solution_path,
        setup = setup,
        asserts_json = asserts_json,
        timeout = PYTHON_GRADE_TIMEOUT_SECS,
    )
}

fn humaneval_grade_script(solution_path: &str, entry: &str, test: &str) -> String {
    format!(
        r#"
import json, sys
try:
    import signal
    signal.alarm({timeout})
except Exception:
    pass
path = {path:?}
entry = {entry:?}
test = {test:?}
ns = {{"__name__": "candidate_solution", "__file__": path}}
try:
    src = open(path, "r", encoding="utf-8").read()
    exec(compile(src, path, "exec"), ns, ns)
    if entry not in ns:
        print(json.dumps({{"correct": False, "score": 0.0, "detail": f"missing entry_point {{entry}}", "metrics": {{"mode": "humaneval"}}}}))
        sys.exit(0)
    exec(test, ns, ns)
    check = ns.get("check")
    if check is None:
        print(json.dumps({{"correct": False, "score": 0.0, "detail": "test has no check()", "metrics": {{"mode": "humaneval"}}}}))
        sys.exit(0)
    check(ns[entry])
    print(json.dumps({{"correct": True, "score": 1.0, "detail": "check() passed", "metrics": {{"mode": "humaneval"}}}}))
except Exception as e:
    print(json.dumps({{
        "correct": False,
        "score": 0.0,
        "detail": f"check failed: {{type(e).__name__}}: {{e}}",
        "metrics": {{"mode": "humaneval", "error_type": type(e).__name__}},
    }}))
"#,
        path = solution_path,
        entry = entry,
        test = test,
        timeout = PYTHON_GRADE_TIMEOUT_SECS,
    )
}

fn required_entry_point(item: &CatalogItem) -> String {
    if let Some(e) = item
        .meta
        .pointer("/python_grade/entry_point")
        .and_then(|t| t.as_str())
    {
        return e.to_string();
    }
    // MBPP: first assert line `assert fn_name(...)`
    if let Some(tests) = item
        .meta
        .pointer("/python_grade/test_list")
        .and_then(|t| t.as_array())
    {
        for t in tests {
            if let Some(line) = t.as_str() {
                if let Some(name) = extract_assert_fn_name(line) {
                    return name;
                }
            }
        }
    }
    item.grade
        .tool_name
        .clone()
        .unwrap_or_else(|| "solution".into())
}

/// Extract function name from an MBPP-style `assert fn_name(...)` line.
pub(crate) fn extract_assert_fn_name(assert_line: &str) -> Option<String> {
    let s = assert_line.trim().strip_prefix("assert ")?;
    let name = s.split('(').next()?.trim();
    if name.is_empty() || name.contains(' ') {
        return None;
    }
    Some(name.to_string())
}

/// True when on-disk code is still the seeded stub (exact or whitespace-normalized).
fn is_seed_unchanged(code: &str, seed: &str) -> bool {
    let a = normalize_py_ws(code);
    let b = normalize_py_ws(seed);
    if a.is_empty() || b.is_empty() {
        return false;
    }
    if a == b {
        return true;
    }
    // Near-prefix: agent only appended blank lines / comments, or kept seed as prefix
    // of a still-incomplete body (seed is entire meaningful content).
    if a.starts_with(&b) {
        let rest = a[b.len()..].trim();
        if rest.is_empty() {
            return true;
        }
        // Only comments / pass / ellipsis after seed
        let only_noise = rest.lines().all(|l| {
            let t = l.trim();
            t.is_empty()
                || t.starts_with('#')
                || t == "pass"
                || t == "..."
                || t.starts_with("raise NotImplementedError")
        });
        if only_noise {
            return true;
        }
    }
    false
}

fn normalize_py_ws(s: &str) -> String {
    s.lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// `def name` or `async def name` at line start (ignoring leading spaces).
fn defines_python_fn(code: &str, name: &str) -> bool {
    let patterns = [
        format!("def {name}("),
        format!("def {name} ("),
        format!("async def {name}("),
        format!("async def {name} ("),
    ];
    for line in code.lines() {
        let t = line.trim_start();
        if patterns.iter().any(|p| t.starts_with(p.as_str())) {
            return true;
        }
    }
    false
}

/// True if the only meaningful body is raise NotImplementedError / pass / ellipsis.
/// Multi-line docstrings are skipped via an open/close delimiter state machine.
fn is_unimplemented_stub(code: &str, name: &str) -> bool {
    // Find the function block roughly: from `def name` until next top-level def/class or EOF.
    let mut in_fn = false;
    let mut body_lines: Vec<String> = Vec::new();
    let mut in_docstring: Option<&'static str> = None; // active delimiter """ or '''

    for line in code.lines() {
        let t = line.trim_start();
        if t.starts_with(&format!("def {name}("))
            || t.starts_with(&format!("def {name} ("))
            || t.starts_with(&format!("async def {name}("))
            || t.starts_with(&format!("async def {name} ("))
        {
            in_fn = true;
            body_lines.clear();
            in_docstring = None;
            continue;
        }
        if in_fn {
            if !line.starts_with(' ')
                && !line.starts_with('\t')
                && !line.trim().is_empty()
                && (t.starts_with("def ") || t.starts_with("class ") || t.starts_with("async def "))
            {
                break;
            }
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Docstring state machine: skip entire multi-line docstring interiors.
            if let Some(delim) = in_docstring {
                // Count delimiter occurrences on this line to detect close.
                if docstring_closes(trimmed, delim) {
                    in_docstring = None;
                }
                continue;
            }
            if let Some(delim) = starts_docstring(trimmed) {
                // Single-line docstring: open and close on same line.
                if !docstring_is_single_line(trimmed, delim) {
                    in_docstring = Some(delim);
                }
                continue;
            }

            body_lines.push(trimmed.to_string());
        }
    }
    if body_lines.is_empty() {
        return true;
    }
    body_lines.iter().all(|l| {
        l == "pass"
            || l == "..."
            || l == "raise NotImplementedError"
            || l == "raise NotImplementedError()"
            || l.starts_with("raise NotImplementedError")
    })
}

fn starts_docstring(trimmed: &str) -> Option<&'static str> {
    if trimmed.starts_with("\"\"\"") {
        Some("\"\"\"")
    } else if trimmed.starts_with("'''") {
        Some("'''")
    } else {
        None
    }
}

fn docstring_is_single_line(trimmed: &str, delim: &str) -> bool {
    // After the opening delimiter, is there a closing one on the same line?
    let rest = &trimmed[delim.len()..];
    rest.contains(delim)
}

fn docstring_closes(trimmed: &str, delim: &str) -> bool {
    // Closing when delimiter appears. For a pure closer line `"""` or content ending with it.
    trimmed.contains(delim)
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
        .rfind(|l| !l.is_empty())
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
        fs::write(
            d.path().join("agent_answer.txt"),
            "Filter data.csv where score>=50",
        )
        .unwrap();
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
    fn solution_file_grades_structurally() {
        let d = tempdir().unwrap();
        let mut it = item("solution_file", "Write similar_elements. Write solution.py");
        it.grade.kind = "solution_file".into();
        it.grade.answer_file = Some("solution.py".into());
        it.grade.tool_name = None;
        it.grade.expected_contains = None;
        it.meta = json!({
            "python_grade": {
                "mode": "mbpp",
                "test_list": [
                    "assert similar_elements((3, 4, 5, 6),(5, 7, 4, 10)) == (4, 5)"
                ],
                "test_setup_code": "",
                "entry_point": "similar_elements"
            }
        });
        // missing file
        let g0 = grade_catalog_item(&it, d.path(), "jewell");
        assert!(!g0.correct, "{}", g0.detail);
        // bare stub fails
        fs::write(
            d.path().join("solution.py"),
            "def similar_elements(*args, **kwargs):\n    raise NotImplementedError\n",
        )
        .unwrap();
        let g_stub = grade_catalog_item(&it, d.path(), "jewell");
        assert!(!g_stub.correct, "stub should fail: {}", g_stub.detail);
        // correct implementation should pass python3 asserts
        fs::write(
            d.path().join("solution.py"),
            "def similar_elements(a, b):\n    return tuple(sorted(set(a) & set(b)))\n",
        )
        .unwrap();
        let g1 = grade_catalog_item(&it, d.path(), "jewell");
        assert!(g1.correct, "{}", g1.detail);
        assert!((g1.score - 1.0).abs() < 1e-9, "score={}", g1.score);
    }

    #[test]
    fn solution_file_humaneval_multiline_docstring_stub_fails() {
        let d = tempdir().unwrap();
        // Official-style HE seed: signature + multi-line docstring, no body.
        let seed = r#"from typing import List


def has_close_elements(numbers: List[float], threshold: float) -> bool:
    """ Check if in given list of numbers, are any two numbers closer to each other than
    given threshold.
    >>> has_close_elements([1.0, 2.0, 3.0], 0.5)
    False
    >>> has_close_elements([1.0, 2.8, 3.0, 4.0, 5.0, 2.0], 0.3)
    True
    """
"#;
        let mut it = item(
            "solution_file",
            "Complete the Python function in solution.py\n\nStarter:\n```python\n",
        );
        it.prompt.push_str(seed);
        it.prompt
            .push_str("\n```\nOnly solution.py on disk is graded.");
        it.grade.kind = "solution_file".into();
        it.grade.answer_file = Some("solution.py".into());
        it.grade.tool_name = None;
        it.grade.expected_contains = None;
        it.workspace_files
            .insert("solution.py".into(), seed.to_string());
        it.meta = json!({
            "python_grade": {
                "mode": "humaneval",
                "entry_point": "has_close_elements",
                "test": "def check(candidate):\n    assert candidate([1.0, 2.0, 3.9], 0.5) == False\n"
            }
        });
        fs::write(d.path().join("solution.py"), seed).unwrap();
        let g = grade_catalog_item(&it, d.path(), "jewell");
        assert!(!g.correct, "HE seed must fail: {}", g.detail);
        assert!(
            g.detail.contains("seed") || g.detail.contains("stub") || g.detail.contains("echo"),
            "detail should mention seed/stub: {}",
            g.detail
        );
        // Docstring-only body is an unimplemented stub even without seed map.
        it.workspace_files.clear();
        let g2 = grade_catalog_item(&it, d.path(), "jewell");
        assert!(!g2.correct, "docstring-only stub must fail: {}", g2.detail);
    }

    #[test]
    fn solution_file_seed_equality_fails() {
        let d = tempdir().unwrap();
        let seed =
            "def foo(x):\n    \"\"\"Implement this function.\"\"\"\n    raise NotImplementedError\n";
        let mut it = item("solution_file", "Implement foo in solution.py");
        it.grade.kind = "solution_file".into();
        it.grade.answer_file = Some("solution.py".into());
        it.grade.tool_name = None;
        it.grade.expected_contains = None;
        it.workspace_files
            .insert("solution.py".into(), seed.to_string());
        it.meta = json!({
            "python_grade": {
                "mode": "mbpp",
                "entry_point": "foo",
                "test_list": ["assert foo(1) == 2"],
                "test_setup_code": ""
            }
        });
        fs::write(d.path().join("solution.py"), seed).unwrap();
        let g = grade_catalog_item(&it, d.path(), "jewell");
        assert!(!g.correct, "seed equality must fail: {}", g.detail);
        assert!(
            g.detail.contains("seed") || g.detail.contains("stub"),
            "{}",
            g.detail
        );
    }

    #[test]
    fn solution_file_python3_known_good_and_bad() {
        let d = tempdir().unwrap();
        let mut it = item("solution_file", "Implement add in solution.py");
        it.grade.kind = "solution_file".into();
        it.grade.answer_file = Some("solution.py".into());
        it.grade.tool_name = None;
        it.grade.expected_contains = None;
        it.meta = json!({
            "python_grade": {
                "mode": "mbpp",
                "entry_point": "add",
                "test_list": ["assert add(1, 2) == 3", "assert add(0, 0) == 0"],
                "test_setup_code": ""
            }
        });
        // known-bad
        fs::write(
            d.path().join("solution.py"),
            "def add(a, b):\n    return a - b\n",
        )
        .unwrap();
        let bad = grade_catalog_item(&it, d.path(), "jewell");
        assert!(!bad.correct, "wrong body must fail asserts: {}", bad.detail);
        // known-good
        fs::write(
            d.path().join("solution.py"),
            "def add(a, b):\n    return a + b\n",
        )
        .unwrap();
        let good = grade_catalog_item(&it, d.path(), "jewell");
        assert!(good.correct, "correct body must pass: {}", good.detail);
        assert!((good.score - 1.0).abs() < 1e-9);
        assert_eq!(
            good.metrics.get("grader").and_then(|v| v.as_str()),
            Some("python3")
        );
    }

    #[test]
    fn solution_file_humaneval_python3_check() {
        let d = tempdir().unwrap();
        let seed = r#"def has_close_elements(numbers, threshold):
    """stub"""
"#;
        let mut it = item("solution_file", "Complete has_close_elements");
        it.grade.kind = "solution_file".into();
        it.grade.answer_file = Some("solution.py".into());
        it.grade.tool_name = None;
        it.grade.expected_contains = None;
        it.workspace_files
            .insert("solution.py".into(), seed.to_string());
        it.meta = json!({
            "python_grade": {
                "mode": "humaneval",
                "entry_point": "has_close_elements",
                "test": "def check(candidate):\n    assert candidate([1.0, 2.0, 3.0], 0.5) is False\n    assert candidate([1.0, 2.8, 3.0, 4.0, 5.0, 2.0], 0.3) is True\n"
            }
        });
        // good implementation
        fs::write(
            d.path().join("solution.py"),
            r#"def has_close_elements(numbers, threshold):
    for i, a in enumerate(numbers):
        for b in numbers[i+1:]:
            if abs(a - b) < threshold:
                return True
    return False
"#,
        )
        .unwrap();
        let g = grade_catalog_item(&it, d.path(), "jewell");
        assert!(g.correct, "HE check should pass: {}", g.detail);
        // bad implementation
        fs::write(
            d.path().join("solution.py"),
            "def has_close_elements(numbers, threshold):\n    return False\n",
        )
        .unwrap();
        let bad = grade_catalog_item(&it, d.path(), "jewell");
        assert!(!bad.correct, "wrong HE solution must fail: {}", bad.detail);
    }

    #[test]
    fn is_unimplemented_stub_skips_multiline_docstring() {
        let code = r#"def has_close_elements(numbers, threshold):
    """ Check if in given list of numbers, are any two numbers closer to each other than
    given threshold.
    >>> has_close_elements([1.0, 2.0, 3.0], 0.5)
    False
    """
"#;
        assert!(
            is_unimplemented_stub(code, "has_close_elements"),
            "docstring-only body is a stub"
        );
        let code2 = r#"def has_close_elements(numbers, threshold):
    """doc"""
    return True
"#;
        assert!(!is_unimplemented_stub(code2, "has_close_elements"));
    }

    #[test]
    fn solution_file_structural_only_never_full_credit() {
        let d = tempdir().unwrap();
        let mut it = item("solution_file", "Implement foo in solution.py");
        it.grade.kind = "solution_file".into();
        it.grade.answer_file = Some("solution.py".into());
        it.grade.tool_name = None;
        it.grade.expected_contains = None;
        // Valid non-stub body, but no python_grade tests in meta.
        it.meta = json!({
            "python_grade": {
                "mode": "mbpp",
                "entry_point": "foo"
            }
        });
        fs::write(
            d.path().join("solution.py"),
            "def foo(x):\n    return x + 1\n",
        )
        .unwrap();
        let g = grade_catalog_item(&it, d.path(), "jewell");
        assert!(
            !g.correct,
            "structural-only must not be correct: {}",
            g.detail
        );
        assert!(
            g.score <= 0.5,
            "structural-only score must be <= 0.5, got {}",
            g.score
        );
        assert!(
            g.detail.contains("structural smoke"),
            "detail should say structural smoke: {}",
            g.detail
        );
        assert_eq!(
            g.metrics.get("grader").and_then(|v| v.as_str()),
            Some("structural_smoke")
        );
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
