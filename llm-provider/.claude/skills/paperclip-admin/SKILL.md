---
name: paperclip-admin
description: >-
  Administer the self-hosted Paperclip instance on the Hostinger VPS (root@2.25.132.76) with
  full confidence — SSH in (key auth), authenticate to the board API, and run any admin task
  via the `paperclipai` CLI or REST API: manage adapters/model routing, hire/configure/pause
  agents, set budgets, inspect costs & activity, edit deployment config/secrets, back up the
  DB, and wire/route agents to the llm-provider gateway via the custom gateway_openai adapter
  (local, LAN, or over the reverse SSH tunnel) with short-lived rotating tokens. Use for ANY
  Paperclip admin, operations, debugging, integration, or configuration work. Always check the
  latest llms.txt before changing the system.
---

# Paperclip admin (`paperclip-t8tg` on `root@2.25.132.76`)

Self-hosted **Paperclip** (open-source AI-agent workforce platform; `paperclipai` CLI,
version `2026.626.0`) running as a **Docker** container behind **Traefik** (TLS) on a
Hostinger VPS. This skill covers everything needed to operate it confidently. For anything
this file doesn't cover, or before changing the system, read the **latest docs** (§ Stay current).

## The instance at a glance

| | |
|---|---|
| SSH | `root@2.25.132.76` (Ubuntu 24.04, 1 vCPU / 3.8 GB RAM, no swap) — key auth installed |
| Container | `paperclip-t8tg-paperclip-1` (`ghcr.io/hostinger/hvps-paperclip:latest`), port 3100 |
| Public API/UI | `https://paperclip-t8tg.srv1829398.hstgr.cloud` (Traefik + Let's Encrypt) |
| Data (host) | `/docker/paperclip-t8tg/data` → container `/paperclip` (`PAPERCLIP_HOME`) |
| Compose | `/docker/paperclip-t8tg/` (and `/docker/traefik/`) |
| DB / storage | embedded PostgreSQL (`…/db`, :54329) + local-disk storage (see `setup-audit.md`) |
| Company | `Flag Seeker` — id `05bc506f-ceb7-49b3-b914-e866a9326064` (issue prefix `FLA`) |
| Admin user | `tyler.p.jewell@gmail.com` (instance admin / owner) |

Full topology and read-only recon: run `./recon.sh`. Scored best-practice audit:
`setup-audit.md` (P1 = remove metered key + strict secrets; P2 = public/explicit URL; etc.).

## SSH in

Key auth is installed — no password needed:
```sh
ssh root@2.25.132.76 'docker ps'
```
Password bootstrap (only if the key breaks): `security find-generic-password -s hostinger-vps-root -a root -w`.
Re-install the key with the `drive-interactive-cli` skill / `../../../deploy/install-tunnel-key.sh`.

## Authenticate to the board (already done — reuse the stored key)

Paperclip has **three** auth schemes (from its OpenAPI `securitySchemes`):

| Scheme | Transport | Use |
|--------|-----------|-----|
| `BoardSessionAuth` | cookie `paperclip_session` (Better Auth) | human web login; bootstrap only |
| `BoardApiKeyAuth` | `Authorization: Bearer pcp_board…` | **durable admin automation — use this** |
| `AgentBearerAuth` | `Authorization: Bearer` (agent key) | an agent acting as itself |

**A durable board API key is already minted and stored** in the macOS keychain (service
`paperclip-board-api-key`, account = the company id). It is instance-admin and grants full
board REST access. The `pc-api.sh` helper reads it automatically — you normally don't touch auth.

If the key is ever missing/revoked, re-mint it (email login → session cookie → mint key). Board
**mutations are CSRF-guarded to a trusted origin**, so cookie POSTs need an `Origin` header:
```sh
API=https://paperclip-t8tg.srv1829398.hstgr.cloud
EMAIL=tyler.p.jewell@gmail.com
PW=$(security find-generic-password -s paperclip-board-admin -a "$EMAIL" -w)
jar=$(mktemp)
# 1) sign in → session cookie
curl -s -c "$jar" -H 'content-type: application/json' \
  --data "$(EMAIL="$EMAIL" PW="$PW" node -e 'console.log(JSON.stringify({email:process.env.EMAIL,password:process.env.PW}))')" \
  "$API/api/auth/sign-in/email" >/dev/null
# 2) mint a board API key (Origin header required for the mutation)
key=$(curl -s -b "$jar" -H "Origin: $API" -H 'content-type: application/json' \
  --data '{"name":"llm-gateway-admin","requestedCompanyId":"05bc506f-ceb7-49b3-b914-e866a9326064"}' \
  "$API/api/board-api-keys" | node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>console.log(JSON.parse(s).token))')
security add-generic-password -a 05bc506f-ceb7-49b3-b914-e866a9326064 -s paperclip-board-api-key -w "$key" -A -U
rm -P "$jar"
```
Gotcha: `paperclipai adapter list --api-key <boardkey>` treats `--api-key` as *agent* auth and
returns `403 Board access required`. For board work use **REST with the Bearer board key** (via
`pc-api.sh`), or `paperclipai connect --persona board` to store a board CLI context.

## Two ways to run admin operations

**A) REST (preferred for board automation)** — the helper injects the board key, base URL, and Origin:
```sh
S=~/Apps/jewell-labs/llm-provider/.claude/skills/paperclip-admin
$S/pc-api.sh GET  /api/adapters
$S/pc-api.sh GET  "/api/companies/$PAPERCLIP_COMPANY/agents"      # $PAPERCLIP_COMPANY preset by the helper
$S/pc-api.sh POST "/api/companies/05bc506f-ceb7-49b3-b914-e866a9326064/agents" '<json>'
$S/pc-api.sh PATCH "/api/agents/<id>" '{"budgetMonthlyCents":500000}'
```
GET reads are proven and never mutate — explore freely. Mutations carry the Origin header.

