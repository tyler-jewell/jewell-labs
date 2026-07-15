# AGENTS.md

This repository is a workspace for agent harnesses that evaluate language models via **llama.cpp**. Any agent (Claude Code, OpenCode, Codex, Cursor, custom harness, etc.) working here should treat this file as the source of truth for how evaluations are run and scored.

Prefer minimal tooling. Prefer local inference. Do not introduce Docker-heavy eval stacks (NeMo Evaluator, etc.) or cloud-only judges unless the user explicitly asks.

### Stack (orchestration + models)

**Canonical production stack — do not substitute:**

| Layer | What |
| --- | --- |
| **Control plane** | Paperclip (self-hosted on Hostinger VPS) |
| **Models** | In-house **llm-provider** on this Mac (`:4141`), OAuth-gated to the allowlist; Paperclip reaches it via reverse tunnel + `gateway_openai` adapter |
| **Not used** | **Hermes** — do not install, run, configure, expand, or route agents through Hermes (`hermes_local`, `hermes_gateway`, `~/hermes-local`, hermes eval vendors, etc.) |

### No Python in Jewell agent or Jewell evals

**Hard rule for this monorepo:**

- Do **not** add, restore, or introduce first-party **Python** as implementation language for the Jewell agent (`rust-agent/`) or Jewell-owned evals (`evals/catalog/`, `evals/datasets/`, `evals/harness_compare/`, `tools/`, etc.).
- Forbidden: new `.py` modules, Python harness CLIs, Python scoring libraries, or Python rewrites of agent/eval logic under first-party paths.
- **Allowed (not first-party harness code):** subject workspaces for coding datasets may contain `solution.py` / other Python **artifacts under test**. Optional host-side `python3` may execute those dataset unit tests against the agent’s workspace file. That is grading *subject* code, not Jewell implementation source.
- Canonical eval entry points are **Rust** (`cargo run --bin eval_catalog`, `eval_compare`, `eval_introspection`) and **curl** against local `llama-server`. Prefer pure Rust graders (regex, file equality, structural checks) when no external subject-language runtime is required.

---

## Evaluations and Benchmarking

### Goal

Run evaluation dataset items against models served by llama.cpp, and score them **locally**—including using llama.cpp models themselves as graders—without proprietary APIs.

### Canonical stack (most efficient)

