#!/usr/bin/env bun
/**
 * Benchmark runner for the benchmark-results Hermes dashboard plugin.
 *
 * Runs EleutherAI lm-evaluation-harness (installed in ~/.hermes/eval-venv)
 * against a local Ollama model via its OpenAI-compatible /v1 endpoint, then
 * normalizes the harness output into ~/.hermes/plugins/benchmark-results/data/
 * run-<timestamp>.json — which the dashboard plugin's backend serves.
 *
 * Usage:
 *   bun run.ts --task humaneval_instruct --model qwen3:14b --limit 20
 *   bun run.ts --task mbpp_instruct --model qwen3:14b --limit 10 --no-think
 */
import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync, writeFileSync, mkdirSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const HOME = homedir();
const PLUGIN_DIR = join(HOME, ".hermes", "plugins", "benchmark-results");
const DATA_DIR = join(PLUGIN_DIR, "data");
const RAW_DIR = join(DATA_DIR, "raw");
const LM_EVAL = join(HOME, ".hermes", "eval-venv", "bin", "lm_eval");

function arg(name: string, def?: string): string | undefined {
  const i = process.argv.indexOf("--" + name);
  if (i !== -1 && i + 1 < process.argv.length) return process.argv[i + 1];
  return def;
}
const hasFlag = (name: string) => process.argv.includes("--" + name);

const task = arg("task", "humaneval_instruct")!;
const model = arg("model", "qwen3:14b")!;
const limit = arg("limit", "20")!;
// "all"/"0"/"full"/"none" → run the whole task (no --limit passed to lm-eval)
const fullRun = ["all", "0", "full", "none"].includes(limit.toLowerCase());
const baseUrl = arg("base-url", "http://localhost:11434/v1/chat/completions")!;
const noThink = hasFlag("no-think"); // pass reasoning_effort=none to disable Qwen3 thinking
const maxTokens = arg("max-tokens", "1024")!;

const ts = new Date().toISOString().replace(/[:.]/g, "-");
const outDir = join(RAW_DIR, ts);
mkdirSync(outDir, { recursive: true });

const genKwargs = `max_tokens=${maxTokens},temperature=0${noThink ? ",reasoning_effort=none" : ""}`;
const modelArgs = `model=${model},base_url=${baseUrl},num_concurrent=1,max_retries=2,tokenizer_backend=None`;

const args = [
  "--model", "local-chat-completions",
  "--model_args", modelArgs,
  "--tasks", task,
  ...(fullRun ? [] : ["--limit", limit]),
  "--apply_chat_template",
  "--confirm_run_unsafe_code",
  "--output_path", outDir,
  "--gen_kwargs", genKwargs,
];

console.log(`▶ Running lm-eval: task=${task} model=${model} limit=${fullRun ? "FULL" : limit} think=${!noThink}`);
const started = Date.now();
try {
  execFileSync(LM_EVAL, args, {
    stdio: "inherit",
    env: { ...process.env, HF_ALLOW_CODE_EVAL: "1", TOKENIZERS_PARALLELISM: "false" },
  });
} catch (e) {
  console.error("lm-eval failed:", (e as Error).message);
  process.exit(1);
}
const wallSeconds = (Date.now() - started) / 1000;

// --- locate the harness results_*.json (recurse under outDir) ----------
function findResults(dir: string): string | null {
  let newest: { path: string; mtime: number } | null = null;
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    const st = statSync(p);
    if (st.isDirectory()) {
      const inner = findResults(p);
      if (inner) {
        const m = statSync(inner).mtimeMs;
        if (!newest || m > newest.mtime) newest = { path: inner, mtime: m };
      }
    } else if (/^results_.*\.json$/.test(name)) {
      if (!newest || st.mtimeMs > newest.mtime) newest = { path: p, mtime: st.mtimeMs };
    }
  }
  return newest ? newest.path : null;
}

const resultsPath = findResults(outDir);
if (!resultsPath) {
  console.error("Could not find lm-eval results_*.json under", outDir);
  process.exit(1);
}

const raw = JSON.parse(readFileSync(resultsPath, "utf8"));
const taskRes = (raw.results && raw.results[task]) || {};
// metric keys look like "pass@1,create_test" — pick the first pass@k / acc value
const metricKey = Object.keys(taskRes).find((k) => /pass@|acc|exact_match|score/i.test(k) && !/stderr/i.test(k));
const score = metricKey ? taskRes[metricKey] : null;
const metricName = metricKey ? metricKey.split(",")[0] : "score";
const nSamples = (raw["n-samples"] && raw["n-samples"][task]) || {};

const normalized = {
  id: ts,
  model: raw.model_name || model,
  task,
  metric: metricName,
  score,
  n_problems: nSamples.effective ?? Number(limit),
  n_total: nSamples.original ?? null,
  date: raw.date ?? Math.floor(started / 1000),
  eval_time_s: Number(raw.total_evaluation_time_seconds ?? wallSeconds),
  think: !noThink,
  endpoint: "ollama " + baseUrl.replace(/\/v1.*$/, "/v1"),
  harness: "lm-eval " + (raw.lm_eval_version || "?"),
  raw_results_path: resultsPath,
};

const outFile = join(DATA_DIR, `run-${ts}.json`);
writeFileSync(outFile, JSON.stringify(normalized, null, 2));
console.log(`✔ ${task} ${metricName}=${score} (${normalized.n_problems} problems, ${Math.round(normalized.eval_time_s)}s)`);
console.log(`  wrote ${outFile}`);
