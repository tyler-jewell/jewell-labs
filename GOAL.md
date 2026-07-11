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

- [x] **T1 — `compact` command.** Re-author a template to the minimum
  history: one call carrying its current system prompt + EVALS (two calls
  when a CONFIG is present). *Accept:* a compacted template scores
  identically to its pre-compaction self on a fresh instance; compacted
  templates are ≤2 lines.
  *(2026-07-10: `agent compact` replays extracted state as real calls —
  CONFIG (if set) then EVALS, both carrying the current system prompt.
  Core template: 3→2 lines, extracted EVALS/CONFIG/sysprompt byte-identical,
  fresh-instance eval 3/3 + 3/3 both before and after. No-CONFIG branch on a
  yes-no copy: 1 line, state semantically identical. Committed with this
  change.)*
- [x] **T2 — `improve --all` fixpoint sweep.** Iterate `improve` over the
  core + every template until a full pass accepts zero rewrites. *Accept:*
  the sweep terminates; per-round accepted/rejected log recorded here.
  *(2026-07-10 round 1, core + 23 templates: 22 already perfect; 2 rewrite
  attempts, both rejected — language-detect 7/8 → 0/8, locate-0 19/20 → 9/20
  (candidates authored by the 0.6b default model scored far below incumbent).
  Zero accepted in a full pass ⇒ fixpoint reached at round 1; no template
  changed. Re-run over the post-T8/T9 set happens at campaign end per the
  final task. Committed with this change.)*
- [x] **T3 — Close the 0.6B's last misses.** `locate-0` (9/10) and
  `language-detect` (3/4), via more improve rounds or better seed turns,
  under the 3-strike rule. *Accept:* improved, or marked capability-limited
  with evidence.
  *(2026-07-10: both misses were finish_reason=length — greedy decoding
  (temp 0) sends the 0.6b's <think> into an unbounded repetition loop (8k+
  tokens on "Donde esta la biblioteca?"); no-think answers wrong instead.
  Fix = better seed turns: per-template CONFIG patch for qwen3-0.6b to
  Qwen's thinking-mode sampling (temp 0.6, top_p 0.95, top_k 20, seed 42),
  folded in via call + compact. Sampling made runs flaky under the eval
  workload (prompt-cache chunking jitter), so core CONFIG now sets
  cache_prompt:false for both models — 4× solo runs and 2× full quick-eval
  now byte-identical. Result: language-detect 4/4 + 4/4, locate-0 10/10 +
  10/10, quick-eval 20/20 + 20/20. Also learned + documented in README:
  the core's CONFIG message must be complete (latest-wins replaces it),
  agent CONFIGs are patches. Committed with this change.)*
  *(Update, later same day: locate-0's sampled patch proved
  workload-marginal — case 7 flipped red when quick-eval grew to 9 rows;
  temp 0.45 converges to a wrong answer, top_k 8 stayed workload-sensitive.
  Superseded by fully deterministic decoding: temp 0 + dry_multiplier 0.8
  (DRY breaks the think-loop as a pure function of the sequence, no RNG).
  locate-0 10/10 on 0.6b; quick-eval ×2 identical, all models green on
  locate-0-7. language-detect keeps its sampled patch — never flipped.)*
- [x] **T4 — Marker-truncation guard.** Messages containing a literal
  `<|im_end|>` / `<|im_start|>` silently truncate extraction. Guard it or
  make it fail loudly. *Accept:* an adversarial call either round-trips or
  errors visibly; no silent truncation.
  *(2026-07-10: `call` refuses any user/system text containing `<|im_` before
  contacting the server or appending — round-tripping an embedded marker is
  inherently ambiguous, so fail-loudly was chosen over escaping. Verified:
  marker in user msg and in `--system` both error rc=1 with target file
  untouched; clean call still answers and appends. Committed with this
  change.)*
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
- [x] **T7 — In-repo portability proof.** Commit `Dockerfile` +
  `portability-test.sh` (llama.cpp `:full` image, `ENV LD_LIBRARY_PATH=/app`,
  apt jq/curl; mount repo ro + a small GGUF; fresh dir inside: bootstrap
  CONFIG → new → call → eval). *Accept:* `docker build` + `docker run`
  passes from a clean checkout.
  *(2026-07-10: image = llama.cpp `:full` + jq/curl/procps, entrypoint runs
  `portability-test.sh` in a fresh `/work`: CONFIG bootstrap into an empty
  core history → `new` from ro-mounted `templates/format/yes-no.jsonl` →
  call answers "Yes" → eval `yes-no 4/4` → PORTABILITY TEST PASSED, exit 0,
  with Qwen3-0.6B-Q8_0 mounted at `/model.gguf`. Committed with this
  change.)*
