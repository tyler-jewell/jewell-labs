# rust-agent

Rust + Leptos agent console for Jewell Labs.

## Layout

```text
rust-agent/
  agents/core/orchestrator.md         # CORE agent (required)
  agents/{category}/{agent-name}.md   # specialty agents (optional)
  tools/{category}/{tool-name}.rs     # lean built-in tools
  src/*                               # app server, schema, UI
  data/sessions/{category}/{name}/    # server-side chat logs
```

## Core vs specialty

| | Core | Specialty |
| --- | --- | --- |
| Id | `core/orchestrator` | e.g. `tutoring/math-tutor` |
| Role | `orchestrator` | `agent` |
| Required for product gate | **yes** | no |
| Tools | explicit lean list (`CORE_AGENT_TOOLS`, 10 tools) | restricted allowlist |

## Lean tools (10)

| Category | Tools |
| --- | --- |
| `introspect` | `list_tools`, `app_status`, `list_agents`, `get_agent`, `certify_agent`, `get_schema`, `list_models` |
| `sessions` | `list_sessions` (optional `query` = search), `get_session`, `upsert_session` |

## Run

```bash
cd rust-agent
cargo run
# open http://127.0.0.1:3000/agents/core/orchestrator
```

## Quality gate

```bash
./scripts/test-all.sh
# includes: line budget ≤300, cargo test, JS SSE tests, core tool-plan eval
```

Eval-only:

```bash
cargo run --bin eval_introspection
```

Dataset: `../evals/datasets/agent_introspection.md`
