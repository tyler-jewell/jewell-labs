//! Multi-harness compare CLI (Rust-only; replaces Python run_compare.py).
//!
//! ```bash
//! cargo run -q --bin eval_compare -- --harnesses dry --tasks all
//! cargo run -q --bin eval_compare -- --harnesses dry,jewell,hermes --tasks coding,closed_form,agent_os
//! ```

use clap::Parser;
use rust_agent::{run_compare, CompareOpts};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(about = "Compare agent harnesses (dry / jewell / hermes) on shared tasks")]
struct Args {
    /// Comma list: dry,jewell,hermes
    #[arg(long, default_value = "dry")]
    harnesses: String,

    /// all | track names | task ids
    #[arg(long, default_value = "all")]
    tasks: String,

    #[arg(long)]
    n_runs: Option<u32>,

    #[arg(long)]
    run_id: Option<String>,

    #[arg(long)]
    tasks_dir: Option<PathBuf>,

    #[arg(long)]
    runs_dir: Option<PathBuf>,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let harnesses: Vec<String> = args
        .harnesses
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let opts = CompareOpts {
        harnesses: harnesses.clone(),
        tasks_filter: args.tasks,
        n_runs: args.n_runs,
        run_id: args.run_id,
        tasks_dir: args.tasks_dir,
        runs_dir: args.runs_dir,
    };

    println!("harnesses={harnesses:?}");
    match run_compare(&opts) {
        Ok(report) => {
            println!("id={}", report.id);
            println!(
                "summary: scored {}/{} correct accuracy={:.0}%",
                report.summary.correct,
                report.summary.total,
                report.summary.accuracy * 100.0
            );
            for (h, s) in &report.summary.harnesses {
                println!(
                    "  {h}: avg={:.2} solid_base={:.2} pass_rate={:.0}% skip={} err={}",
                    s.avg_score,
                    s.solid_base,
                    s.pass_rate * 100.0,
                    s.n_skip,
                    s.n_error
                );
            }
            // dry-only runs must be fully green
            if harnesses == ["dry"] {
                let bad = report.items.iter().any(|i| i.status != "pass");
                return if bad {
                    ExitCode::FAILURE
                } else {
                    ExitCode::SUCCESS
                };
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("compare failed: {e}");
            ExitCode::FAILURE
        }
    }
}