- [x] **T8 — Wider benchmark surface.** New `templates/reason/` category
  (GSM8K-style, regex-gradeable numeric answers, ~3 templates × ~5 cases,
  pulled via the HF datasets rows API with curl+jq) and a harder SWE framing
  (file *and* function). *Accept:* new templates in the matrix, baselines
  recorded here, one improve pass run over them.
  *(2026-07-10: `reason/gsm-0..2` = GSM8K test rows 0–14 via
  datasets-server rows API; graded by final-line-number regex.
  `swe/locate-fn` = 5 SWE-bench-Verified single-file/single-function issues
  graded `file\.py::(\w+\.)?function`; carries a 35b max_tokens:8192 CONFIG
  patch (two issues need >4096 thinking tokens; core 35b ctx now 16384).
  Seed baselines: 0.6b 11/20, 35b 17/20, 4b 5/20. Improve pass (35b
  authoring): gsm-0 11/15→12/15 and gsm-2 10/15→11/15 accepted — gsm-0's
  self-authored "last line must contain ONLY the number" rule fixed the 4b's
  'Final answer:' prefix habit and was propagated to gsm-1/2 as reseeds;
  gsm-1 and locate-fn rewrites rejected. Post-refine: 0.6b 10/20, 35b
  17/20, 4b 11/20. Remaining misses are genuine headroom (0.6b math errors,
  4b's residual prefix habit, hard localizations incl. one 35b think-loop
  beyond 8k tokens); canaries quick/gsm-0-0 + quick/locate-fn-0 added.
  Committed with this change.)*
- [x] **T9 — Third model.** Register a mid-size instruct GGUF (~4B class)
  with one `CONFIG` call after checking disk/RAM; seed it like the others.
  *Accept:* three-column matrix committed.
  *(2026-07-10: qwen3-4b = ggml-org Qwen3-4B-Instruct-2507 Q8_0 (4.3GB;
  32GB RAM / 266GB disk checked), registered by re-authoring the core
  CONFIG with its recommended sampling; router preset now serves three
  models. Three-column matrix below — committed with this change.)*
- [x] **T11 — Quick-eval option (user, 2026-07-10).** A fast TDD loop instead
  of comprehensive runs while iterating: `quick/` holds one-line derived
  templates (source template's system prompt, EVALS = only its known-failing
  items) plus `quick-eval.sh` runs those and one fast representative template
  per category. Every newly discovered failing item gets added to `quick/`;
  the comprehensive matrix runs once at campaign end as the final summary.
  *Accept:* `quick-eval.sh` runs in a small fraction of full-matrix time;
  known failures reproduce red in it; new failures found later are added.
  *(2026-07-10: both known 0.6b failures diagnosed as finish_reason=length —
  the model burns all 2048 completion tokens inside <think> and emits no
  content. quick/language-detect-1.jsonl + quick/locate-0-7.jsonl derived as
  one-line templates; quick-eval.sh = those + core + 4 category reps: 20
  cases, 1m46s both models, 18/20 + 20/20 — red exactly on the two known
  items. Committed with this change.)*
- [x] **T10 — Starter-pack export/import.** A pack IS a compacted template
  file. Document share/import (copy the file + `./agent new`) in README.
  Explicit non-goal: marketplace infrastructure. *Accept:* a pack exported
  from this repo imports and evals clean in a scratch dir elsewhere.
  *(2026-07-10: README "Starter packs" section documents export
  (compact + cp) and import (new + eval). Proof: a live sentiment-json
  instance was called, compacted to a 1-line pack, copied to a scratch dir
  outside the repo holding only the `agent` script; after one bootstrap
  CONFIG call there, `new` + a live call answered and the pack evaled
  4/4 + 4/4 + 4/4 across all three models. The proof exposed a mkdir race
  (call stamps runs/.lastuse before ensure creates runs/ when the server is
  already healthy) — fixed by moving mkdir before ensure's early return;
  re-proof in a second fresh scratch dir ran clean. Committed with this
  change.)*