**B) `paperclipai` CLI (inside the container)** — richer commands, needs a board context:
```sh
ssh root@2.25.132.76 'docker exec paperclip-t8tg-paperclip-1 paperclipai <cmd> --json'
# board-scoped commands first need: docker exec -it … paperclipai connect --persona board
```

## `paperclipai` CLI map (admin-relevant)

Run `paperclipai <group> --help` for exact flags. Groups:

- **agent** — `list · get · create · update · delete · pause · resume · approve · terminate ·
  hire · heartbeat:invoke · claude-login · permissions:update · configuration ·
  config-revisions · config-revision:get · config-revision:rollback`
- **agent-config** `list` · **company** `list/get/current/stats/create/update/archive/export/import/delete`
- **adapter** — `list · install · get · update · override · reload · reinstall · delete ·
  config-schema · models · model-profiles · detect-model · test-environment`
- **budget** `overview/policy:upsert/company:update/agent:update/incident:resolve` ·
  **cost** `summary/by-agent/by-provider/by-project/window-spend/event:create` ·
  **finance** `event:create/events/summary`
- **approval** `list/get/create/approve/reject/request-revision/resubmit/comment`
- **issue** `list/get/create/update/comment/archive/child:create/…` · **project** · **goal** · **routine**
- **token** `agent {create,list,revoke}` · `board {create,list,revoke}` · **access** `whoami`
- **instance** `settings:general[:update]/settings:experimental[:update]/database-backup` ·
  **admin** `user …` · **configure** (`--section llm|database|logging|server|storage|secrets`) ·
  **connect** · **doctor** · **db:backup** · **allowed-hostname** · **environment** · **workspace** · **skill**

## REST API map (base `https://paperclip-t8tg.srv1829398.hstgr.cloud`)

345 endpoints across tags: `agents (39) · issues (77) · access (37) · companies (23) ·
costs (20) · adapters (14) · approvals (10) · environments · execution-workspaces · goals ·
instance · admin · activity · assets · health · auth · cloud-upstreams`. Key ones:

- Adapters: `GET /api/adapters`, `GET /api/adapters/{type}/config-schema`, `POST /api/adapters/install`,
  `PATCH /api/adapters/{type}`, `GET /api/companies/{c}/adapters/{type}/models`, `POST …/test-environment`
- Agents: `GET/POST /api/companies/{c}/agents`, `GET/PATCH/DELETE /api/agents/{id}`,
  `POST /api/agents/{id}/{pause,resume,terminate,approve,wakeup,heartbeat/invoke}`,
  `GET/POST/DELETE /api/agents/{id}/keys`, `GET /api/agents/{id}/configuration`, `…/config-revisions`
- Costs/budgets: `GET /api/companies/{c}/costs/{summary,by-agent,by-provider}`,
  `PATCH /api/companies/{c}/budgets`, `PATCH /api/agents/{id}/budgets`, `GET /api/companies/{c}/budgets/overview`
