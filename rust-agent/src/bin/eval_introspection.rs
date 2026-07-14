//! Core agent eval: orchestrator introspection via built-in tools.
//!
//! ```bash
//! cd rust-agent
//! cargo run --bin eval_introspection
//! # with live llama-server (uses agent default_model):
//! cargo run --bin eval_introspection -- --llm
//! ```
//!
//! Writes `evals/runs/agent-introspection-<ts>.json` under the monorepo root.

use clap::Parser;
use rust_agent::{run_full_eval, write_report};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(about = "Evaluate orchestrator complete app introspection via built-in tools")]
struct Args {
    /// Also run live LLM agent track (requires llama-server).
    #[arg(long, default_value_t = false)]
    llm: bool,

    /// Output path (default: <repo>/evals/runs/agent-introspection-<ts>.json)
    #[arg(long)]
    output: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    println!("running agent introspection eval (llm={})", args.llm);

    let report = run_full_eval(args.llm).await;

    let out = args.output.unwrap_or_else(|| {
        let root = rust_agent::repo_root();
        root.join("evals")
            .join("runs")
            .join(format!("{}.json", report.id))
    });

    match write_report(&report, &out) {
        Ok(p) => println!("wrote {}", p.display()),
        Err(e) => {
            eprintln!("failed to write report: {e}");
            return ExitCode::FAILURE;
        }
    }

    println!(
        "summary: {}/{} correct (accuracy={:.0}%)",
        report.summary.correct,
        report.summary.total,
        report.summary.accuracy * 100.0
    );
    println!(
        "  tool_plan_accuracy={:.0}%  all_tools_invoked={}  complete_introspection={}",
        report.summary.tool_plan_accuracy * 100.0,
        report.summary.all_tools_invoked_in_plan,
        report.summary.complete_introspection
    );
    if let Some(a) = report.summary.llm_agent_accuracy {
        println!(
            "  llm_agent_accuracy={:.0}%  model={:?} server={:?}",
            a * 100.0,
            report.model,
            report.server
        );
    }

    for c in &report.cases {
        let mark = if c.correct { "PASS" } else { "FAIL" };
        println!(
            "  [{mark}] {} track={} tools={:?} facts_ok={}/{}",
            c.id,
            c.track,
            c.tools_called,
            c.facts.iter().filter(|f| f.correct).count(),
            c.facts.len()
        );
        if !c.correct {
            for f in c.facts.iter().filter(|f| !f.correct) {
                println!("      - {}: expected={} got={}", f.id, f.expected, f.got);
            }
        }
    }

    if report.summary.complete_introspection
        && report.summary.tool_plan_accuracy >= 1.0
        && report
            .cases
            .iter()
            .filter(|c| c.track == "tool_plan")
            .all(|c| c.correct)
    {
        // Core gate: tool-plan complete introspection must pass.
        // LLM track is informative unless EVAL_REQUIRE_LLM=1
        let require_llm = std::env::var("EVAL_REQUIRE_LLM").ok().as_deref() == Some("1");
        if require_llm {
            if report.summary.llm_agent_accuracy == Some(1.0) {
                ExitCode::SUCCESS
            } else {
                eprintln!("EVAL_REQUIRE_LLM=1 but llm track did not fully pass");
                ExitCode::FAILURE
            }
        } else {
            ExitCode::SUCCESS
        }
    } else {
        ExitCode::FAILURE
    }
}
