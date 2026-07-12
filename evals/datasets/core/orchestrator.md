# Structural eval dataset pointer for core/orchestrator
# Host still uses the built-in full tool plan for the core agent.

## case: smoke-list
track: tool_plan
prompt: "List tools and agents"
require_tools: [list_tools, list_agents, app_status]