- Governance: `GET/POST /api/companies/{c}/approvals`, `POST /api/approvals/{id}/{approve,reject}`
- Ops: `GET /api/health`, `POST /api/instance/database-backups`, `GET /api/companies/{c}/activity` (immutable audit)
- Full spec anytime: `pc-api.sh GET /api/openapi.json` or `paperclipai openapi`.

## Adapters & model routing (how agents get their brains)

An agent has `adapterType` + `adapterConfig`. Built-in types include `process` (runs a local
coding CLI — inner `adapter: claude_local | codex_local`), `http` (webhook to a remote agent),
`claude_local`, `grok_local`, `gemini_local`, `openclaw`, etc. Inspect a type's fields with
`pc-api.sh GET /api/adapters/<type>/config-schema`. Custom external adapters implement the
`ServerAdapterModule` contract (`type`, `execute`, `testEnvironment`) and register via
`POST /api/adapters/install`.

**Do not use Hermes** (`hermes_local`, `hermes_gateway`, or any hermes CLI). Canonical model
routing for this fleet is the in-house **llm-provider** via the custom **`gateway_openai`**
adapter only — full wiring in **§ llm-provider gateway integration** below.

**Routing agents to the llm-provider gateway (all three providers):** set
`adapterType: gateway_openai` and point `adapterConfig` at the gateway through the reverse
tunnel. From the container the gateway is reachable at **`http://172.16.0.1:4141/v1`** (bind
the tunnel to the docker gateway IP — the container's `127.0.0.1` is NOT the host's). A creds
bundle is required (`credsFile`, see below). Proven from the container for `claude-*`,
`grok-*`, and local `qwen3-4b`.

## llm-provider gateway integration (the `gateway_openai` adapter)

The only supported Paperclip→model path. Agents reach the gateway through the custom
**`gateway_openai`** external adapter (`integrations/paperclip/gateway-adapter/`) whose
`execute(ctx)` POSTs to `/v1/chat/completions` and streams the reply — direct OpenAI wire, no
hermes, no agent CLI.

**Where the gateway lives:** only on the Mac (launchd `com.jewell-labs.llm-provider`, port
4141). It is Mac-bound — Claude uses the macOS Keychain, local models use the M1 Max GPU, grok
uses `~/.grok` — so it can't run on the Linux VPS; a remote Paperclip reaches *into* the Mac
over the reverse tunnel (Scenario 3).

**Auth model.** `auth.trust_loopback = true` (default) trusts `127.0.0.1`/`::1` keyless.
`auth.trust_loopback = false` (**current — the gateway is tunnel-exposed**) requires a token on
every request, because the reverse tunnel makes VPS traffic look like loopback.

**Tokens are short-lived and rotate — nothing is permanent.** Every mint returns a *bundle*:
`{ access_token (llmgw-…), refresh_token (llmgwr-…), expires_at }`. The adapter presents the
access token and, within ~60s of expiry, POSTs the refresh token to `/auth/refresh`, gets a
fresh bundle, and rewrites the `credsFile` atomically. Refresh **rotates** (the old refresh
token dies on use). `apiKey`/`apiKeyFile` static keys still work for simple use but never rotate.

### Mint the creds bundle

```sh
# admin mint (simplest) -> write the bundle to the creds file:
LLM_PROVIDER_DIR=~/Apps/jewell-labs/llm-provider \
  ~/Apps/jewell-labs/llm-provider/target/release/llm-provider mint tyler.p.jewell@gmail.com \
  > ~/Apps/jewell-labs/llm-provider/.gateway-creds.json
# or headless Google (1h gcloud token) — the response IS the bundle:
curl -s -X POST localhost:4141/auth/google -H content-type:application/json \
  -d "{\"id_token\": \"$(gcloud auth print-identity-token)\"}" \
  > ~/Apps/jewell-labs/llm-provider/.gateway-creds.json
# or xAI device-code:  llm-provider login-xai
```
The local bundle lives at `~/Apps/jewell-labs/llm-provider/.gateway-creds.json` (0600, gitignored).

### Register the adapter (every Paperclip instance)

```sh
node integrations/paperclip/register-adapter.mjs   # writes $PAPERCLIP_HOME/adapter-plugins.json
npx paperclipai run                                 # gateway_openai now selectable
```
Then set an agent's `adapterType: gateway_openai` + the `adapterConfig` from
`integrations/paperclip/adapter-config.json`, edited per scenario:

- **Scenario 1 — Local (Paperclip on this Mac):**
  `{"baseUrl":"http://localhost:4141/v1","model":"grok-4.20-0309-non-reasoning","credsFile":"/Users/studio/Apps/jewell-labs/llm-provider/.gateway-creds.json"}`
  Verify: `node integrations/paperclip/test-gateway-direct.mjs` → 16 checks pass.
