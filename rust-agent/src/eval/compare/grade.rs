//! Post-hoc graders in Rust (no Python runner). Coding behavioral checks may
//! shell to `python3` only as an external grade process when available.

use serde_json::{json, Map, Value};
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct Grade {
    pub correct: bool,
    pub score: f64,
    pub metrics: Map<String, Value>,
    pub detail: String,
    pub grader_ok: bool,
}

impl Grade {
    pub fn to_json(&self) -> Value {
        json!({
            "correct": self.correct,
            "score": self.score,
            "metrics": self.metrics,
            "detail": self.detail,
            "grader_ok": self.grader_ok,
        })
    }
}

/// Grade workspace for a known task id (pure Rust + optional python3 for coding).
pub fn grade_task(task_id: &str, workspace: &Path, task_path: &Path) -> Grade {
    match task_id {
        "closed_form_ratio" => grade_closed_form(workspace),
        "multi_step_report" => grade_multi_step(workspace),
        "agent_os_introspection" | "agent_os_team" => grade_host_eval(workspace),
        "coding_fix_bug" => grade_sum_range(workspace),
        "coding_fizzbuzz" => grade_fizzbuzz(workspace),
        _ => grade_legacy_check_py(task_path, workspace),
    }
}

fn grade_closed_form(ws: &Path) -> Grade {
    let mut text = String::new();
    for name in ["agent_answer.txt", "agent_transcript.txt"] {
        let p = ws.join(name);
        if p.is_file() {
            text = fs::read_to_string(p).unwrap_or_default();
            break;
        }
    }
    if text.is_empty() {
        return Grade {
            correct: false,
            score: 0.0,
            metrics: Map::new(),
            detail: "no agent_answer.txt".into(),
            grader_ok: true,
        };
    }
    let re = regex_lite_boxed(&text);
    let extracted = if let Some(s) = re {
        s
    } else {
        // last integer
        let nums: Vec<&str> = text
            .split(|c: char| !c.is_ascii_digit() && c != '-')
            .filter(|s| !s.is_empty() && s.chars().any(|c| c.is_ascii_digit()))
            .collect();
        nums.last().map(|s| s.to_string()).unwrap_or_default()
    };
    let extracted_n: String = extracted
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    let gold = "18";
    let correct = extracted_n == gold || extracted.trim() == gold;
    let mut metrics = Map::new();
    metrics.insert("extracted".into(), json!(extracted));
    metrics.insert("gold".into(), json!(gold));
    Grade {
        correct,
        score: if correct { 1.0 } else { 0.0 },
        metrics,
        detail: format!("extracted={extracted:?}"),
        grader_ok: true,
    }
}

