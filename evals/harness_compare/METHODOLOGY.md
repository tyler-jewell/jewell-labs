# Methodology notes (from top-3 Hermes public comparisons)

## 1. WolfBench harness eval (Apr 2026)

**Claim:** Hermes Agent > Claude Code > OpenClaw on 89 tasks with fixed models (Opus 4.6, GPT-5.4).

**Design choices we keep:**

- Same task set for every harness.
- Report **avg** and **solid_base** (`min` over `--n-runs`) — reliability floor, not just a lucky run.
- Compare **harnesses**, not just models (fix model when possible).

**What we do not clone:** proprietary suite, commercial runner, effort-level sweeps (`medium` vs `xhigh`).

## 2. WildClawBench (InternLM, arXiv:2605.10912)

**Claim:** Four harnesses (OpenClaw, Claude Code, Codex, Hermes) on 60 in-the-wild tasks; scores depend heavily on scaffolding.

**Design choices we keep:**

- Isolated **workspace copy** per (harness, task, run).
- **Post-hoc** grading only (tests never present inside the agent-visible workspace seed).
- 0/1 + partial `score` / `metrics` JSON.
- Explicit harness comparison table in `report.md`.

**What we do not clone:** Docker images per harness, multimodal/video tasks, cost accounting, full 60-task suite.

## 3. AARRI-Bench (arXiv:2606.07462)

**Claim:** Mini-SWE-Agent + Opus 4.7 (68.3%) > Hermes + Opus (64.6%) > Claude Code + Opus (62.2%) on research-intern tasks — complex harness ≠ always better.

**Design choices we keep:**

- Harbor-lite task dirs: `task.toml` + `instruction.md` + `workspace/` + `tests/`.
- Classic outcome reward; trajectory length left optional for later.
- Willingness to show **minimal harness** winning (our `dry` / future `mini` adapter).

**What we do not clone:** 82 research tasks, Harbor cloud runners, fine-grained unit taxonomies (context/mindset/hands-on).

## Fair product-vs-product rules for jewell vs Hermes

| Rule | Why |
| --- | --- |
| Capability tags on tasks | Avoid silent zeros when jewell has no terminal/FS tools |
| Host gates for agent_os | Jewell’s real strength (tool_plan / team) is structural |
| Coding track favors Hermes today | Honest; force investment if coding is a goal |
| Shared closed_form | Isolates model+wrapper when both can answer |
| No Docker required for smoke | Matches monorepo `AGENTS.md` |

## Suggested first real compare

```bash
# From rust-agent/ (Rust-only; no first-party Python harness)
# 1) grader sanity
cargo run -q --bin eval_compare -- --harnesses dry --tasks all

# 2) jewell home field
cargo run -q --bin eval_compare -- --harnesses jewell --tasks agent_os

# 3) after `hermes` installed + same model endpoint
cargo run -q --bin eval_compare -- --harnesses hermes,jewell --tasks coding,closed_form --n-runs 3
```
