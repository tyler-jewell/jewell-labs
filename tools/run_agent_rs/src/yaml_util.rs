//! Frontmatter + registry YAML helpers for the minimal agent runner.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub fn parse_frontmatter(text: &str) -> (HashMap<String, Value>, String) {
    if !text.starts_with("---") {
        return (HashMap::new(), text.to_string());
    }
    let parts: Vec<&str> = text.splitn(3, "---").collect();
    if parts.len() < 3 {
        return (HashMap::new(), text.to_string());
    }
    let raw = parts[1];
    let body = parts[2].trim_start_matches('\n').to_string();
    let mut data: HashMap<String, Value> = HashMap::new();
    let mut root = Value::Object(serde_json::Map::new());
    let mut path_stack: Vec<(usize, Vec<String>)> = vec![(0, vec![])];

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let Some((key, rest)) = line.trim_start().split_once(':') else {
            continue;
        };
        let key = key.trim().to_string();
        let val = rest.trim();
        while path_stack.len() > 1 && indent <= path_stack.last().unwrap().0 {
            path_stack.pop();
        }
        let parent_path = path_stack.last().unwrap().1.clone();
        if val.is_empty() {
            let mut full = parent_path.clone();
            full.push(key.clone());
            set_path(&mut root, &full, Value::Object(serde_json::Map::new()));
            path_stack.push((indent, full));
        } else {
            let mut full = parent_path;
            full.push(key);
            set_path(&mut root, &full, parse_scalar(val));
        }
    }
    if let Value::Object(map) = root {
        for (k, v) in map {
            data.insert(k, v);
        }
    }
    (data, body)
}

fn parse_scalar(val: &str) -> Value {
    let val = val.trim();
    let val = if (val.starts_with('"') && val.ends_with('"'))
        || (val.starts_with('\'') && val.ends_with('\''))
    {
        &val[1..val.len() - 1]
    } else {
        val
    };
    match val {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        "null" => Value::Null,
        _ => {
            if let Ok(i) = val.parse::<i64>() {
                json!(i)
            } else if let Ok(f) = val.parse::<f64>() {
                json!(f)
            } else {
                Value::String(val.to_string())
            }
        }
    }
}

fn set_path(root: &mut Value, path: &[String], val: Value) {
    if path.is_empty() {
        return;
    }
    if !root.is_object() {
        *root = Value::Object(serde_json::Map::new());
    }
    let mut cur = root.as_object_mut().unwrap();
    for (i, k) in path.iter().enumerate() {
        if i + 1 == path.len() {
            cur.insert(k.clone(), val);
            return;
        }
        let entry = cur
            .entry(k.clone())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        if !entry.is_object() {
            *entry = Value::Object(serde_json::Map::new());
        }
        cur = entry.as_object_mut().unwrap();
    }
}

pub fn load_registry(path: &Path) -> HashMap<String, Value> {
    let text = fs::read_to_string(path).unwrap_or_default();
    let mut models = HashMap::new();
    let mut cur: Option<String> = None;
    let mut in_defaults = false;
    for line in text.lines() {
        if let Some(caps) = regex_model(line) {
            cur = Some(caps);
            models.insert(cur.clone().unwrap(), Value::Object(serde_json::Map::new()));
            in_defaults = false;
            continue;
        }
        let Some(ref c) = cur else { continue };
        if line.trim() == "defaults:" || line.starts_with("    defaults:") {
            if let Some(Value::Object(m)) = models.get_mut(c) {
                m.insert("defaults".into(), Value::Object(serde_json::Map::new()));
            }
            in_defaults = true;
            continue;
        }
        if !in_defaults {
            if let Some((k, v)) = field_line(line, 4) {
                if let Some(Value::Object(m)) = models.get_mut(c) {
                    m.insert(k, Value::String(v));
                }
            }
        } else if let Some((k, v)) = field_line(line, 6) {
            if let Some(Value::Object(m)) = models.get_mut(c) {
                let defs = m
                    .entry("defaults".to_string())
                    .or_insert_with(|| Value::Object(serde_json::Map::new()));
                if let Value::Object(d) = defs {
                    d.insert(k, parse_scalar(&v));
                }
            }
        }
    }
    models
}

fn regex_model(line: &str) -> Option<String> {
    let line = line.strip_suffix(':')?;
    if line.starts_with("  ") && !line.starts_with("   ") {
        let name = line.trim();
        if name == "models" {
            return None;
        }
        if name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
            return Some(name.to_string());
        }
    }
    None
}

fn field_line(line: &str, spaces: usize) -> Option<(String, String)> {
    if spaces == 4 && !(line.starts_with("    ") && !line.starts_with("     ")) {
        return None;
    }
    if spaces == 6 && !line.starts_with("      ") {
        return None;
    }
    let rest = line.trim_start();
    let (k, v) = rest.split_once(':')?;
    Some((
        k.trim().to_string(),
        v.trim()
            .trim_matches(|c| c == '"' || c == '\'')
            .to_string(),
    ))
}
