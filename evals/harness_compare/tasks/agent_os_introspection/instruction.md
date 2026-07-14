# Agent OS introspection (host)

This task is scored by the **jewell host eval** (`cargo run --bin eval_introspection`), not by free-form chat.

Harnesses that implement `host_eval` should run the structural tool_plan + team gate.

Harnesses without host_eval (e.g. Hermes) will be **skipped** unless they implement an equivalent probe.
