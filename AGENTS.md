# AGENTS.md

## Grok harness contract (required)

- **Standard:** follow `~/.grok/standards/GROK_HARNESS_v1.md` (latest Grok Build layout).
- **Canonical harness root:** `.grok/` only. Do **not** add or use project `.claude/skills`, `.claude/agents`, `.cursor/skills`, or `.cursor/agents`.
- **Grade:** after any change under `.grok/` or this file, run `/grok-harness-grade` and leave the tree at grade **A** (or document an explicit waiver in the grade report).
- **Rules budget:** keep this file short; put long procedures in `.grok/skills/*/SKILL.md`, not here.
- **No docs.** Never create a `docs/` folder or standalone doc files. Knowledge goes into **skills** (`.grok/skills/*/SKILL.md`), **agents**, or **this `AGENTS.md`** — nowhere else.
- **Scaffold:** new skills/agents/personas via `/agent-creator` or `/create-skill`; match bundled style under `~/.grok/bundled/`.

This repo runs a **Paperclip agent-company** on an in-house **OpenAI-compatible model gateway** (`llm-provider`). The custom home-grown harness (`rust-agent/`) and the local eval framework (`evals/`) have been **removed** — we no longer build or maintain our own agent/eval harness. The coding-agent runtime is being standardized on a single external harness (**opencode vs Hermes** comparison in progress).

## Canonical stack

| Layer | What |
| --- | --- |
| **Control plane** | Paperclip (self-hosted VPS) |
| **Model gateway** | In-house **llm-provider** on this Mac (`:4141`), OAuth allowlist; Paperclip reaches it via reverse tunnel. Local Qwen3.6-35B-A3B is the default (`default_model`); cloud (Claude/Grok) is escalation only. |
| **Coding harness** | Being standardized — **opencode vs Hermes** (see the harness comparison). Until decided, do not add a new custom harness. |

## Hard rules

1. **No custom harness.** Do not reintroduce a home-grown agent/eval harness (the removed `rust-agent/`, `evals/`, `tools/run_agent*`). Standardize on the chosen external harness.
2. **No hardcoded model/endpoint config.** Models, endpoints, and lists come from central config (`llm-provider/config.toml`, Paperclip adapterConfig), never source. See `llm-provider/.grok/skills/`.
3. **Hermes:** under evaluation as a harness candidate (previously banned; the ban is on hold pending the opencode-vs-Hermes decision — do not adopt or route production agents through it until that lands).
4. **SendBlue / messaging:** agent-native skills (`skills/sendblue`, Paperclip setup scripts) — not a monorepo poller daemon.
5. **llm-provider package:** also read `llm-provider/AGENTS.md`. Package skills live under **`llm-provider/.grok/skills/`** only.

## Skills

| Location | Skills |
| --- | --- |
| `llm-provider/.grok/skills/` | `add-provider`, `drive-interactive-cli`, `paperclip-admin`, `tune-local-llm` |

When adding **Grok project skills**, put them under the relevant package’s **`.grok/skills/`**, never `.claude/skills/`.
