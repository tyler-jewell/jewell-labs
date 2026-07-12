use crate::registry::ModelRegistry;
use crate::tools::{ToolContext, ToolError, ToolSpec};
use serde_json::{json, Value};

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "list_models".into(),
        category: "introspect".into(),
        description: "List models from models/registry.yaml (keys, aliases, paths, defaults)."
            .into(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }),
    }
}

pub fn run(ctx: &ToolContext, _args: &Value) -> Result<Value, ToolError> {
    let reg = ModelRegistry::load(&ctx.registry_path).map_err(|e| ToolError::Msg(e.to_string()))?;
    let mut models = Vec::new();
    for (key, spec) in reg.models.iter() {
        models.push(json!({
            "key": key,
            "alias": spec.alias,
            "path": spec.path,
            "defaults": {
                "ctx": spec.defaults.ctx,
                "reasoning": spec.defaults.reasoning,
            }
        }));
    }
    models.sort_by(|a, b| {
        a["key"]
            .as_str()
            .unwrap_or("")
            .cmp(b["key"].as_str().unwrap_or(""))
    });
    Ok(json!({ "models": models }))
}
