# rust-agent

Rust + Leptos agent console for Jewell Labs — a **fixed four-agent team** with eval-gated improvement.

## Team

| Id | Role | Default model | Owns |
| --- | --- | --- | --- |
| `core/orchestrator` | Manager (default) | `qwen3-4b` | introspect, sessions, **`run_agent`**, `run_eval` |
| `system/learner` | Diagnose / propose | `qwen3.6-35b` | **`learn`** (dry_run default), introspect, `run_eval` |
| `system/agent-implementor` | Apply agent markdown | `qwen3-4b` | **`write_agent`**, certify, `run_eval` |
| `system/tool-implementor` | Tool drafts only | `qwen3-4b` | **`write_tool`**, `list_tools` |

Improve loop: **orchestrator → learner → implementor → run_eval**.

## Layout

```text
rust-agent/
  agents/core/orchestrator.md
  agents/system/{learner,agent-implementor,tool-implementor}.md
  tools/{category}/{tool}.rs
  src/*                          # host (immutable to agents)
  data/sessions/{category}/{name}/
```

## Run

```bash
cd rust-agent
cargo run
# open http://127.0.0.1:3000/          → core/orchestrator
# open http://127.0.0.1:3000/evals    → history, run suite, progress log
```

## Quality gate

```bash
./scripts/test-all.sh
# line budget ≤300, cargo test, wasm build, team tool_plan eval
```

Eval-only:

```bash
cargo run --bin eval_introspection
# optional live LLM:
cargo run --bin eval_introspection -- --llm
```

Artifacts: `evals/runs/agent-introspection-*.json`, `eval-*.json`.

## Docs

- Repo eval principles: `../AGENTS.md`
- Product SSoT: `goal/TEAM.md`
