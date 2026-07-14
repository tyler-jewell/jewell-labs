# Product SSoT — four-agent team

Supersedes the multi-phase transform checklist. Shipped shape:

## Agents (only these four)

1. **core/orchestrator** — routes work via `run_agent`; never writes agents/tools.
2. **system/learner** — proposes improvements via `learn` (dry_run default).
3. **system/agent-implementor** — `write_agent` with fail-closed eval.
4. **system/tool-implementor** — `write_tool` drafts (`.rs.draft` / agent-local).

## Host rules

- Path jail + core write lock on `core/orchestrator`.
- `run_agent` depth ≤ 1; dry_run skips LLM (CI).
- Nested `run_eval` depth ≤ 1.
- Fitness = host evals under `evals/runs/` (tool_plan + team_collaboration).

## Models

| Agent | Model |
| --- | --- |
| orchestrator, implementors | qwen3-4b |
| learner | qwen3.6-35b |

## UI

- Sidebar: Agents (by category) + **Evals → Dashboard**
- `/evals`: history, Run (team/agent/all), progress log, detail
