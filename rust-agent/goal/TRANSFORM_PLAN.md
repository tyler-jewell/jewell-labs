# Multi-Phase Transform Plan — rust-agent

**Checkpoint commit:** `efde332` (2026-07-11)  
**Date:** 2026-07-11  
**Principles:** KISS · SSoT · DRY · first principles · no framework cargo-cult  

This plan turns the current orchestrator console into a **lean agent-improving machine**: one host-enforced orchestrator, markdown agents, allowlisted tools, eval-gated learning, minimal UI.

---

## 0. Goals (acceptance targets)

| # | Goal | Done when |
|---|------|-----------|
| 1 | KISS / SSoT / DRY everywhere | No dual nav for same concept; one path jail; one eval runner; line budgets hold |
| 2 | First principles, not copy-paste frameworks | No CrewAI/LangGraph/AutoGen; lessons only (orchestrator-as-manager, eval gate, path jail) |
| 3 | Always start with main orchestrator + tools | `/` → `core/orchestrator` chat; orch has full tool surface |
| 4 | Layout: `agents/{cat}/{name}.md`, `tools/{cat}/{name}.rs`, rest is host `src/` | Enforced by jail + docs; orch never updates `src/` |
| 5 | Orch frontmatter → all tools; write only `agents/*` + `tools/*` | Host write tools + pre-open path jail; core identity locked |
| 6 | Min UI: agents-only sidebar; tools as metadata → definition | No tools tree in left nav; chips + drawer |
| 7 | Built-in `learn` tool | Introspect agents/logs; improve one or many; eval keep/revert |
| 8 | Minimal evals for ALL agents; create requires eval setup; orch can eval any incl. self | Generic `run_eval`; co-located datasets; host certify gate; `eval_depth≤1` |

---

## 1. Evidence base (research + vet)

Four research briefs + four isolated vet harnesses. **Only ship patterns that were PROVED.**

### 1.1 Architecture (research)

- Production pattern: **orchestrator + specialists-as-tools**, not free-form multi-agent chat.
- Agents-as-markdown (AGENTS.md / SKILL.md / Claude skills shape) matches us already.
- **Trust boundary:** agent must not write policy, `src/`, or allowlist enforcer (CBSE-class failures).
- Improve via failure-mined **bounded** prompt patches + external eval — not RL.

### 1.2 Vet scorecard (what holds in practice)

| Area | Claim | Verdict | Implication for plan |
|------|--------|---------|----------------------|
| Jail | Dual-root path jail for `agents/`+`tools/` | **PROVED** in isolation | Build `WriteJail` host API |
| Jail | Current `write_agent_file` TOCTOU/symlink | **PARTIAL / gap** | Pre-open checks; no post-write-only safety |
| Jail | Frontmatter = security principal | **FAILED** | Lock writes to `core/*`; role not self-asserted as power |
| Tools | Hot-reload agent `.rs` | **PROVED free is false** | Propose tool → host accept/rebuild; not live eval of Rust |
| Eval | Deterministic `tool_plan` CI, no LLM judge | **PROVED** (structural claims) | Keep as only merge gate |
| Eval | frontmatter.eval + dataset path = enough SSoT | **OVERCLAIMED** | Need validators (non-empty require_tools, known tools) |
| Eval | create incomplete without green eval | **PROVED host mechanism; absent in prod** | Wire into certify/publish |
| Eval | `eval_depth≤1` | **PROVED necessary** when `run_eval` is a tool | Host counter |
| Eval | exact golden order brittle | **PROVED** | Score with require_tools **superset** + optional forbid |
| Learn | unguided self-rewrite regresses | **CONFIRMED** (reg=1.0) | Never auto-apply without eval |
| Learn | snapshot→patch→eval→keep/revert | **CONFIRMED** (~200 LOC host) | Core of `learn` |
| Learn | parallel needs file claims | **CONFIRMED** | Disjoint targets only |
| Learn | `max_diff_lines` + `dry_run` default | **CONFIRMED** | Defaults in tool contract |
| UI | Tools sidebar wrong | **UPHELD** | Remove peer tools tree |
| UI | Chips + definition drawer | **UPHELD**; mock ~135 LOC | Implement |
| UI | Default orchestrator | **gap** | Redirect `/` |
| UI | SSR + light SSE adequate | **UPHELD** | Keep stack |

**Isolated artifacts (reproduce):**  
`/tmp/vet-path-jail-*`, `/tmp/vet-evals-*`, `/tmp/vet-learn-*`, `/tmp/vet-ui-mock/`

### 1.3 Current product gaps (honest)