## Current baseline (reproduce before starting)

- Commit `d936eb2`, working tree clean. `agent` = 228 lines; 23 templates
  (`extract/ classify/ format/ transform/ swe/`) + core
  `template.history.jsonl` at root.
- Matrix (fresh instances, seeded): **qwen3-0.6b 93/95, qwen3.6-35b 95/95**
  (SWE localization: 28–29/30 and 30/30).
- Sanity check: `./agent eval templates/format/yes-no.jsonl -m qwen3-0.6b`
  → `4/4`. Server processes after `idle_ttl` (600s): zero.

- [x] **T12 — Fastest evals (user, 2026-07-10).** Optimize the llama.cpp
  configuration so evals — quick-eval first — run as fast as this machine
  allows, maximizing compute use (32GB Apple Silicon). Levers: per-model
  parallel slots, batching, keeping models resident (router `--models-max`),
  flash attention. *Accept:* measured quick-eval wall-time improvement with
  an unchanged results table; SEQUENTIAL=1 stays available for
  bit-reproducible benchmark claims.
  *(2026-07-10: measured everything. Profile: 0.6b round 10.5s, 4b 13.5s,
  35b 107s — 95% of the 35b round is locate-fn-0's ~5k-token think chain at
  48 tok/s, the Metal decode floor for this A3B Q3 (flash-attn already
  auto-on; explicit on: gen unchanged, prompt 417→460 tok/s). Rejected with
  evidence: parallel slots (2m55 vs 2m28 — batched decode splits the GPU
  while the bottleneck is one chain, and it flips marginal cases);
  0.6b-as-draft speculative decoding for the 35b (worker fails to load:
  cross-generation vocab mismatch). Adopted: `models_max` CONFIG key
  (router --models-max; core now 3 — user directive supersedes the
  "--models-max 1" invariant wording; reaper unchanged, so no idle servers)
  and a per-model `server {...}` CONFIG passthrough into the preset as the
  durable tuning surface. Result: all three models resident, quick-eval
  2m15–2m25 with the table unchanged, and the real iteration win —
  single-model TDD loops stay warm: `./quick-eval.sh -m qwen3-0.6b` = 10s,
  `-m qwen3-4b` = 13s. Committed with this change.)*

## Campaign complete — final state (2026-07-10)

Definition of done met:
1. Every task checked (none WONTFIX); T11 added mid-campaign by the user.
2. Fixpoint: final-sweep round 2 (core + 27 templates, 35b authoring)
   accepted zero rewrites in a complete pass; every already-perfect
   template scored identically across rounds 1 and 2.
3. Reproducibility: two consecutive fresh-instance full-matrix runs
   (`rm -rf runs` between) printed **byte-identical** tables under
   `SEQUENTIAL=1`. Finding: at 28-template parallelism, request timing
   perturbs numerics enough to flip marginal cases run-to-run (4 rows
   flipped between parallel runs; solo and 9-row workloads were stable),
   so `evaluate` gained a 3-line opt-in sequential mode — strictly one
   call in flight — documented in README. Parallel eval remains the
   default for iteration speed; benchmark claims use SEQUENTIAL=1.
4. Working tree clean, every milestone committed.

Final matrix (fresh instances, SEQUENTIAL=1, identical across two runs) —
115 cases, 28 rows (core + 27 templates):
**qwen3-0.6b 105/115 · qwen3.6-35b 113/115 · qwen3-4b 104/115**
Remaining misses are recorded headroom: GSM math errors (0.6b), the 4b's
residual "Final answer:" prefix habit (canaried in quick/), and hard
localizations in locate-fn (incl. one 35b think-loop beyond 8k tokens).
Quick TDD loop: `./quick-eval.sh` (~1.5 min, all three models) carries
one derived template per known-failing item: gsm-0-0, hashtag-gen-2,
locate-fn-0 red by design; language-detect-1 and locate-0-7 green since
their fixes.
