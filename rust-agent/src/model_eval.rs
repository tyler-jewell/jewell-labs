//! Project micro-eval for model comparison (OpenAI-compatible local servers).
//! Scores are per-model inference results — not file metadata theater.

use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;

/// Fixed project microbench (must stay small, deterministic, local).
pub const PROJECT_MICRO_CASES: &[(&str, &str)] = &[
    ("Reply with only the single digit for 2+2.", "4"),
    ("Reply with only the single digit for 7-3.", "4"),
    ("What is 15-8? Reply with only the number.", "7"),
    (
        "Spell the number after nine as one lowercase word only.",
        "ten",
    ),
    ("Is 11 prime? Reply yes or no only.", "yes"),
];

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProjectEvalScore {
    pub score: f64,
    pub correct: usize,
    pub total: usize,
    pub model_requested: String,
    pub model_served: String,
    pub base_url: String,
    pub ok: bool,
    pub error: Option<String>,
    pub case_hits: Vec<bool>,
}

impl ProjectEvalScore {
    pub fn unavailable(target: &str, err: impl Into<String>) -> Self {
        Self {
            score: 0.0,
            correct: 0,
            total: PROJECT_MICRO_CASES.len(),
            model_requested: target.into(),
            model_served: String::new(),
            base_url: String::new(),
            ok: false,
            error: Some(err.into()),
            case_hits: vec![],
        }
    }
}

fn hit(expect: &str, text: &str) -> bool {
    let t = text.trim().to_ascii_lowercase();
    if expect.chars().all(|c| c.is_ascii_digit()) {
        let digits: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
        return digits.contains(expect);
    }
    t == expect || t.starts_with(expect) || t.contains(expect)
}

/// Score one model id on a base_url (blocking HTTP).
pub fn score_project_eval(base_url: &str, model: &str) -> ProjectEvalScore {
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(90))
        .build()
    {
        Ok(c) => c,
        Err(e) => return ProjectEvalScore::unavailable(model, e.to_string()),
    };
    // server up?
    let models_url = format!("{}/v1/models", base_url.trim_end_matches('/'));
    if client
        .get(&models_url)
        .send()
        .map(|r| r.status().is_success())
        .unwrap_or(false)
        == false
    {
        return ProjectEvalScore::unavailable(model, format!("server down: {base_url}"));
    }

    let mut correct = 0usize;
    let mut hits = Vec::new();
    let mut served = String::new();
    for (prompt, expect) in PROJECT_MICRO_CASES {
        let body = json!({
            "model": model,
            "messages": [{"role":"user","content": prompt}],
            "max_tokens": 24,
            "temperature": 0.0,
            "stream": false,
        });
        let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
        let resp = match client.post(&url).json(&body).send() {
            Ok(r) => r,
            Err(e) => return ProjectEvalScore::unavailable(model, e.to_string()),
        };
        if !resp.status().is_success() {
            let st = resp.status();
            let t = resp.text().unwrap_or_default();
            return ProjectEvalScore::unavailable(model, format!("http {st}: {t}"));
        }
        let v: Value = match resp.json() {
            Ok(v) => v,
            Err(e) => return ProjectEvalScore::unavailable(model, e.to_string()),
        };
        if let Some(m) = v.get("model").and_then(|m| m.as_str()) {
            served = m.to_string();
        }
        let text = v
            .pointer("/choices/0/message/content")
            .and_then(|c| c.as_str())
            .unwrap_or("");
        let h = hit(expect, text);
        hits.push(h);
        if h {
            correct += 1;
        }
    }
    let total = PROJECT_MICRO_CASES.len();
    ProjectEvalScore {
        score: correct as f64 / total as f64,
        correct,
        total,
        model_requested: model.into(),
        model_served: served,
        base_url: base_url.into(),
        ok: true,
        error: None,
        case_hits: hits,
    }
}

/// Resolve how to score a target: `ollama:name` or filesystem GGUF path.
pub fn score_inference_target(target: &str) -> ProjectEvalScore {
    if let Some(id) = target.strip_prefix("ollama:") {
        return score_project_eval("http://127.0.0.1:11434", id);
    }
    let path = Path::new(target);
    if !path.is_file() {
        return ProjectEvalScore::unavailable(target, "weights file missing");
    }
    // Prefer llama-server if it is serving this exact path.
    let llama = "http://127.0.0.1:8080";
    if let Some(loaded) = loaded_llama_model(llama) {
        let path_s = path.display().to_string();
        if loaded == path_s
            || loaded.ends_with(path.file_name().and_then(|s| s.to_str()).unwrap_or(""))
        {
            return score_project_eval(llama, &loaded);
        }
        return ProjectEvalScore::unavailable(
            target,
            format!("llama-server loaded different model: {loaded}"),
        );
    }
    ProjectEvalScore::unavailable(target, "llama-server unreachable; cannot score GGUF")
}

fn loaded_llama_model(base_url: &str) -> Option<String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .ok()?;
    let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
    let v: Value = client.get(url).send().ok()?.json().ok()?;
    // llama.cpp: data[0].id or models[0].model
    if let Some(id) = v
        .pointer("/data/0/id")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
    {
        return Some(id);
    }
    v.pointer("/models/0/model")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_digits_and_words() {
        assert!(hit("4", "the answer is 4."));
        assert!(hit("yes", "Yes."));
        assert!(!hit("4", "five"));
    }
}