| Capability | Today |
|------------|--------|
| Core agent at `agents/core/orchestrator.md` | Yes |
| Lean 10 tools + claim-backed tool_plan eval | Yes (orch only) |
| `write_agent_file` HTTP API | Yes — **not** exposed as agent tool |
| Write `tools/*` | No |
| `learn` | No |
| `run_eval` as agent tool | No (offline bin only) |
| Agents-only sidebar / tool chips | No (dual tree) |
| Default → orchestrator | No (empty home) |
| Create-agent requires green eval | No |
| Pre-open path jail (symlink-safe) | Partial / unsafe under symlink poison |

---

## 2. Target architecture (first principles)

```text
┌─────────────────────────────────────────────────────────────┐
│ HOST (src/) — immutable to agents                           │
│  path jail · schema certify · tool registry · tool loop     │
│  sessions · eval runner · learn keep/revert · SSE shell     │
└───────────────┬─────────────────────────────┬───────────────┘
                │ read/write (jail)           │ register (compile)
                ▼                             ▼
     agents/{cat}/{name}.md          tools/{cat}/{name}.rs
     (+ optional eval: pointer)      (orch may propose; host builds)
                │
                ▼
     evals/datasets/{cat}/{name}.md   evals/runs/{agent}--{ts}.json
```

**SSoT rules**

1. Agent identity = file path `agents/{cat}/{name}.md` (not free-form `role` power).
2. Tool identity = registered host module name (disk `.rs` alone is not callable).
3. Capability = frontmatter `tools: []` intersect host registry + allowlist enforcer.
4. Fitness = host eval scores under `evals/runs/` (never model self-score).
5. UI agent list = live FS under `agents/` only.

**Orchestrator model:** manager that uses tools (introspect, write, eval, learn, later `run_agent`). Specialists are markdown + restricted tools — not peer chat swarms.

**Write surface (goal 5, hardened by vet):**

| Path | Orch write? | Notes |
|------|-------------|--------|
| `agents/**/*.md` except lock policy | Yes | Prefer create-new + replace regular files only |
| `agents/core/orchestrator.md` | **Default: no** (or human flag) | Prevents identity takeover |
| `tools/**/*.rs` + category `mod.rs` stubs | **Propose only** → host accept | No live register without rebuild |
| `src/**`, `Cargo.toml`, scripts, eval host | **Never** | Jail deny |
| `evals/datasets/**` | Yes (as part of create/learn) | Required for goal 8 |

---

## 3. Multi-phase plan

Each phase has: scope, deliverables, acceptance tests, and **out of scope**.  
Do not start phase N+1 until phase N acceptance is green. Prefer vertical slices.

---

### Phase 0 — Baseline freeze (½ day)

**Why:** Stable ground under transform; already mostly done at `efde332`.

| Deliverable | Detail |
|-------------|--------|
| Quality gate green | `./scripts/test-all.sh` |
| Document claim map | Keep CORE_AGENT_TOOLS ↔ eval facts |
| Plan committed | This file |

**Accept:** test-all pass; no new features.

**Out:** UI/learn/eval redesign.

---

### Phase 1 — Hardened write surface + path jail (1–2 days)

**Why:** Goal 4–5 security foundation; vet proved current write path is not content-safe under symlink/hardlink.

| Deliverable | Detail |
|-------------|--------|
| `src/jail.rs` (or `agents/jail.rs`) | `WriteJail { allowed_roots }` — resolve pre-write; reject abs/`..`/`\0`; refuse symlink/hardlink targets; create-new / write-replace |
| Route `write_agent_file` through jail | Pre-open; no “write then canonicalize then unlink link” |
| Dual-root optional | Phase 1: **`agents/` only**. Phase later adds `evals/datasets/`. `tools/` write stays separate propose API |
| Lock core agent | Config flag or hard deny write to `CORE_AGENT_ID` path by default |
| Symlink/hardlink tests | Port vet cases into `cargo test` |

**Accept:**

- Unit tests: all vet attack vectors DENY without damaging outside content.
- Integration: create agent under `agents/lab/x.md` OK; escape paths fail.
- Line budget: jail module ≤150 LOC.

**Out:** tool source writing; learn; UI.

---

### Phase 2 — Minimal UI transform (1–2 days)

**Why:** Goal 3 + 6; vet mock proved agents-only + chips in ~135 LOC.