/// Last non-empty `\boxed{...}`.
fn regex_lite_boxed(text: &str) -> Option<String> {
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

fn grade_multi_step(ws: &Path) -> Grade {
    let notes = ws.join("notes.md");
    let summary = ws.join("summary.txt");
    if !notes.is_file() || !summary.is_file() {
        return Grade {
            correct: false,
            score: 0.0,
            metrics: Map::new(),
            detail: format!("notes={} summary={}", notes.is_file(), summary.is_file()),
            grader_ok: true,
        };
    }
    let ntext = fs::read_to_string(&notes).unwrap_or_default();
    let bullets = ntext
        .lines()
        .filter(|ln| ln.trim_start().starts_with('-'))
        .count();
    let lower = ntext.to_ascii_lowercase();
    let words_ok = ["alpha", "beta", "gamma"].iter().all(|w| lower.contains(w));
    let bullets_ok = bullets >= 3;
    let stext = fs::read_to_string(&summary).unwrap_or_default();
    let sum_ok = stext.trim() == "topics: alpha, beta, gamma";
    let score = (if bullets_ok { 0.4 } else { 0.0 })
        + (if words_ok { 0.3 } else { 0.0 })
        + (if sum_ok { 0.3 } else { 0.0 });
    let mut metrics = Map::new();
    metrics.insert("bullets".into(), json!(bullets));
    metrics.insert("words_ok".into(), json!(words_ok));
    metrics.insert("sum_ok".into(), json!(sum_ok));
    Grade {
        correct: bullets_ok && words_ok && sum_ok,
        score,
        metrics,
        detail: if score >= 1.0 {
            "ok".into()
        } else {
            format!("bullets={bullets} words={words_ok} sum={sum_ok}")
        },
        grader_ok: true,
    }
}

fn grade_host_eval(ws: &Path) -> Grade {
    if ws.join("host_eval_ok.txt").is_file() {
        return Grade {
            correct: true,
            score: 1.0,
            metrics: Map::new(),
            detail: "dry marker".into(),
            grader_ok: true,
        };
    }
    let code_path = ws.join("host_exit_code.txt");
    if !code_path.is_file() {
        return Grade {
            correct: false,
            score: 0.0,
            metrics: Map::new(),
            detail: "no host_exit_code".into(),
            grader_ok: true,
        };
    }
    let code: i32 = fs::read_to_string(code_path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(-1);
    let mut metrics = Map::new();
    metrics.insert("exit_code".into(), json!(code));
    Grade {
        correct: code == 0,
        score: if code == 0 { 1.0 } else { 0.0 },
        metrics,
        detail: format!("host exit={code}"),
        grader_ok: true,
    }
}

fn grade_sum_range(ws: &Path) -> Grade {
    let path = ws.join("sum_range.py");
    if !path.is_file() {
        return Grade {
            correct: false,
            score: 0.0,
            metrics: Map::new(),
            detail: "missing sum_range.py".into(),
            grader_ok: true,
        };
    }
    // Prefer behavioral grade via python3
    if let Some(g) = python_grade_sum_range(&path) {
        return g;
    }
    // Structural fallback: inclusive loop
    let src = fs::read_to_string(&path).unwrap_or_default();
    let inclusive = src.contains("while x <= hi")
        || src.contains("while x<=hi")
        || src.contains("range(lo, hi + 1)")
        || src.contains("range(lo, hi+1)");
    Grade {
        correct: inclusive,
        score: if inclusive { 1.0 } else { 0.0 },
        metrics: Map::new(),
        detail: if inclusive {
            "structural inclusive loop (python3 unavailable)".into()
        } else {
            "structural: missing inclusive bound".into()
        },
        grader_ok: true,
    }
}

fn python_grade_sum_range(path: &Path) -> Option<Grade> {
    let py = format!(
        r#"
import importlib.util, json, sys
path = {path:?}
spec = importlib.util.spec_from_file_location("sum_range_ut", path)
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)
cases = [((1, 1), 1), ((1, 3), 6), ((0, 10), 55), ((-2, 2), 0)]
ok = 0
for (lo, hi), want in cases:
    try:
        got = mod.sum_range(lo, hi)
    except Exception:
        got = None
    if got == want:
        ok += 1
score = ok / len(cases)
print(json.dumps({{"correct": ok == len(cases), "score": score, "metrics": {{"cases_ok": ok, "cases_total": len(cases)}}, "detail": f"{{ok}}/{{len(cases)}} cases"}}))
"#,
        path = path.display().to_string()
    );
    let out = Command::new("python3").arg("-c").arg(&py).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout.lines().last()?;
    let v: Value = serde_json::from_str(line).ok()?;
    Some(json_to_grade(v))
}

fn grade_fizzbuzz(ws: &Path) -> Grade {
    let path = ws.join("fizzbuzz.py");
    if !path.is_file() {
        return Grade {
            correct: false,
            score: 0.0,
            metrics: Map::new(),
            detail: "missing fizzbuzz.py".into(),
            grader_ok: true,
        };
    }
    if let Some(g) = python_grade_fizzbuzz(&path) {
        return g;
    }
    // Structural-only: never mark correct without behavioral python3 grade.
    let src = fs::read_to_string(&path).unwrap_or_default();
    let ok = src.contains("def fizzbuzz") && src.contains("FizzBuzz");
    Grade {
        correct: false,
        score: if ok { 0.5 } else { 0.0 },
        metrics: Map::new(),
        detail: if ok {
            "structural fizzbuzz only (python3 unavailable); correct=false".into()
        } else {
            "no fizzbuzz def".into()
        },
        grader_ok: true,
    }
}

fn python_grade_fizzbuzz(path: &Path) -> Option<Grade> {
    let py = format!(
        r#"
import importlib.util, json, subprocess, sys
path = {path:?}
spec = importlib.util.spec_from_file_location("fizzbuzz_ut", path)
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)
want = ["1","2","Fizz","4","Buzz","Fizz","7","8","Fizz","Buzz","11","Fizz","13","14","FizzBuzz"]
fn_ok = hasattr(mod, "fizzbuzz") and mod.fizzbuzz(15) == want
main_ok = False
try:
    proc = subprocess.run([sys.executable, path], capture_output=True, text=True, timeout=10)
    main_ok = proc.returncode == 0 and proc.stdout.strip().splitlines() == want
except Exception:
    main_ok = False
score = (0.7 if fn_ok else 0.0) + (0.3 if main_ok else 0.0)
print(json.dumps({{"correct": fn_ok and main_ok, "score": score, "metrics": {{"fn_ok": fn_ok, "main_ok": main_ok}}, "detail": "fizzbuzz(15)" if fn_ok and main_ok else f"fn_ok={{fn_ok}} main_ok={{main_ok}}"}}))
"#,
        path = path.display().to_string()
    );
    let out = Command::new("python3").arg("-c").arg(&py).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout.lines().last()?;
    let v: Value = serde_json::from_str(line).ok()?;
    Some(json_to_grade(v))
}