- **Scenario 2 — LAN (another home machine):** same, but `baseUrl=http://10.0.0.166:4141/v1`
  (the Mac's LAN IP). Copy the bundle over and point `credsFile` at it. Verify with
  `GATEWAY_URL=http://10.0.0.166:4141 node …test-gateway-direct.mjs`.
- **Scenario 3 — Remote (this VPS):** see below. From the container use
  `baseUrl=http://172.16.0.1:4141/v1` (docker gateway IP, NOT `127.0.0.1`).

### Scenario 3 — Remote (Paperclip on this VPS over the reverse tunnel)

The VPS is a front door, not the host. A reverse SSH tunnel (autossh + launchd, Mac dials the
VPS) exposes the Mac gateway on the VPS loopback; nothing is public.

```sh
# one-time (Mac): brew install autossh; ssh-copy-id root@2.25.132.76; ssh root@2.25.132.76 true
cp deploy/com.jewell-labs.llm-tunnel.plist ~/Library/LaunchAgents/
launchctl load ~/Library/LaunchAgents/com.jewell-labs.llm-tunnel.plist
ssh root@2.25.132.76 'curl -s localhost:4141/healthz'   # {"ok":true}
# push the bundle + adapter to the VPS:
scp ~/Apps/jewell-labs/llm-provider/.gateway-creds.json root@2.25.132.76:/root/llm-provider-creds.json
scp -r integrations/paperclip/gateway-adapter root@2.25.132.76:/root/gateway-openai-adapter
```
On the VPS register the adapter (point `register-adapter.mjs`'s `PKG_DIR` at
`/root/gateway-openai-adapter`) and use
`{"baseUrl":"http://172.16.0.1:4141/v1","model":"grok-4.20-0309-non-reasoning","credsFile":"/root/llm-provider-creds.json"}`.
The adapter refreshes over the tunnel and rewrites `/root/llm-provider-creds.json` in place near
expiry — no hourly re-copy. Topology:
```
Mac: llm-provider :4141 (trust_loopback=false)
  ▲ reverse SSH tunnel (autossh + launchd; Mac dials the VPS)
  │ VPS loopback/172.16.0.1:4141  ⇒  Mac gateway
VPS container: Paperclip ─▶ http://172.16.0.1:4141/v1  (Bearer <access_token from creds bundle>)
```

### Refresh & re-provision / troubleshooting

- Tokens rotate automatically (above); re-mint by hand only if a bundle is lost or its 30-day
  refresh window fully lapses unused — re-run mint, overwrite `.gateway-creds.json`, `scp` it across.
- **401 everywhere:** `trust_loopback=false` + no/invalid token — check `credsFile` resolves,
  holds a live bundle, and the access token isn't past `expires_at` with a dead refresh (re-mint).
- **Remote can't connect:** tunnel down — `cat tunnel.log`, confirm `ssh root@2.25.132.76 true`
  works non-interactively, reload the tunnel agent.
- **Local model 404:** the local backend isn't up — `./launch-local.sh` (see `tune-local-llm`).

## Common admin tasks (recipes)

```sh
S=~/Apps/jewell-labs/llm-provider/.claude/skills/paperclip-admin; C=05bc506f-ceb7-49b3-b914-e866a9326064
$S/pc-api.sh GET "/api/companies/$C/agents"                       # list agents
$S/pc-api.sh GET "/api/agents/<id>/configuration"                # redacted agent config (model/adapter)
$S/pc-api.sh PATCH "/api/agents/<id>" '{"adapterType":"gateway_openai","adapterConfig":{"baseUrl":"http://172.16.0.1:4141/v1","model":"grok-4.5","credsFile":"/root/llm-provider-creds.json"}}'  # switch backend
$S/pc-api.sh POST "/api/agents/<id>/pause" '{}'                   # pause / resume / terminate
$S/pc-api.sh GET "/api/companies/$C/costs/summary"               # spend
$S/pc-api.sh PATCH "/api/agents/<id>/budgets" '{"budgetMonthlyCents":200000}'   # $2,000/mo
$S/pc-api.sh GET "/api/companies/$C/approvals"                   # pending approvals
$S/pc-api.sh GET "/api/companies/$C/activity"                    # immutable audit log
$S/pc-api.sh POST /api/instance/database-backups '{}'           # DB backup
ssh root@2.25.132.76 'docker exec paperclip-t8tg-paperclip-1 paperclipai doctor'   # config health
```
Agent create/hire payloads: required `name`, `role` (ceo/cto/engineer/designer/pm/qa/devops/
researcher/general), `adapterType`, `adapterConfig`; optional `reportsTo`, `title`,
`budgetMonthlyCents`, `heartbeatSchedule{enabled,intervalSec≥30}`. Budgets are in **cents**.

## Configuration & secrets

Config resolves env > `config.json` (`/paperclip/instances/default/config.json`) > defaults.
Change via `paperclipai configure --section <llm|database|logging|server|storage|secrets>`
(needs a restart) or the managed `/docker/paperclip-t8tg/.env` + `docker compose up -d`. Key
levers: `PAPERCLIP_DEPLOYMENT_MODE` (authenticated), `PAPERCLIP_DEPLOYMENT_EXPOSURE`
(private|public), `PAPERCLIP_AUTH_BASE_URL_MODE` (auto|explicit) + `PAPERCLIP_AUTH_PUBLIC_BASE_URL`,
`PAPERCLIP_SECRETS_STRICT_MODE` (blocks inline `*_API_KEY/_TOKEN/_SECRET/_PASSWORD`),
`PAPERCLIP_STORAGE_PROVIDER` (local_disk|s3), `PAPERCLIP_ENABLE_COMPANY_DELETION`. Store
secrets as encrypted vault refs, not inline. See `setup-audit.md` for what to change and why.

**Claude billing — OAuth vs metered (important):** `claude_local` picks billing from the
effective process env: **any non-empty `ANTHROPIC_API_KEY` ⇒ metered API**; otherwise it uses
the container's Claude **Max-subscription OAuth** (`/paperclip/.claude/.credentials.json`,
auto-refreshing). An agent-level `env.ANTHROPIC_API_KEY:""` does NOT help (empty bindings are
dropped → inherits the container value), and strict mode doesn't touch the deployment env. The
durable guard is pinned in `docker-compose.yml` (`environment: ANTHROPIC_API_KEY: ""`), which
overrides `env_file`. **Monitor after any Hostinger redeploy** — it must stay empty:
```sh
ssh root@2.25.132.76 'docker exec paperclip-t8tg-paperclip-1 printenv ANTHROPIC_API_KEY'   # want: empty
```
If it comes back non-empty, re-add `ANTHROPIC_API_KEY: ""` to the compose `environment:` block
and `docker compose up -d` (backup: `docker-compose.yml.pre-oauth.bak`).

