# Team collaboration suite (host structural gate)

Host executes `run_team_collaboration_case` — not a free-form LLM plan.

Expected product loop:
1. orchestrator `run_agent` → system/learner
2. learner `learn` (dry_run) proposes patches
3. orchestrator `run_agent` → system/agent-implementor or system/tool-implementor
4. implementor applies write_agent / write_tool
5. `run_eval` confirms green

## case: structural-host
track: tool_plan
prompt: "Host team collaboration facts"
require_tools: [run_agent, learn, write_agent, write_tool]
