# Benchmarks (benchmark-results)

A [Hermes](https://github.com/) dashboard plugin that benchmarks **local
Ollama models** with EleutherAI's
[lm-evaluation-harness](https://github.com/EleutherAI/lm-evaluation-harness)
and shows the results in a **Benchmarks** tab (pass@1 / score per task, model,
think-mode, and wall-clock time).

## Install

```sh
hermes plugins install tyler-jewell/jewell-labs/plugins/benchmark-results --enable
hermes dashboard      # open the "Benchmarks" tab (/benchmarks)
```

## Requirements

- [Ollama](https://ollama.com) running locally (default `http://localhost:11434`).
- lm-evaluation-harness installed in a venv at `~/.hermes/eval-venv`:
  ```sh
  python3 -m venv ~/.hermes/eval-venv
  ~/.hermes/eval-venv/bin/pip install "lm-eval[api]"
  ```
- [Bun](https://bun.sh) to run the runner.

## Run a benchmark

```sh
cd ~/.hermes/plugins/benchmark-results
bun runner/run.ts --task humaneval_instruct --model qwen3:14b --limit 20
# full run:
bun runner/run.ts --task humaneval_instruct --model qwen3:14b --limit all --no-think
```

The runner normalizes harness output into `data/run-<timestamp>.json`, which
the dashboard backend serves at `GET /api/plugins/benchmark-results/runs`.
Generated run files are local-only (gitignored) — your `data/` starts empty.

## Endpoints

`GET /runs` (all runs, newest first) · `GET /latest`