| Deliverable | Detail |
|-------------|--------|
| Remove tools sidebar section | `shell.rs` — agents only |
| Stop full tools tree SSR on every page | Drop `list_tools_from_fs` from sidebar path |
| `/` → redirect or render `core/orchestrator` chat | Always active one agent |
| Available tools chips | From agent frontmatter allowlist (resolve registry defs) |
| Tool definition drawer/panel | Click chip → name, description, params (from `builtin_tools` / protocol) |
| Tabs | Primary: Chat, Sessions. Secondary/collapsed: Schema (frontmatter+cert). Registry optional/debug or drop |
| Brand subcopy | “agents/ · src/” not tools-as-nav |

**Accept:**

- HTTP test: `/` lands on orchestrator.
- No `/tools` primary nav links in shell HTML.
- Tool chip click returns definition JSON/HTML without leaving agent.
- Manual: SSE chat still streams tool cards.
- JS SSE tests still pass.

**Out:** redesign chat chrome; SPA; graph IDE.

---

### Phase 3 — Orchestrator write tools (agents first) (1–2 days)

**Why:** Goal 5 — orch must edit/create agents via tools, not “host will write later” prompt fiction.

| Deliverable | Detail |
|-------------|--------|
| Tool `write_agent` | args: `id`, `markdown` → jail write → return path + cert status |
| Tool `read` already covered | `get_agent` |
| Optional `delete_agent` | deny core; soft-delete or require empty sessions |
| HTTP create path | Same host function as tool (DRY) |
| Update orch frontmatter | Add `write_agent` to allowlist + CORE_AGENT_TOOLS |
| Eval cases | tool_plan fact: write + reload + certify for non-core id in temp dir |
| Prompt update | “Use write_agent; never claim host will save” |

**Accept:**

- Integration: invoke `write_agent` → file on disk → `list_agents` sees it → `certify_agent` reflects schema.
- Escape id rejected.
- Core overwrite denied by default.
- tool_plan eval green including new tool claims.

**Out:** writing Rust tools; auto-compile.

---

### Phase 4 — Universal minimal eval system (2–3 days)

**Why:** Goal 8; research + vet: code-fact tool_plan is the durable gate; co-location needs validators.

| Deliverable | Detail |
|-------------|--------|
| Schema: optional `eval:` in frontmatter | `dataset: {cat}/{name}`, `gate: tool_plan`, `max_turns`, `claims[]` optional |
| Dataset layout | `evals/datasets/{cat}/{name}.md` (migrate orch from flat `agent_introspection.md`) |
| Case format (minimal) | `track`, `prompt`, `require_tools[]`, `forbid_tools[]?`, `facts[]` with ops: `set_contains`, `eq`, `set_eq` |
| Validators | Reject empty `require_tools` when track=tool_plan; unknown tool names; duplicate case ids |
| Generic runner | `run_eval(agent_id, track?)` shared by bin + future tool |
| Scoring | require_tools as **superset**; facts code-only; no LLM-as-sole-gate |
| Certify gate | `certify` / publish path: if `eval.dataset` present, last run green OR inline run; else warn (phase 4) / fail (phase 5 specialty) |
| Bin | `eval_agent --agent X` and `--all --track tool_plan` |
| Math-tutor | Ship ≥1–3 smoke cases as template for specialty |

**Accept:**

- Orch + math-tutor tool_plan green offline.
- Malformed empty require_tools rejected at parse.
- `test-all.sh` runs `--all` tool_plan.
- Self-eval via bin works without recursion issue (still offline).

**Out:** LLM-judge CI; trajectory exact-order scoring; Braintrust/HELM.

---

### Phase 5 — `run_eval` tool + create-with-evals (1–2 days)

**Why:** Orch must eval any agent incl. self anytime; create must install eval best practice.

| Deliverable | Detail |
|-------------|--------|
| Tool `run_eval` | `agent_id`, optional `track`, `n_cases` |
| Host `eval_depth` | Thread-local / request counter; reject nested ≥1 |
| Create template | `write_agent` requires or auto-scaffolds: frontmatter `eval:` + dataset file with ≥1 case per claimed tool (or ≥1 smoke + 1 negative) |
| Publish semantics | Draft write OK; `certify_agent` fails if eval red / missing dataset when role=agent |
| Orch self-eval | `run_eval("core/orchestrator")` on frozen tool_plan only; host scores |
| Dataset write | Jail root includes `evals/datasets/` for orch tools |

**Accept:**

- Nested `run_eval` → `eval_depth_exceeded` (port vet).
- Creating agent without dataset → not certified.
- Creating agent with green tool_plan → certified.
- Orch can run eval on math-tutor and self in one session (depth 1 each call, sequential).

**Out:** parallel eval fan-out (can be phase 6 learn); live LLM track as required gate.

