//! llama-server ensure + chat helpers.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

pub fn expand_home(p: &str) -> String {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Ok(home) = env::var("HOME") {
            return format!("{home}/{rest}");
        }
    }
    p.to_string()
}

pub fn resolve_model(key: &str, reg: &HashMap<String, Value>) -> Result<Value, String> {
    if let Some(spec) = reg.get(key) {
        let mut out = spec.clone();
        if let Value::Object(m) = &mut out {
            m.insert("key".into(), Value::String(key.into()));
            if let Some(Value::String(p)) = m.get("path").cloned() {
                m.insert("path".into(), Value::String(expand_home(&p)));
            }
        }
        return Ok(out);
    }
    let path = expand_home(key);
    if path.ends_with(".gguf") && Path::new(&path).exists() {
        return Ok(json!({
            "key": key,
            "path": path,
            "alias": Path::new(&path).file_stem().and_then(|s| s.to_str()).unwrap_or("local"),
            "defaults": {}
        }));
    }
    Err(format!("unknown model '{key}'"))
}

pub fn as_str(v: Option<&Value>, def: &str) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => def.to_string(),
    }
}

pub fn as_i64(v: Option<&Value>, def: i64) -> i64 {
    match v {
        Some(Value::Number(n)) => n.as_i64().unwrap_or(def),
        Some(Value::String(s)) => s.parse().unwrap_or(def),
        _ => def,
    }
}

pub fn as_f64(v: Option<&Value>, def: f64) -> f64 {
    match v {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(def),
        Some(Value::String(s)) => s.parse().unwrap_or(def),
        _ => def,
    }
}

fn server_up(base: &str) -> bool {
    ureq::get(&format!("{base}/v1/models"))
        .timeout(Duration::from_secs(1))
        .call()
        .map(|r| r.status() == 200)
        .unwrap_or(false)
}

pub fn ensure_server(
    path: &str,
    host: &str,
    port: i64,
    ctx: i64,
    reasoning: &str,
) -> Result<String, String> {
    let base = format!("http://{host}:{port}");
    if server_up(&base) {
        return Ok(base);
    }
    let log = fs::File::create("/tmp/run-agent-llama.log").map_err(|e| e.to_string())?;
    Command::new("llama-server")
        .args([
            "-m",
            path,
            "--host",
            host,
            "--port",
            &port.to_string(),
            "-c",
            &ctx.to_string(),
            "-np",
            "1",
            "--ctx-checkpoints",
            "0",
            "--reasoning",
            reasoning,
        ])
        .stdout(Stdio::from(log.try_clone().map_err(|e| e.to_string())?))
        .stderr(Stdio::from(log))
        .spawn()
        .map_err(|e| e.to_string())?;
    for _ in 0..60 {
        if server_up(&base) {
            return Ok(base);
        }
        thread::sleep(Duration::from_millis(500));
    }
    Err("llama-server failed to become ready".into())
}

pub fn chat(
    base: &str,
    model: &str,
    system: &str,
    user: &str,
    temperature: f64,
    max_tokens: i64,
) -> Result<String, String> {
    let body = json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user}
        ],
        "temperature": temperature,
        "max_tokens": max_tokens
    });
    let resp = ureq::post(&format!("{base}/v1/chat/completions"))
        .set("Content-Type", "application/json")
        .send_string(&body.to_string())
        .map_err(|e| e.to_string())?;
    let mut s = String::new();
    resp.into_reader()
        .read_to_string(&mut s)
        .map_err(|e| e.to_string())?;
    let data: Value = serde_json::from_str(&s).map_err(|e| e.to_string())?;
    let msg = &data["choices"][0]["message"];
    let content = msg["content"].as_str().unwrap_or("");
    if !content.is_empty() {
        return Ok(content.to_string());
    }
    Ok(msg["reasoning_content"].as_str().unwrap_or("").to_string())
}
