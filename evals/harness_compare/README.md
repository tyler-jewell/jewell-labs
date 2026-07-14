# Legacy local Harbor tasks (demoted)

> **Default multi-source evals live in `evals/catalog/`** (SWE-bench, Terminal-Bench, BFCL):
>
> ```bash
> cargo run -q --bin eval_catalog -- --list-sources
> cargo run -q --bin eval_catalog -- --harnesses dry,jewell,hermes --sample-n 20 --seed 42
> ```
>
> Multi-source evals: `evals/catalog/` (online datasets via `remote.toml`).

Lean evaluation framework for head-to-head runs of:

| Harness | How we drive it |
| --- | --- |
| **hermes** | `hermes chat -q "…"` (or `hermes -z`) with `cwd` = task workspace |
| **jewell** | Host structural tracks + HTTP `/api/chat/stream` against rust-agent |

Inspired by the three strongest public Hermes-vs-peers comparisons (not a clone of any one of them):

| Source | What we borrow | What we skip (for now) |
| --- | --- | --- |
| **WolfBench** (X, Apr 2026) | Same tasks × harnesses; **avg + solid base** (min over `n_runs`); multi-run reliability | Proprietary 89-task suite, commercial platform |
| **WildClawBench** ([InternLM](https://github.com/InternLM/WildClawBench)) | Isolated workspace per (task, harness, run); **post-hoc** graders (tests after agent exits); 0/1 + fine metrics | Docker images, 60-task multimodal suite, cost telemetry |
| **AARRI-Bench** ([arXiv:2606.07462](https://arxiv.org/abs/2606.07462)) | Harbor-lite layout (`task.toml` + `instruction.md` + `tests/`); multi-harness table | Full research-intern taxonomy, cloud Harbor runners |

Repo rules still apply (`AGENTS.md`): local llama.cpp preferred, regex/shell graders preferred, **no Docker-heavy eval stack** required for smoke runs.

---

## Fairness notes (read before ranking)

1. **Hermes ≠ jewell capability surface.** Hermes ships terminal + file + skills. Jewell’s core team is an **agent OS** (list/certify/write agents & tools, learn, run_eval). Coding workspace tasks will often favor Hermes until jewell grows general FS/shell tools.
2. **Fix the model when comparing harnesses.** Point both at the same OpenAI-compatible endpoint when possible (local `llama-server` or one cloud model). WolfBench/WildClaw both stress *harness* vs *model*.
3. **Outcome grading only.** Graders never see the agent’s chain-of-thought as ground truth; they only inspect workspace files / stdout JSON after the run (WildClaw style).
4. **Multi-run.** Use `--n-runs 3` for a WolfBench-style solid base; smoke with `--n-runs 1`.

---

## One-shot install + sample (recommended)

Non-interactive install of Hermes **into this repo** (gitignored), local llama wiring, compare sample, and score audit:

```bash
# From monorepo root
bash evals/harness_compare/install_and_compare.sh

bash evals/harness_compare/install_and_compare.sh --install-only
bash evals/harness_compare/install_and_compare.sh --sample-only \
  --tasks coding_fix_bug,closed_form_ratio,agent_os_introspection
```

| Path | Role |
| --- | --- |
| `vendor/hermes-agent/` | Hermes code + venv |
| `vendor/hermes-home/` | `HERMES_HOME` (config, sessions) |
| `vendor/bin/hermes` | Stable launcher |

`evals/harness_compare/vendor/` is **gitignored**. Hermes needs ~18k+ context for tools; the script auto-starts compare `llama-server` on **:8091** (Qwen3-4B, `-c 32768`) when missing. Override: `COMPARE_LLAMA`, `COMPARE_MODEL_PATH`, `COMPARE_CTX`.

```bash
# Inspect a compare report (JSON under evals/runs/)
jq '.summary, (.items[] | {status, harness, task_id, score})' evals/runs/<run-id>.json
```

First-party Python harnesses (`run_compare.py`, `lib/**`) have been **removed**. Use the Rust CLI only. Hermes vendor under `vendor/hermes-agent/` is third-party and may remain Python.

## Manual quick start

```bash
# From monorepo root / rust-agent
cd rust-agent

# Dry-run (no agents; validates tasks + graders against gold workspaces)
cargo run -q --bin eval_compare -- --harnesses dry --tasks all

# Jewell structural + chat (rust-agent must be up for chat tasks)
#   cargo run -q  # serve API if needed
cargo run -q --bin eval_compare -- --harnesses jewell --tasks agent_os,closed_form

# Hermes one-shot (vendored binary auto-detected, or HERMES_BIN=...)
cargo run -q --bin eval_compare -- --harnesses hermes --tasks coding,closed_form

# Head-to-head smoke
cargo run -q --bin eval_compare -- --harnesses hermes,jewell --tasks all --n-runs 1

# Reliability pass (WolfBench-style)
cargo run -q --bin eval_compare -- --harnesses hermes,jewell --tasks coding --n-runs 3
```

Artifacts land under `evals/runs/<run-id>.json` (and companion HTML when produced). Per-run workspaces may also appear under `evals/harness_compare/runs/` depending on runner config.

---

## Task layout (Harbor-lite)

```text
tasks/<task_id>/
  task.toml            # id, track, timeout, capabilities
  instruction.md       # prompt shown to the agent
  workspace/           # seed files (copied per run; no tests here)
  tests/               # optional post-hoc checks (prefer Rust graders in rust-agent)
```

`task.toml` fields:

| Field | Meaning |
| --- | --- |
| `id` | Stable task id |
| `track` | `coding` \| `agent_os` \| `closed_form` \| `multi_step` |
| `timeout_s` | Wall-clock budget for the agent process |
| `capabilities` | Optional tags the harness must claim (e.g. `terminal`, `host_eval`) |
| `n_runs_default` | Default repeats for solid-base |

---

## Tracks

| Track | What it stresses | Hermes | Jewell |
| --- | --- | --- | --- |
| `coding` | Edit workspace files, fix bugs | Strong | Weak until FS tools exist |
| `agent_os` | Introspect / certify / team structure | Via shell on repo | Native host tools |
| `closed_form` | Single gold string / boxed answer | Both | Both |
| `multi_step` | Ordered file artifacts | Strong | Partial |

---

## Adding a task

1. Copy an existing task under `tasks/` (or the template if present).
2. Seed `workspace/` (no answer keys in files the agent can “accidentally” open — put gold only in grader-side data).
3. Wire grading in **Rust** (`rust-agent/src/eval/compare/grade.rs`) for known task ids. Prefer pure Rust structural/file checks. Do **not** add first-party Python harness modules (see monorepo `AGENTS.md`).
4. Coding *subject* files under workspace may be `.py` when the task itself is Python; that is graded artifact code, not Jewell implementation.

---

## Hermes setup (optional)

```bash
curl -fsSL https://hermes-agent.nousresearch.com/install.sh | bash
hermes model   # point at same llama-server / OpenRouter model as jewell when possible
hermes chat -q "ping"
```

Env overrides:

| Env | Default | Role |
| --- | --- | --- |
| `HERMES_BIN` | `hermes` | CLI binary |
| `HERMES_MODE` | `chat` | `chat` (`-q`) or `z` (`hermes -z`) |
| `JEWELL_BASE` | `http://127.0.0.1:3000` | rust-agent HTTP |
| `JEWELL_AGENT` | `core/orchestrator` | agent ref for chat (`category/name`) |
| `COMPARE_LLAMA` | `http://127.0.0.1:8080` | optional closed_form direct chat |

---

## What “done” looks like for a smoke compare

1. `cargo run -q --bin eval_compare -- --harnesses dry --tasks all` → all graders green on gold fixtures.
2. At least one real harness run with a report under `evals/runs/` showing **avg** and **solid_base** per harness.
3. Capability skips recorded as `skipped` (not silent zeros).
