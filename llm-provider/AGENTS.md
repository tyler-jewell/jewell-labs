# AGENTS.md — operating rules for agents in this repo

## Grok harness contract (required)

- **Standard:** follow `~/.grok/standards/GROK_HARNESS_v1.md` (latest Grok Build layout).
- **Canonical harness root:** `.grok/` only. Do **not** add or use project `.claude/skills`, `.claude/agents`, `.cursor/skills`, or `.cursor/agents`.
- **Grade:** after any change under `.grok/` or this file, run `/grok-harness-grade` and leave the tree at grade **A** (or document an explicit waiver in the grade report).
- **Rules budget:** keep this file short; put long procedures in `.grok/skills/*/SKILL.md`, not here.
- **Scaffold:** new skills/agents/personas via `/agent-creator` or `/create-skill`; match bundled style under `~/.grok/bundled/`.

This is the **llm-provider** gateway: an OpenAI-compatible endpoint on `:4141` unifying local
llama.cpp/ollama + grok + Claude. Read `README.md` for the architecture.


## 1. Never modify Paperclip core

We operate one or more Paperclip instances (see the `paperclip-admin` skill).
**Never edit Paperclip's own source code.** Interact with any Paperclip instance only
through:

- the `paperclipai` CLI,
- its REST API,
- built-in tools (external adapters, the secrets vault, board/agent config).

Our Paperclip code lives entirely under `integrations/paperclip/` — these are **external**
adapters loaded via `paperclipai adapter install` / REST. That directory is ours to change;
Paperclip's installed source (in its container/data dir) is off-limits. Before changing any
Paperclip behavior, consult the latest docs (`https://paperclip.ing/llms.txt`) and prefer a
CLI/API/adapter path over touching core.

## 2. Gateway credentials are short-lived — nothing is permanent

Every credential this gateway issues is an **access token + rotating refresh token**, modeled on
how we consume Claude/grok upstream. No issued token is valid forever.

- Mint (`/auth/google`, `/auth/refresh`, `login-xai`, `mint <email>`) returns a bundle:
  `{ access_token (llmgw-…), refresh_token (llmgwr-…), expires_at }`.
- `access_token` is short-lived (`auth.access_ttl_secs`, default 1h); present it as
  `Authorization: Bearer …`.
- `refresh_token` (`auth.refresh_ttl_secs`, default 30d, sliding) renews via
  `POST /auth/refresh`. Refresh **rotates**: the old refresh token is invalidated on use.
- Clients renew themselves near expiry. The Paperclip `gateway_openai` adapter does this
  automatically from a `credsFile` bundle (`integrations/paperclip/gateway-adapter/index.mjs`);
  the `paperclip-admin` skill documents the full wiring.

When adding a new client, give it a creds bundle and a refresh step — never a static forever key.

## 3. Built-in Paperclip skills & tools — extract and use them

Paperclip ships **skills** (SKILL.md instruction packs its agents load) and a slot for **tool
plugins**. Both live on the instance; enumerate them before writing anything new so you reuse a
built-in instead of duplicating it. The robust, boilerplate-free path is the board REST API via
the `paperclip-admin` skill's `pc-api.sh` (it injects the keychain board key + Origin). The
`paperclipai` CLI can do the same but needs an interactive `connect --persona board` first
(bare `docker exec … paperclipai skill list` returns `401/403`), so prefer REST.

```sh
S=~/Apps/jewell-labs/llm-provider/.claude/skills/paperclip-admin
export PAPERCLIP_COMPANY=05bc506f-ceb7-49b3-b914-e866a9326064          # Flag Seeker

# --- SKILLS ---
$S/pc-api.sh GET "/api/companies/$PAPERCLIP_COMPANY/skills"            # installed (server-bundled) skills
$S/pc-api.sh GET /api/skills/catalog                                   # public catalog (installable)
$S/pc-api.sh GET "/api/companies/$PAPERCLIP_COMPANY/skills/<skillId>/files"  # -> {path:"SKILL.md", content} = extract the pack
$S/pc-api.sh GET "/api/agents/<agentId>/skills"                        # which skills an agent sees (entries[].origin/state)
# use/attach on the instance (mutations):
$S/pc-api.sh POST "/api/agents/<agentId>/skills/sync" '{"desiredSkills":["paperclip","para-memory-files"]}'   # set an agent's skill set
$S/pc-api.sh POST "/api/companies/$PAPERCLIP_COMPANY/skills/install-catalog" '<body>'  # install a catalog skill (body untyped in spec; CLI `skill import` is clearer)

# --- TOOL PLUGINS ---
$S/pc-api.sh GET /api/plugins/tools                                    # installed external tool plugins (currently [])
$S/pc-api.sh POST /api/plugins/tools/execute '{"tool":"<name>","parameters":{},"runContext":{}}'   # invoke one (tool*, runContext* required)
```

CLI equivalents (inside the container, after `docker exec -it … paperclipai connect --persona board`):
`paperclipai skill list --json` · `skill file <id> --json` · `available-skill list --json` ·
`llm agent-configuration` (agent tool/config prompt docs).

**Current inventory (2026-07-15).** Five server-bundled skills are installed
(`@paperclipai/server/skills/…`, origin *company_managed*): `paperclip` (control-plane
API/coordination), `paperclip-board` (board management via chat), `paperclip-converting-plans-to-tasks`
(plan→task methodology; pairs with `paperclip`), `paperclip-create-agent` (governance-aware
hiring), `para-memory-files` (PARA cross-session memory). The public catalog holds 11 more,
none installed. **No tool plugins are installed** (`/api/plugins/tools` is empty); agent
capabilities come through the `paperclip` skill + control-plane API + the agent's adapter.

**De-dup rule.** The five built-ins are complementary — do not add a skill that restates one.
In particular, **do not install catalog `task-planning`** — it duplicates the built-in
`paperclip-converting-plans-to-tasks` (both turn a plan/issue into a task graph) and would give
agents two competing methodologies. Catalog `issue-triage` narrowly overlaps `paperclip-board`
(triage decisions only) — fine to add, but keep board ops authoritative. Our Mac-side Claude
Code skills (`paperclip-admin`) operate a *different surface* (admin from the Mac) than the
in-instance agent skills and are not duplicates.
