//! Modular multi-source catalog runner (filter + seeded sample).
//!
//! ```bash
//! cargo run -q --bin eval_catalog -- --list-sources
//! cargo run -q --bin eval_catalog -- --list-vendors
//! cargo run -q --bin eval_catalog -- --sample-n 20 --seed 42
//! cargo run -q --bin eval_catalog -- --harnesses jewell,hermes --sample-n 5
//! ```

use clap::Parser;
use rust_agent::{
    default_vendor_ids, list_source_summaries, load_vendor_manifests, run_catalog_sample,
    CatalogRunOpts,
};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(about = "Run filtered/sampled catalog eval items across discovered vendors")]
struct Args {
    /// List registered sources and exit
    #[arg(long, default_value_t = false)]
    list_sources: bool,

    /// List discovered vendors and exit
    #[arg(long, default_value_t = false)]
    list_vendors: bool,

    /// Include disabled sources
    #[arg(long, default_value_t = false)]
    include_disabled: bool,

    /// Comma vendor ids (empty = all enabled from evals/vendors/)
    #[arg(long, default_value = "")]
    harnesses: String,

    /// Comma source ids (empty = all enabled)
    #[arg(long, default_value = "")]
    sources: String,

    /// Comma tags (OR)
    #[arg(long, default_value = "")]
    tags: String,

    /// Comma tags (AND)
    #[arg(long, default_value = "")]
    tags_all: String,

    /// Comma tracks
    #[arg(long, default_value = "")]
    tracks: String,

    /// Random sample size (omit = all matching runnable items)
    #[arg(long)]
    sample_n: Option<usize>,

    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Include Docker/Harbor sandbox items (they will skip). Default excludes them.
    #[arg(long, default_value_t = false)]
    include_sandbox: bool,

    /// Skip shared model preflight (tests only)
    #[arg(long, default_value_t = false)]
    skip_model_preflight: bool,

    #[arg(long)]
    model_file: Option<PathBuf>,

    #[arg(long)]
    run_id: Option<String>,

    #[arg(long)]
    runs_dir: Option<PathBuf>,
}

fn split_csv(s: &str) -> Vec<String> {
    s.split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

fn main() -> ExitCode {
    let args = Args::parse();
    if args.list_sources {
        match list_source_summaries(args.include_disabled) {
            Ok(list) => {
                println!("{}", serde_json::to_string_pretty(&list).unwrap_or_default());
                return ExitCode::SUCCESS;
            }
            Err(e) => {
                eprintln!("list sources failed: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    if args.list_vendors {
        match load_vendor_manifests(None) {
            Ok(list) => {
                let slim: Vec<_> = list
                    .iter()
                    .map(|m| {
                        serde_json::json!({
                            "id": m.id,
                            "name": m.name,
                            "enabled": m.enabled,
                            "kind": m.kind,
                            "capabilities": m.capabilities,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&slim).unwrap_or_default());
                if let Ok(d) = default_vendor_ids(None) {
                    eprintln!("default harnesses: {}", d.join(","));
                }
                return ExitCode::SUCCESS;
            }
            Err(e) => {
                eprintln!("list vendors failed: {e}");
                return ExitCode::FAILURE;
            }
        }
    }

    let opts = CatalogRunOpts {
        harnesses: split_csv(&args.harnesses),
        sources: split_csv(&args.sources),
        tags_any: split_csv(&args.tags),
        tags_all: split_csv(&args.tags_all),
        tracks: split_csv(&args.tracks),
        sample_n: args.sample_n,
        seed: args.seed,
        run_id: args.run_id,
        include_disabled: args.include_disabled,
        exclude_sandbox: !args.include_sandbox,
        skip_model_preflight: args.skip_model_preflight,
        model_file: args.model_file,
        runs_dir: args.runs_dir,
    };

    match run_catalog_sample(&opts) {
        Ok(report) => {
            println!("id={}", report.id);
            if let Some(em) = report.meta.get("eval_model") {
                println!(
                    "eval_model={}",
                    serde_json::to_string(em).unwrap_or_default()
                );
            }
            println!(
                "selection={} scored={}/{} accuracy={:.0}% skip={} err={}",
                report.selection.len(),
                report.summary.correct,
                report.summary.n_scored,
                report.summary.accuracy * 100.0,
                report.summary.n_skip,
                report.summary.n_error
            );
            println!("selection_ids={}", report.selection.join(", "));
            println!("by_source:");
            for (s, b) in &report.by_source {
                println!(
                    "  {s}: pass={} fail={} skip={} err={} acc={:.0}%",
                    b.n_pass,
                    b.n_fail,
                    b.n_skip,
                    b.n_error,
                    b.accuracy * 100.0
                );
            }
            println!("by_harness:");
            for (h, b) in &report.by_harness {
                println!(
                    "  {h}: pass={} fail={} skip={} err={} acc={:.0}%",
                    b.n_pass,
                    b.n_fail,
                    b.n_skip,
                    b.n_error,
                    b.accuracy * 100.0
                );
            }
            let hard_err = report.items.iter().any(|i| i.status == "error");
            if hard_err {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(e) => {
            eprintln!("catalog run failed: {e}");
            ExitCode::FAILURE
        }
    }
}
