# GOAL.md — the history-agents completion campaign

A living document. The executing agent checks boxes, appends results
`(date, result, commit)` per task, and records rejected approaches inline.
GOAL.md edits ride in the same commit as the milestone they describe.

## Mission

This repo is a llama.cpp agent framework in which an agent is nothing but a
folder of raw `llama-server -v` response dumps: `template.history.jsonl`
(seed — the only thing an agent may change) unioned with `history.jsonl`
(append-only log). System prompt, evals (`EVALS [...]`) and the entire
configuration (`CONFIG {...}`) are extracted latest-wins from that union on
every use; the runner is the 228-line POSIX-sh script `agent`. This campaign
closes the remaining vision (compaction, autonomous self-improvement),
hardens the edge cases found in code review, and extends reach (benchmarks,
a third model, portable starter packs).

## Invariants — never violate

- No Python anywhere; the runner stays POSIX `sh` + `curl` + `jq`; every
  change must pass `sh -n agent` and run under dash.
- Agents have only a system prompt and a user prompt — no tools, skills, or
  subagents. Everything is extracted latest-wins from
  `template.history.jsonl` ∪ `history.jsonl`.
- Only `template.history.jsonl` may change an agent. `history.jsonl` is
  append-only. EVALS are pinned during `improve`: agents never edit their
  own tests.
- One llama-server router (`--models-preset`, `--models-max 1`), idle reaper
  always armed. Never leave idle servers running.
- Minimization: every new line must justify itself; prefer deleting.
- Git: commit per verified milestone with the evidence in the message.
  **Never push.** Never delete committed histories or templates except where
  a task explicitly says so.

## Definition of done — stop when ALL of these hold

1. Every task below is checked, or marked `WONTFIX` with a one-line reason.
2. Improvement fixpoint: a full `improve` sweep over the core agent and
   every template accepts **zero** rewrites in one complete pass.
3. Matrix reproducibility: two consecutive fresh-instance matrix runs
   (`rm -rf runs` between them) print identical tables.
4. Working tree clean; all milestones committed; these checkboxes current.

Per-item stop rule: a template that survives **3 consecutive** improve
attempts with no accepted rewrite is marked *capability-limited* here with
its scores, and is not retried. Do not chase sub-1B capability limits.

## Keep going — do NOT stop for

- Failed verifications: fix and re-verify.
- Long model runs: run in background, poll, continue other tasks.
- Transient server errors: `ensure` restarts the router; retry once.
- Partial sweeps: resume where the log left off.

Ask the human ONLY for: changing an invariant, adding a paid or remote
dependency, deleting user data, or work not covered by any task below.

## Tasks

Suggested order: T6 → T5 → T4 → T1 → T2 → T3 → T7 → T9 → T8 → T10, then
re-run T2 (the fixpoint must cover the final template set after T8/T9).

- [ ] **T1 — `compact` command.** Re-author a template to the minimum
  history: one call carrying its current system prompt + EVALS (two calls
  when a CONFIG is present). *Accept:* a compacted template scores
  identically to its pre-compaction self on a fresh instance; compacted
  templates are ≤2 lines.
- [ ] **T2 — `improve --all` fixpoint sweep.** Iterate `improve` over the
  core + every template until a full pass accepts zero rewrites. *Accept:*
  the sweep terminates; per-round accepted/rejected log recorded here.
- [ ] **T3 — Close the 0.6B's last misses.** `locate-0` (9/10) and
  `language-detect` (3/4), via more improve rounds or better seed turns,
  under the 3-strike rule. *Accept:* improved, or marked capability-limited
  with evidence.
- [ ] **T4 — Marker-truncation guard.** Messages containing a literal
  `<|im_end|>` / `<|im_start|>` silently truncate extraction. Guard it or
  make it fail loudly. *Accept:* an adversarial call either round-trips or
  errors visibly; no silent truncation.
- [x] **T5 — Eval basename-collision guard.** Two templates with the same
  filename clobber one `runs/` instance. *Accept:* collision detected,
  clean error.
  *(2026-07-10: `evaluate` rejects duplicate basenames up front —
  `duplicate template names: yes-no`, rc=1, no server spawned; distinct
  names still pass the guard. +2 lines. Committed with this change.)*
- [x] **T6 — mktemp leak fix.** Early-exit paths in `improve`/`score` leak
  temp dirs. *Accept:* no orphan dirs after a forced early exit.
  *(2026-07-10: `improve`/`evaluate` now set `trap 'rm -rf "$TMP"' EXIT` +
  route INT/TERM through `exit`; `call` no longer resets INT/TERM to default.
  Verified with a stub llama-server: eval hitting the 60s startup timeout
  (exit 1) and TERM mid-improve both leave a private TMPDIR empty. Script
  226 lines (−2). Committed with this change.)*
- [ ] **T7 — In-repo portability proof.** Commit `Dockerfile` +
  `portability-test.sh` (llama.cpp `:full` image, `ENV LD_LIBRARY_PATH=/app`,
  apt jq/curl; mount repo ro + a small GGUF; fresh dir inside: bootstrap
  CONFIG → new → call → eval). *Accept:* `docker build` + `docker run`
  passes from a clean checkout.
- [ ] **T8 — Wider benchmark surface.** New `templates/reason/` category
  (GSM8K-style, regex-gradeable numeric answers, ~3 templates × ~5 cases,
  pulled via the HF datasets rows API with curl+jq) and a harder SWE framing
  (file *and* function). *Accept:* new templates in the matrix, baselines
  recorded here, one improve pass run over them.
- [ ] **T9 — Third model.** Register a mid-size instruct GGUF (~4B class)
  with one `CONFIG` call after checking disk/RAM; seed it like the others.
  *Accept:* three-column matrix committed.
- [ ] **T10 — Starter-pack export/import.** A pack IS a compacted template
  file. Document share/import (copy the file + `./agent new`) in README.
  Explicit non-goal: marketplace infrastructure. *Accept:* a pack exported
  from this repo imports and evals clean in a scratch dir elsewhere.

## Current baseline (reproduce before starting)

- Commit `d936eb2`, working tree clean. `agent` = 228 lines; 23 templates
  (`extract/ classify/ format/ transform/ swe/`) + core
  `template.history.jsonl` at root.
- Matrix (fresh instances, seeded): **qwen3-0.6b 93/95, qwen3.6-35b 95/95**
  (SWE localization: 28–29/30 and 30/30).
- Sanity check: `./agent eval templates/format/yes-no.jsonl -m qwen3-0.6b`
  → `4/4`. Server processes after `idle_ttl` (600s): zero.