---

### Phase 6 — `learn` tool (2–3 days)

**Why:** Goal 7; vet confirmed keep/revert + claims + dry_run + max_diff.

| Deliverable | Detail |
|-------------|--------|
| Tool `learn` | See contract below |
| Host algorithm | claim → snapshot → patch proposal (LLM or structured) → sandbox eval → keep\|revert → log |
| Signals | Prefer `eval` fails; optional session log mining (`list_sessions` query) |
| Parallel | `mode=parallel` only on **disjoint** agent ids; file claim table |
| Defaults | `dry_run=true`, `max_diff_lines=40`, `max_iters=1`, `human_approve_tools=true` |
| Scope default | `agents` only; tools scope later and always needs_human |
| Artifacts | `evals/runs/learn-{id}.json` immutable report |
| Orch self-learn | Default **off** / needs_human |

**`learn` contract (minimal):**

```text
learn(
  targets: string[],              // agent ids
  mode: "single" | "parallel",
  signal: "eval" | "sessions" | "both",
  max_iters: 1,
  max_diff_lines: 40,
  dry_run: true
) -> LearnReport
```

**Accept:**

- Port vet experiments A–F as host unit tests (fake eval substring OK for control structure).
- dry_run leaves disk unchanged.
- Bad patch reverts; good patch keep only if score ≥ baseline.
- Parallel same target serializes or second fails claim.
- Path outside agents denied.

**Out:** multi-file transactions; RL; automatic tool capability expansion.

---

### Phase 7 — Tool proposals (tools/*) without live RCE (2 days)

**Why:** Goal 5 says orch can edit tools; vet proved compile-time registry — hot-reload is a lie.

| Deliverable | Detail |
|-------------|--------|
| Tool `propose_tool` | Write `tools/{cat}/{name}.rs` + stub note under jail **as draft** (`.rs.draft` or `tools/_proposed/`) |
| Manifest | `tools/PROPOSED.md` or JSON listing pending |
| Host accept path | Human or CI: move into tree, add `mod`, rebuild, restart |
| No auto-register | Disk proposal never callable until rebuild |
| Eval claim | Each proposed tool must declare claim string for future fact |
| Orch prompt | Explicit: propose ≠ live |

**Accept:**

- Propose creates draft only; `list_tools` runtime unchanged.
- Escape to `src/` denied.
- Documented rebuild flow in README (≤10 lines).

**Out:** dynamic WASM tools; in-process `dlopen`; agent-owned network tools.

---

### Phase 8 — Polish & lean (1–2 days)

**Why:** Goal 1 — delete dead surface after new capabilities land.

| Deliverable | Detail |
|-------------|--------|
| Dead code pass | Remove ToolPanel as primary focus if unused; drop unused tabs |
| CORE_AGENT_TOOLS final set | introspect + sessions + write_agent + run_eval + learn (+ propose_tool if shipped) — still explicit list |
| Line budget | Enforce ≤300/file; jail/learn modules tight |
| README + orchestrator.md | Single source of claims matching evals |
| Quality gate | test-all = line budget + cargo test + JS + eval --all tool_plan + learn unit tests |
| Optional | `run_agent` specialist-as-tool (if still needed; else defer — YAGNI until second specialty proves need) |

**Accept:**

- All 8 goals checked off with automated proof where possible.
- No tools in sidebar; default orch; learn dry_run smoke; eval all agents green.

---

## 4. Suggested tool surface evolution

| Phase end | Orchestrator tools (lean growth) |
|-----------|-----------------------------------|
| Now | 10: list_tools, app_status, list_agents, get_agent, certify_agent, get_schema, list_models, list_sessions, get_session, upsert_session |
| After 3 | + `write_agent` |
| After 5 | + `run_eval` (+ dataset write via write_agent scaffold / `write_dataset`) |
| After 6 | + `learn` |
| After 7 | + `propose_tool` (optional) |

**Do not add** generic FS, bash, or network tools to orchestrator until a failing eval demands them and safety model is explicit.

Specialty agents: **explicit allowlist only**, never `*` in production certify.

---

## 5. Eval practice (SSoT for all agents)

```text
agents/{cat}/{name}.md
  eval:
    dataset: {cat}/{name}
    gate: tool_plan

evals/datasets/{cat}/{name}.md
  ## case: smoke-...
  track: tool_plan
  require_tools: [...]
  facts: [...]

evals/runs/{agent}--{ts}.json
```

**Rules (enforced by host validators):**

