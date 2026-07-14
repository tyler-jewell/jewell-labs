//! Minimal agent runner (Rust): agents/{name}.md → llama-server chat.

mod server;
mod yaml_util;

use serde_json::json;
use std::env;
use std::fs;
use std::path::PathBuf;

fn root_dir() -> PathBuf {
    if let Ok(wd) = env::current_dir() {
        if wd.join("agents").is_dir() {
            return wd;
        }
        if wd.join("..").join("agents").is_dir() {
            return wd.join("..").canonicalize().unwrap_or(wd);
        }
    }
    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut dry_parse = false;
    let mut serve_only = false;
    let mut model_override: Option<String> = None;
    let mut positional: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--dry-parse" => dry_parse = true,
            "--serve-only" => serve_only = true,
            "--model" => {
                i += 1;
                if i < args.len() {
                    model_override = Some(args[i].clone());
                }
            }
            a if a.starts_with('-') => {
                eprintln!("unknown flag: {a}");
                std::process::exit(1);
            }
            a => positional.push(a.to_string()),
        }
        i += 1;
    }
    if positional.is_empty() {
        eprintln!(
            "usage: run_agent_rs <agent> [prompt] [--dry-parse] [--serve-only] [--model KEY]"
        );
        std::process::exit(1);
    }
    let agent = &positional[0];
    let prompt = if positional.len() > 1 {
        positional[1..].join(" ")
    } else {
        r"What is 2+2? Put answer in \boxed{}.".to_string()
    };

    let root = root_dir();
    let agent_path = root.join("agents").join(format!("{agent}.md"));
    let raw = fs::read_to_string(&agent_path).unwrap_or_else(|_| {
        eprintln!("missing {}", agent_path.display());
        std::process::exit(1);
    });
    let (fm, body) = yaml_util::parse_frontmatter(&raw);
    let model_key = model_override
        .or_else(|| fm.get("model").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .or_else(|| {
            fm.get("default_model")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| {
            eprintln!("missing default_model");
            std::process::exit(1);
        });

    let reg = yaml_util::load_registry(&root.join("models/registry.yaml"));
    let spec = server::resolve_model(&model_key, &reg).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });

    let server_cfg = fm.get("server").cloned().unwrap_or(json!({}));
    let sampling = fm.get("sampling").cloned().unwrap_or(json!({}));
    let defaults = spec.get("defaults").cloned().unwrap_or(json!({}));

    let host = server::as_str(server_cfg.get("host"), "127.0.0.1");
    let port = server::as_i64(server_cfg.get("port"), 8080);
    let ctx = server::as_i64(server_cfg.get("ctx"), server::as_i64(defaults.get("ctx"), 2048));
    let reasoning = server::as_str(
        server_cfg.get("reasoning"),
        &server::as_str(defaults.get("reasoning"), "off"),
    );
    let temp = server::as_f64(sampling.get("temperature"), 0.0);
    let max_tokens = server::as_i64(sampling.get("max_tokens"), 256);
    let path = server::as_str(spec.get("path"), "");
    let alias = server::as_str(spec.get("alias"), "local");

    if dry_parse {
        println!(
            "{}",
            json!({
                "agent": agent,
                "model_key": model_key,
                "path": path,
                "system_chars": body.len(),
                "port": port,
            })
        );
        return;
    }

    let base = server::ensure_server(&path, &host, port, ctx, &reasoning).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });
    if serve_only {
        println!("{base}");
        return;
    }
    match server::chat(&base, &alias, &body, &prompt, temp, max_tokens) {
        Ok(t) => println!("{t}"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