fn grade_legacy_check_py(task_path: &Path, workspace: &Path) -> Grade {
    let check = task_path.join("tests").join("check.py");
    if !check.is_file() {
        return Grade {
            correct: false,
            score: 0.0,
            metrics: Map::new(),
            detail: format!("no grader for {}", task_path.display()),
            grader_ok: false,
        };
    }
    let out = Command::new("python3").arg(&check).arg(workspace).output();
    match out {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            for line in stdout.lines().rev() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if let Ok(v) = serde_json::from_str::<Value>(line) {
                    return json_to_grade(v);
                }
            }
            Grade {
                correct: false,
                score: 0.0,
                metrics: Map::new(),
                detail: "grader non-JSON".into(),
                grader_ok: false,
            }
        }
        Err(e) => Grade {
            correct: false,
            score: 0.0,
            metrics: Map::new(),
            detail: format!("grader spawn: {e}"),
            grader_ok: false,
        },
    }
}

fn json_to_grade(v: Value) -> Grade {
    let correct = v.get("correct").and_then(|x| x.as_bool()).unwrap_or(false);
    let score = v
        .get("score")
        .and_then(|x| x.as_f64())
        .unwrap_or(if correct { 1.0 } else { 0.0 });
    let metrics = v
        .get("metrics")
        .and_then(|m| m.as_object())
        .cloned()
        .unwrap_or_default();
    let detail = v
        .get("detail")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    Grade {
        correct,
        score,
        metrics,
        detail,
        grader_ok: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn closed_form_extracts_last_boxed() {
        let d = tempdir().unwrap();
        let mut f = fs::File::create(d.path().join("agent_answer.txt")).unwrap();
        writeln!(f, r"noise \boxed{{}} then \boxed{{18}}").unwrap();
        let g = grade_closed_form(d.path());
        assert!(g.correct, "{}", g.detail);
        assert!((g.score - 1.0).abs() < 1e-9);
    }

    #[test]
    fn multi_step_gold() {
        let d = tempdir().unwrap();
        fs::write(d.path().join("notes.md"), "- alpha\n- beta\n- gamma\n").unwrap();
        fs::write(d.path().join("summary.txt"), "topics: alpha, beta, gamma\n").unwrap();
        let g = grade_multi_step(d.path());
        assert!(g.correct);
    }

    #[test]
    fn host_dry_marker() {
        let d = tempdir().unwrap();
        fs::write(d.path().join("host_eval_ok.txt"), "ok\n").unwrap();
        assert!(grade_host_eval(d.path()).correct);
    }
}