The lean path used by upstream llama.cpp (merged as `examples/llama-eval`, PR [#21152](https://github.com/ggml-org/llama.cpp/pull/21152)):

| Layer | What | Why |
| --- | --- | --- |
| **Serve** | `llama-server` with OpenAI-compatible `/v1/chat/completions` | Load the GGUF once; amortize startup across all items |
| **Run** | HTTP POST to that endpoint (`curl` or Rust harnesses) | No custom inference bindings; works with any OpenAI client/`curl` |
| **Score** | Prefer **regex** exact match; fall back to **LLM grader** on a second (or same) `llama-server` | Regex is free; LLM grader only when free-form extraction is needed |

Do **not** use `llama-bench` for quality evals (it measures tokens/s, not accuracy). Do **not** require `lm-evaluation-harness` for the simple path (heavy; optional later for leaderboard parity). Do **not** add first-party Python harness scripts (see ban above).

### Prerequisites

1. Built llama.cpp with `llama-server` on `PATH` (or a known absolute path).
2. A GGUF model path (or `-hf org/repo` if network is allowed).
3. For multi-item / multi-vendor runs: a built `rust-agent` tree (`cargo run --bin eval_catalog` / `eval_compare` / `eval_introspection`).

### Step 0 — Start the model server (do this once)

Keep the process running for the whole eval session. Example:

```bash
llama-server \
  -m /path/to/model.gguf \
  --host 127.0.0.1 --port 8080 \
  -c 8192 -np 4 \
  --ctx-checkpoints 0
```

Notes:

- `-np N` parallel slots speeds multi-item runs; for a **single** item, `-np 1` is fine.
- `--ctx-checkpoints 0` avoids checkpoint overhead for pure eval.
- Health check: `curl -s http://127.0.0.1:8080/v1/models`
- Endpoint for chat: `POST http://127.0.0.1:8080/v1/chat/completions`

Optional second server for grading (stronger / different model):

```bash
llama-server -m /path/to/grader.gguf --host 127.0.0.1 --port 8081 -c 4096 -np 1
```

### Simplest path: one dataset item, closed-form answer

**Start here.** GSM8K-style numeric (or any item with a single gold string) is the simplest case.

#### 1. Generate (subject model)

```bash
curl -s http://127.0.0.1:8080/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{
    "model": "local",
    "temperature": 0,
    "messages": [
      {
        "role": "user",
        "content": "Solve step by step. Put the final numeric answer in \\boxed{}.\n\nJanet has 3 apples and buys 2 more. How many apples does she have?"
      }
    ]
  }'
```

Extract `choices[0].message.content` as the model response.

#### 2a. Score with regex (preferred when possible)

Extract the answer (last `\boxed{...}` or last integer), then exact-match against gold:

```text
gold = "5"
pred_extracted == gold  → correct
```

This is the cheapest grader: **zero extra model calls**.

#### 2b. Score with a llama.cpp model (LLM grader)

Use when the response is free-form and regex is unreliable, or when the task is open-ended. Same protocol as llama-eval’s LLM grader:

```bash
curl -s http://127.0.0.1:8080/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{
    "model": "local",
    "temperature": 0,
    "messages": [
      {
        "role": "system",
        "content": "You are an answer extraction system. Output only the extracted answer, nothing else. If no clear answer, reply with: no answer"
      },
      {
        "role": "user",
        "content": "Extract the answer from the following response:\n\n\"<MODEL_RESPONSE_HERE>\"\n\nProvide only the extracted answer."
      }
    ]
  }'
```

Then in the harness (not the model):

```text
extracted.strip().lower() == gold.strip().lower()  → correct / incorrect
```

**Rules for LLM graders:**

- Always `temperature: 0` (and omit creative sampling).
- Ask for **extraction** of a short answer (or a fixed schema), not a long essay.
- Prefer comparing extracted answer to gold in code; do not ask the judge to “be fair” without a gold label when a gold label exists.
- Same model can generate and grade; a separate stronger grader is better when available (`--grader-server` / second port).
- Truncating the subject response to the last few lines before grading often improves extraction (matches llama-eval practice).

#### Open-ended / rubric scoring (still local)

When there is no single gold string (quality, style, partial credit):

1. Generate with the subject model as above.
2. Grade with a fixed rubric prompt on the grader server, requiring a parseable last line, e.g.:

```text
Score 0 or 1.
Reasoning: <one short paragraph>
SCORE: <0|1>
```

3. Parse `SCORE:` with a regex in the harness. Do not accept free-form “looks good” without a machine-readable field.

### Batch path: curl smoke or Rust catalog/compare

For multi-item work in **this** monorepo, prefer the Rust bins (no first-party Python). For a single closed-form item, the curl path above is enough.

**Catalog sample (public sources, Jewell harness):**

```bash
cd rust-agent
cargo run -q --bin eval_catalog -- --list-sources
cargo run -q --bin eval_catalog -- --sample-n 1 --seed 42
cargo run -q --bin eval_catalog -- --harnesses jewell --sample-n 5
```

**Harness compare (dry / jewell on shared local tasks):**

```bash
cd rust-agent
cargo run -q --bin eval_compare -- --harnesses dry --tasks all
cargo run -q --bin eval_compare -- --harnesses dry,jewell --tasks coding,closed_form,agent_os
```

**Core team gate:**

```bash
cd rust-agent
cargo run --bin eval_introspection
```

Artifacts land under `evals/runs/`. Use `curl` against `llama-server` when you need a one-off generation/grade without spinning a catalog sample.

Upstream llama.cpp still ships `examples/llama-eval/` for reference; **do not** vendor that (or any other) Python harness into this repo as Jewell eval implementation.

### Harness contract (any agent in this repo)

When implementing or running evals, agents MUST:

1. **Serve first** — ensure `llama-server` is up before generating.
2. **Prefer regex** for closed-form items; use LLM grader only when needed.
3. **Score deterministically** — `temperature: 0` on graders; fixed seeds when sampling subjects for reproducibility reports.
4. **Persist artifacts** under something like:

   ```text
   evals/
     datasets/          # optional local JSONL items
     runs/<run-id>.json # per-item: prompt, response, gold, extracted, correct, timings
   ```

5. **Record per item at least:**

   | Field | Meaning |
   | --- | --- |
   | `id` | Dataset item id |
   | `prompt` | Full prompt sent to subject |
   | `response` | Full model output |
   | `gold` | Expected answer (if any) |
   | `extracted` | Parsed/grader answer |
   | `correct` | boolean |
   | `grader` | `regex` \| `llm` \| `cli` \| `rubric` |
   | `model` / `server` | Subject identity |
   | `grader_model` / `grader_server` | If LLM graded |
   | `tokens`, `t_gen_ms` | If available from server timings |

6. **Not** pull in heavy frameworks unless the user requests parity with a public leaderboard.
7. **Not** send prompts or answers to external judge APIs by default; local llama.cpp is the default judge.
8. **Not** introduce first-party Python into the Jewell agent or Jewell evals (see ban at top).
9. **Not** use Hermes for orchestration, eval harnesses, or model routing (see stack at top).

### Local JSONL dataset (custom items)

For custom single items / small private sets, use JSONL (one object per line):

```json
{"id": "demo-001", "prompt": "What is 2+2? Put the answer in \\boxed{}.", "gold": "4"}
```

Harness loop:

```text
for each line:
  response = chat_completions(server, prompt, temperature=0)
  extracted = regex_or_llm_extract(response)
  correct = (extracted == gold)
  append result to runs/<run-id>.json
```

This is enough for the “simplest evaluation dataset item” without HuggingFace.

### Efficiency checklist

- [ ] One long-lived `llama-server` (do not restart per item)
- [ ] Regex grade when the gold is numeric / multiple-choice letter
- [ ] LLM grade only for extraction or open-ended rubrics
- [ ] `temperature: 0` on graders
- [ ] Start validation with `--n_cases 1` (or one JSONL row) before full sweeps
- [ ] Use `-np` and multi-server only when batch size justifies it
- [ ] Resume from saved JSON rather than re-running completed items
- [ ] No new first-party `.py` under agent/evals
- [ ] No Hermes harnesses or Hermes agent runtime

### What “done” looks like for a smoke eval

A successful minimal run produces:

1. A running subject `llama-server`.
2. One completed item with `response`, `extracted`, and `correct`.
3. A written run artifact under `evals/runs/`.
4. Clear report: `1/1 correct` or `0/1 correct` with grader type noted.

### Core team eval: rust-agent four-agent gate

Fixed product team under `rust-agent/agents/`:

| Id | Role |
| --- | --- |
| `core/orchestrator` | Manager — `run_agent` + `run_eval` (no writes) |
| `system/learner` | Propose via `learn` |
| `system/agent-implementor` | Apply `write_agent` |
| `system/tool-implementor` | Draft `write_tool` |

```bash
cd rust-agent
cargo run --bin eval_introspection          # tool_plan + team_collaboration (required)
cargo run --bin eval_introspection -- --llm # + live model track
# UI: open /evals for history, Run, progress log
```

- Product SSoT: `rust-agent/goal/TEAM.md`
- Datasets: `evals/datasets/core/`, `evals/datasets/system/`, `evals/datasets/team/`
- Artifacts: `evals/runs/agent-introspection-*.json`
- Pass: `complete_introspection=true`, `tool_plan_accuracy=1.0`, team collaboration green

That is the baseline every harness in this repo should be able to execute before scaling to full datasets.

### Multi-source catalog evals (public agentic benches)

Modular catalog under `evals/catalog/sources/` (SWE-bench, Terminal-Bench, BFCL). **Online datasets only** — `remote.toml` pointers; no hard-coded task bodies or `legacy-local` smokes.

```bash
cd rust-agent
cargo run -q --bin eval_catalog -- --list-sources
cargo run -q --bin eval_catalog -- --list-vendors
cargo run -q --bin eval_catalog -- --sample-n 20 --seed 42
cargo run -q --bin eval_catalog -- --harnesses jewell --sample-n 5
```

**Multi-vendor catalog (default path)**

- Shared model pin: `evals/model.toml` (override with `EVAL_MODEL_BASE_URL` / `EVAL_MODEL`). All vendors use this endpoint — apples-to-apples.
- Vendors: drop-in `evals/vendors/<id>/vendor.toml`. Empty `--harnesses` runs every `enabled = true` vendor. **Do not enable or use a Hermes vendor.**
- Add a CLI vendor by writing one TOML file (no Rust edits). See `evals/vendors/README.md`.
- Sandbox-only items are excluded from sampling unless `--include-sandbox` is set.

- Catalog: `evals/catalog/` (`source.toml` + `remote.toml` per source; hydrate online)
- Artifacts: `evals/runs/catalog-*.json`
- UI: `/evals` → **Catalog sample**
- Structural CI gates remain: `eval_introspection` (tool_plan + team) — separate from public benches
