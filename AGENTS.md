# AGENTS.md

## Grok harness contract (required)

- **Standard:** follow `~/.grok/standards/GROK_HARNESS_v1.md` (latest Grok Build layout).
- **Canonical harness root:** `.grok/` only. Do **not** add or use project `.claude/skills`, `.claude/agents`, `.cursor/skills`, or `.cursor/agents`.
- **Grade:** after any change under `.grok/` or this file, run `/grok-harness-grade` and leave the tree at grade **A** (or document an explicit waiver in the grade report).
- **Rules budget:** keep this file short; put long procedures in `.grok/skills/*/SKILL.md`, not here.
- **No docs.** Never create a `docs/` folder or standalone doc files. Knowledge goes into **skills** (`.grok/skills/*/SKILL.md`), **agents**, or **this `AGENTS.md`** — nowhere else.
- **Scaffold:** new skills/agents/personas via `/agent-creator` or `/create-skill`; match bundled style under `~/.grok/bundled/`.

This repo runs a **Paperclip agent-company** on an in-house **OpenAI-compatible model gateway** (`llm-provider`). The custom home-grown harness (`rust-agent/`) and the local eval framework (`evals/`) have been **removed** — we no longer build or maintain our own agent/eval harness. The standardized coding-agent runtime is **opencode** (chosen over Hermes: TypeScript not Python, first-class `opencode run` headless CLI that Paperclip's `opencode_local`/`process` adapter spawns directly, `@ai-sdk/openai-compatible` provider pins `/v1/chat/completions` against our gateway, LSP + `apply_patch`, 1.x stability). Hermes stays out (Python + general-assistant design + prior ban).

## Canonical stack

| Layer | What |
| --- | --- |
| **Control plane** | Paperclip (self-hosted VPS) |
| **Model gateway** | In-house **llm-provider** on this Mac (`:4141`), OAuth allowlist; Paperclip reaches it via reverse tunnel. Local Qwen3.6-35B-A3B is the default (`default_model`); cloud (Claude/Grok) is escalation only. |
| **Coding harness** | **opencode** — via Paperclip's built-in `opencode_local` adapter, provider `@ai-sdk/openai-compatible` pointed at the gateway (`PAPERCLIP_OPENCODE_PROVIDERS`). No custom harness. |

## Hard rules

1. **No custom harness.** Do not reintroduce a home-grown agent/eval harness (the removed `rust-agent/`, `evals/`, `tools/run_agent*`). The standard is **opencode** (`opencode_local` adapter).
2. **No hardcoded model/endpoint config.** Models, endpoints, and lists come from central config (`llm-provider/config.toml`, Paperclip adapterConfig), never source. See `llm-provider/.grok/skills/`.
3. **No Hermes.** Not adopted — do not install, run, or route agents through Hermes (`hermes_local`, `hermes_gateway`).
4. **SendBlue / messaging:** agent-native skills (`skills/sendblue`, Paperclip setup scripts) — not a monorepo poller daemon.
5. **llm-provider package:** also read `llm-provider/AGENTS.md`. Package skills live under **`llm-provider/.grok/skills/`** only.
6. **SendBlue plugin package:** also read `plugins/paperclip-plugin-sendblue/AGENTS.md` (≤300 lines/file, develop/test/release).

## Skills

| Location | Skills |
| --- | --- |
| `llm-provider/.grok/skills/` | `add-provider`, `drive-interactive-cli`, `paperclip-admin`, `tune-local-llm` |
| `plugins/paperclip-plugin-sendblue/.grok/skills/` | `plugin-configuration` (interactive SendBlue setup + dashboard Run test) |

When adding **Grok project skills**, put them under the relevant package’s **`.grok/skills/`**, never `.claude/skills/`.
