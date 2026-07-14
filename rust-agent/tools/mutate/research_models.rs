//! Periodic model research: record HF/benchmark sources + local gate comparison.

use crate::model_research::{
    run_model_research, write_research_report, ModelResearchRequest, BENCHMARK_SOURCES,
};
use crate::paths::registry_path;
use crate::tools::types::{arg_bool, arg_str, ToolError, ToolSpec};
use crate::tools::ToolContext;
use serde_json::{json, Value};

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "research_models".into(),
        category: "mutate".into(),
        description: "Run model research job: document public benchmark sources, compare candidate to registry baseline on local gates; promote only with evidence (apply=true).".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "candidate_id": {"type": "string"},
                "candidate_path": {"type": "string", "description": "GGUF path or ollama:model"},
                "baseline_path": {"type": "string", "description": "optional override; GGUF path or ollama:model"},
                "offline": {"type": "boolean"},
                "apply": {"type": "boolean"},
                "list_sources_only": {"type": "boolean"}
            },
            "required": []
        }),
    }
}

pub fn run(ctx: &ToolContext, args: &Value) -> Result<Value, ToolError> {
    if arg_bool(args, "list_sources_only", false) {
        let sources: Vec<_> = BENCHMARK_SOURCES
            .iter()
            .map(|(id, name, url)| json!({"id": id, "name": name, "url": url}))
            .collect();
        return Ok(json!({ "sources": sources }));
    }

    let candidate_id = arg_str(args, "candidate_id").unwrap_or_else(|| "candidate".into());
    let candidate_path = arg_str(args, "candidate_path");
    let baseline_path = arg_str(args, "baseline_path");
    let req = ModelResearchRequest {
        candidate_id,
        candidate_path,
        baseline_path,
        offline: arg_bool(args, "offline", true),
        apply: arg_bool(args, "apply", false),
    };
    let reg = if ctx.registry_path.exists() {
        ctx.registry_path.clone()
    } else {
        registry_path()
    };
    let mut report = run_model_research(&reg, &req);
    let out = ctx
        .repo_root
        .join("evals/runs")
        .join(format!("{}.json", report.id));
    let path = write_research_report(&report, &out).map_err(|e| ToolError::Msg(e.to_string()))?;
    report.report_path = Some(path.display().to_string());

    Ok(serde_json::to_value(&report).unwrap_or(json!({"error": "serialize"})))
}
