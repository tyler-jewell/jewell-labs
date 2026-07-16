# AGENTS.md

## Grok harness contract (required)

- **Standard:** follow `~/.grok/standards/GROK_HARNESS_v1.md` (latest Grok Build layout).
- **Canonical harness root:** `.grok/` only. Do **not** add or use project `.claude/skills`, `.claude/agents`, `.cursor/skills`, or `.cursor/agents`.
- **Grade:** after any change under `.grok/` or this file, run `/grok-harness-grade` and leave the tree at grade **A** (or document an explicit waiver in the grade report).
- **Rules budget:** keep this file short; put long procedures in `.grok/skills/*/SKILL.md` or `docs/`, not here.
- **Scaffold:** new skills/agents/personas via `/agent-creator` or `/create-skill`; match bundled style under `~/.grok/bundled/`.

Workspace for **agent harnesses** that evaluate language models via **llama.cpp**. Prefer minimal tooling and **local inference**. No Docker-heavy eval stacks or cloud-only judges unless the user asks.


## Canonical stack

| Layer | What |
| --- | --- |
| **Control plane** | Paperclip (self-hosted VPS) |
| **Models** | In-house **llm-provider** on this Mac (`:4141`), OAuth allowlist; Paperclip via reverse tunnel + `gateway_openai` |
| **Not used** | **Hermes** — do not install or route through Hermes |

## Hard rules

1. **No first-party Python** for Jewell agent (`rust-agent/`) or Jewell-owned evals (`evals/**`, first-party `tools/`). Subject datasets may contain Python under test; host `python3` may grade those artifacts only.
2. **Eval entry points:** Rust (`cargo run --bin eval_catalog`, `eval_compare`, `eval_introspection`) and **curl** against local `llama-server`. Prefer pure Rust graders.
3. **SendBlue / messaging:** agent-native skills (`skills/sendblue`, Paperclip setup scripts) — not a monorepo poller daemon. See `docs/SENDBLUE_PAPERCLIP.md`.
4. **llm-provider package:** also read `llm-provider/AGENTS.md`. Package skills live under **`llm-provider/.grok/skills/`** only.

## Skills

| Location | Skills |
| --- | --- |
| `llm-provider/.grok/skills/` | `add-provider`, `drive-interactive-cli`, `paperclip-admin`, `tune-local-llm` |
| Repo `skills/` (company) | e.g. `sendblue` — keep; prefer documenting paths clearly |

When adding **Grok project skills**, put them under the relevant package’s **`.grok/skills/`**, never `.claude/skills/`.

## Merge / run signals

- Local serve: `llama-server` OpenAI-compatible `/v1/chat/completions`.
- Prefer existing Rust eval bins over inventing new harness languages.