## Stay current — read the latest docs before changing anything

Paperclip is open-source and moves fast; do not trust this snapshot for a mutation. First
`WebFetch` the relevant page:
- Raw index: **https://paperclip.ing/llms.txt**
- Doc index: **https://paperclipai-paperclip.mintlify.app/llms.txt** (pages: `https://mintlify.wiki/paperclipai/paperclip/<path>.md`)
- Source/changelog: https://github.com/paperclipai/paperclip
- On-box, version-matched: `paperclipai llm agent-configuration`, `paperclipai doctor`, `pc-api.sh GET /api/openapi.json`

## Safety rules (production instance)

1. **Read before you write.** Prefer GETs, `--json`, `paperclipai doctor`, `docker compose config`,
   `apt-get -s`. For any Paperclip change, check the latest `llms.txt` first.
2. **Never break `ssh.service` or `traefik`** — one locks you out, the other takes Paperclip
   offline. Validate (`sshd -t`, `compose config`) and keep a second SSH session open on restarts.
3. **Manage containers via `/docker/*/docker-compose.yml`**, not `docker rm`/`run`.
4. **Snapshot before risky changes** (Hostinger panel) — embedded DB, no swap, single host.
   Managed files (`.env`, compose) can be overwritten by Hostinger; prefer its dashboard where possible.
5. **Mind resources** (1 vCPU / 3.8 GB / no swap): `free -h` before heavy work.
6. **`terminate` and company `delete` are irreversible**; agent budget exhaustion auto-pauses.
7. **Secrets** stay in the keychain / encrypted vault — never in files, commits, chat, or logs;
   redact env values in output. Revoke the board key if exposed (`paperclipai token board revoke <id>`).

## Files in this skill
- `pc-api.sh` — board REST helper (keychain board key, base URL, Origin). **Primary tool.**
- `recon.sh` — read-only VPS + Docker + Paperclip profile.
- `setup-audit.md` — scored best-practice audit + prioritized fix list (dated).
