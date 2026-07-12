# Core agent eval: orchestrator claims

**Core agent:** `core/orchestrator` (`rust-agent/agents/core/orchestrator.md`)  
**Specialty agents:** optional (e.g. `tutoring/math-tutor`) — never required for core gate.

## Lean tool surface (10 tools, each claimed + evaluated)

| Tool | Claim | Eval fact / case |
| --- | --- | --- |
| `list_tools` | Catalog tools available to agent | `all_tools_present`, tool_plan |
| `app_status` | Runtime paths/counts/registry | `agent_count`, `tool_count`, `registry_present` |
| `list_agents` | List agents + cert status | `agent_ids`, `has_core_orchestrator` |
| `get_agent` | Inspect one agent | `orchestrator_role`, `orchestrator_model` |
| `certify_agent` | Certify vs live schema | `core_agent_cert` |
| `get_schema` | Schema version + fields | `schema_version`, `schema_fields` |
| `list_models` | Registry models | `model_keys` |
| `list_sessions` | List sessions; optional `query` searches | list mode + `session_search_via_list` |
| `get_session` | Full chat log | tool_plan invoke ok |
| `upsert_session` | Create/update sessions | seeds session for get/search |

Removed: `search_chat_logs` (merged into `list_sessions` via `query`).

## Tracks

| Track | Gate |
| --- | --- |
| `tool_plan` | **Must** pass 100% facts; all 10 tools exercised |
| `llm_agent` | Optional; model/context failures do not fail the gate |

## Run

```bash
cd rust-agent
./scripts/test-all.sh
cargo run --bin eval_introspection
# optional:
cargo run --bin eval_introspection -- --llm
```

Artifact: `evals/runs/agent-introspection-*.json` (or `core-orchestrator-*.json` id prefix).

## Pass criteria (gating)

- Core agent loads as `core/orchestrator`, role `orchestrator`
- Frontmatter tools == `CORE_AGENT_TOOLS` == registry names (exactly 10)
- `tool_plan_accuracy == 1.0`, `complete_introspection == true`
- Specialty agents optional