1. Every claimed tool has ≥1 case touching it OR one smoke covering set membership of allowlist.
2. `require_tools` non-empty for tool_plan cases.
3. Score: superset membership + code facts; optional `forbid_tools`.
4. CI: tool_plan only. LLM track optional nightly.
5. Create-agent: scaffold dataset; certify requires green.
6. Self-eval: host-run, depth 1, frozen plan for orch.

---

## 6. UI IA (final)

```text
┌──────────────┬────────────────────────────────────────┐
│ AGENTS       │  core/orchestrator · cert · model      │
│ by category  │  [Chat] [Sessions]  · Schema (secondary)│
│  ● orch      │  Available tools: [chips…] → definition│
│    math-tutor│  transcript + tool cards + composer    │
└──────────────┴────────────────────────────────────────┘
```

- One active agent. Default always orchestrator.
- Tools are properties of the agent, not destinations.
- Sessions per agent; server files remain SSoT.

---

## 7. Risk register

| Risk | Mitigation |
|------|------------|
| Symlink/hardlink escape | Phase 1 pre-open jail; tests from vet |
| Orch overwrites self → identity takeover | Deny core write by default |
| Hollow eval greenwash | Validators + forbid empty require_tools |
| Nested eval cost bomb | `eval_depth≤1` |
| Parallel learn races | File claims; disjoint only |
| Ungated self-rewrite | dry_run default; eval keep/revert |
| Agent-written Rust RCE | propose-only + rebuild accept |
| Weak local models “learn” badly | Prefer log+eval patches; human for tool expansion |
| Scope creep (SPA, multi-agent tabs) | Explicitly out of plan |

---

## 8. Execution order (summary)

```text
P0 Baseline freeze
 → P1 Path jail + core lock
 → P2 UI agents-only + chips + default orch
 → P3 write_agent tool + eval claims
 → P4 Universal datasets + generic runner + validators
 → P5 run_eval tool + create-with-evals + depth guard
 → P6 learn (dry_run, claims, keep/revert)
 → P7 propose_tool (draft only)
 → P8 Delete dead code + final quality gate
```

**Critical path for user-visible “new machine”:** P1 → P2 → P3 → P4 → P5 → P6.  
P7 is capability completeness for tools/*; can slip if needed without blocking learn.

---

## 9. What we will NOT do

- Adopt CrewAI / AutoGen / LangGraph / DSPy as product core.
- LLM-as-judge as sole CI gate.
- Exact multi-step golden trajectory as only success.
- Agent write access to `src/`, policy, or Cargo.
- Live hot-reload of arbitrary agent-authored Rust.
- Tools as left-nav peer tree.
- Multi-agent multi-chat tab explosion.
- Open-ended AutoGPT self-rewrite without eval.
- Heavy enterprise eval platforms for local lean CI.

---

## 10. Success definition (end state)

A developer opens the console and lands on **Orchestrator**. Sidebar lists agents by category only. Tool chips show definitions. Orchestrator can:

1. Inspect agents, sessions, schema, models.
2. Create/edit specialty agents under `agents/*` (not core, not `src/`).
3. Scaffold and run **tool_plan** evals for any agent including itself.
4. **Learn** from eval/session signals with dry_run and keep/revert.
5. Propose tools under `tools/*` drafts without compromising the host.

All of this is proven by `./scripts/test-all.sh` plus learn/jail unit tests ported from vet harnesses — a **lean mean agent-improving machine**, not a framework museum.

---

## Appendix A — Research & vet index

| Brief | Path |
|-------|------|
| Arch research | `/tmp/research-agent-arch.md` |
| Eval research | `/tmp/research-agent-evals.md` |
| UI research | `/tmp/research-agent-ui.md` |
| Learn research | `/tmp/research-agent-learn.md` |
| Jail vet | `/tmp/vet-path-jail-report.md` |
| Eval vet | `/tmp/vet-evals-report.md` |
| Learn vet | `/tmp/vet-learn-report.md` |
| UI vet | `/tmp/vet-ui-report.md` |
| UI mock | `/tmp/vet-ui-mock/index.html` |

Copy durable copies into `rust-agent/goal/research/` if long-term retention is needed (optional follow-up).

## Appendix B — Phase checklist (for implementers)

- [ ] P0 test-all green at checkpoint
- [ ] P1 WriteJail + tests; core lock
- [ ] P2 UI IA transform
- [ ] P3 write_agent tool
- [ ] P4 generic eval + datasets by agent
- [ ] P5 run_eval tool + create gate + depth
- [ ] P6 learn tool
- [ ] P7 propose_tool
- [ ] P8 lean pass + goal matrix green
