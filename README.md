# jewell-labs

Source of truth for operating **Jewell Labs** on a self-hosted **Paperclip** control plane (Hostinger VPS) and related local tooling.

**Update this file after every setup step.** If a procedure is not written here, it is not the agreed setup.

---

## Current state (2026-07-16)

| Item | Status |
| --- | --- |
| Paperclip instance | **Live** — company **Jewell Labs** active |
| Dashboard | https://paperclip-t8tg.srv1829398.hstgr.cloud |
| Company id | `d2a2da0c-3b25-41b3-a9fa-57a60ae37488` · issue prefix **JEW** |
| Agents | **Platform Lead** (ceo, `claude_local`) · **Forge** (engineer, `claude_local`) |
| Primary codebase | **https://github.com/tyler-jewell/jewell-labs** (`main`) |
| Workspace path (container) | `/paperclip/instances/default/projects/d2a2da0c-…/b033e672-…/jewell-labs` |
| **Grok Build CLI** | Installed + authenticated (`grok` 0.2.101) |
| Claude Code CLI | Authenticated (claude.ai Max) |
| Agent concurrency | **maxConcurrentRuns=1** (VPS is 3.8 GB — higher values OOM/lock SSH) |
| llm-provider / tunnel | **Not** configured yet |
| SendBlue plugin | **Installed (local-path)** — key `jewell-labs.sendblue` status **ready** on live Paperclip (no npm publish) |

**Note:** GitHub `main` currently has the *published* repo (history-agents harness experiments, commit `9fdd074`). Local Mac working tree after the wipe may differ until you push this README.

Health check (no auth):

```bash
curl -sS https://paperclip-t8tg.srv1829398.hstgr.cloud/api/health
# expect: "status":"ok", "bootstrapStatus":"ready"
```

---

## Paperclip instance

| | |
| --- | --- |
| **Public UI / API** | https://paperclip-t8tg.srv1829398.hstgr.cloud |
| **Direct host port** | `http://2.25.132.76:50768` → container `:3100` (TLS is via Traefik on 443) |
| **SSH** | `ssh root@2.25.132.76` (key auth) |
| **Hostinger project** | `/docker/paperclip-t8tg/` |
| **Compose** | `/docker/paperclip-t8tg/docker-compose.yml` |
| **Data volume** | `/docker/paperclip-t8tg/data` → container `/paperclip` (`PAPERCLIP_HOME`) |
| **Image** | `ghcr.io/hostinger/hvps-paperclip:latest` |
| **Reverse proxy** | Traefik (separate project `/docker/traefik/`), hostname `paperclip-t8tg.srv1829398.hstgr.cloud` |
| **DB** | Embedded PostgreSQL inside the container (lives under the data volume) |
| **Version** | Check with `curl -sS …/api/health` → `version` field, or `docker exec paperclip-t8tg-paperclip-1 paperclipai --version` |

Container name: `paperclip-t8tg-paperclip-1`.

---

## Official Paperclip documentation

Read these before changing the instance. Prefer live docs over memory.

| Doc | URL |
| --- | --- |
| **llms.txt index (canonical)** | https://paperclip.ing/llms.txt |
| **Full docs index (Mintlify)** | https://mintlify.wiki/paperclipai/paperclip/llms.txt |
| Site | https://paperclip.ing |
| GitHub | https://github.com/paperclipai/paperclip |
| Quickstart | https://mintlify.wiki/paperclipai/paperclip/quickstart.md |
| Installation | https://mintlify.wiki/paperclipai/paperclip/installation.md |
| Architecture | https://mintlify.wiki/paperclipai/paperclip/docs/start/architecture.md |
| Local deployment | https://mintlify.wiki/paperclipai/paperclip/deployment/local.md |
| Production deployment | https://mintlify.wiki/paperclipai/paperclip/deployment/production.md |
| Configuration | https://mintlify.wiki/paperclipai/paperclip/deployment/configuration.md |
| Security | https://mintlify.wiki/paperclipai/paperclip/deployment/security.md |
| Agent runtime | https://mintlify.wiki/paperclipai/paperclip/docs/agents-runtime.md |
| Adapters overview | https://mintlify.wiki/paperclipai/paperclip/docs/adapters/overview.md |
| External adapters | https://mintlify.wiki/paperclipai/paperclip/docs/adapters/external-adapters.md |
| Custom adapters | https://mintlify.wiki/paperclipai/paperclip/agents/custom-adapters.md |
| Adapter guide | https://mintlify.wiki/paperclipai/paperclip/guides/adapters.md |
| Plugins (spec) | https://github.com/paperclipai/paperclip/blob/master/doc/plugins/PLUGIN_SPEC.md |
| Plugin authoring | https://github.com/paperclipai/paperclip/blob/master/doc/plugins/PLUGIN_AUTHORING_GUIDE.md |
| Local plugin dev | https://github.com/paperclipai/paperclip/blob/master/doc/plugins/LOCAL_PLUGIN_DEVELOPMENT.md |
| API intro | https://mintlify.wiki/paperclipai/paperclip/api/introduction.md |
| OpenAPI (on instance) | `GET https://paperclip-t8tg.srv1829398.hstgr.cloud/api/openapi.json` (auth as required) |
| CLI overview | https://mintlify.wiki/paperclipai/paperclip/cli/overview.md |

On-box, version-matched help:

```bash
ssh root@2.25.132.76 'docker exec paperclip-t8tg-paperclip-1 paperclipai doctor'
ssh root@2.25.132.76 'docker exec paperclip-t8tg-paperclip-1 paperclipai --help'
```

---

## Initial setup (Hostinger catalog instance)

This is how the **current** instance is provisioned. Do not reinvent paths.

### 1. Prerequisites

- Hostinger VPS with Docker Compose catalog support
- Traefik catalog app installed first (TLS + routing)
- SSH key for `root@2.25.132.76`
- Paperclip installed from Hostinger’s Docker catalog as project **`paperclip-t8tg`**

### 2. Layout on the VPS

```text
/docker/
  traefik/                 # host network, Let's Encrypt
  paperclip-t8tg/
    docker-compose.yml     # image + Traefik labels + volume
    .env                   # ADMIN_*, TRAEFIK_HOST, PUBLIC_PORT, VPS_IP, TZ
    data/                  # PAPERCLIP_HOME (DB, config, secrets, workspaces)
```

Important:

- **All durable Paperclip state** is under `data/` (mapped to `/paperclip`).
- Container recreate **keeps** state; removing or replacing `data/` is a full product reset.
- Hostinger panel redeploys can rewrite `.env` / compose — re-check after panel actions.

### 3. First login (board)

1. Open **https://paperclip-t8tg.srv1829398.hstgr.cloud**
2. Sign in with the bootstrap admin (`ADMIN_EMAIL` in `/docker/paperclip-t8tg/.env`).  
   Password lives only on the VPS `.env` / your password manager — **never commit it**.
3. Clear old browser cookies if you previously used this hostname (stale sessions return 401).
4. Expect **no companies** on a fresh wipe. Create a company only when intentionally starting product setup (next steps — not done yet).

### 4. Health and process checks

```bash
# Public
curl -sS https://paperclip-t8tg.srv1829398.hstgr.cloud/api/health

# On VPS
ssh root@2.25.132.76 'docker ps'
ssh root@2.25.132.76 'cd /docker/paperclip-t8tg && docker compose ps'
ssh root@2.25.132.76 'docker exec paperclip-t8tg-paperclip-1 paperclipai doctor'
```

### 5. CLI inside the container

```bash
ssh root@2.25.132.76 \
  'docker exec paperclip-t8tg-paperclip-1 paperclipai <command> --json'
```

Board-scoped CLI often needs an interactive board context first:

```bash
ssh -t root@2.25.132.76 \
  'docker exec -it paperclip-t8tg-paperclip-1 paperclipai connect --persona board'
```

Prefer **REST + board API key** for automation once a key is minted (document that key’s storage here when added — not yet).

### 6. Safe restart (keeps data)

```bash
ssh root@2.25.132.76 'cd /docker/paperclip-t8tg && docker compose up -d'
```

### 7. Full product reset (Level 1 — data only)

Use only when intentionally wiping companies/agents/config. Keeps compose + Traefik + hostname.

```bash
ssh root@2.25.132.76 'bash -s' <<'EOF'
set -euo pipefail
cd /docker/paperclip-t8tg
docker compose down
ts=$(date -u +%Y%m%dT%H%M%SZ)
mv data "data.destroyed-$ts"   # optional archive; delete when sure
mkdir -p data && chown 1000:1000 data
docker compose pull
docker compose up -d
curl -sS http://127.0.0.1:50768/api/health
EOF
```

Then re-login via the dashboard. Old board API keys and browser sessions are invalid.

**Do not** remove `/docker/traefik` as part of a Paperclip reset.

---

## Host coding CLIs (inside Paperclip container)

Paperclip local adapters run CLI tools **inside** `paperclip-t8tg-paperclip-1`.
Install and auth as user **`node`** with **`HOME=/paperclip`** so credentials live on the
data volume and survive container recreate.

Docs: [Grok Local adapter](https://docs.paperclip.ing/reference/adapters/grok-local) ·
adapter type `grok_local` · default model `grok-build`.

### Grok Build (`grok`)

**Install (already done 2026-07-16)** — official installer into the container:

```bash
ssh root@2.25.132.76 'docker exec -u root paperclip-t8tg-paperclip-1 bash -lc "
  export GROK_BIN_DIR=/usr/local/bin HOME=/paperclip
  curl -fsSL https://x.ai/cli/install.sh | bash
  chown -R node:node /paperclip/.grok
"'
```

- Binary: `/usr/local/bin/grok` (symlink into `/paperclip/.grok/downloads/…`)
- Config/auth home: `/paperclip/.grok/` (host: `/docker/paperclip-t8tg/data/.grok/`)

**Verify without auth:**

```bash
ssh root@2.25.132.76 \
  'docker exec -u node -e HOME=/paperclip paperclip-t8tg-paperclip-1 grok --version'
# expect: grok 0.2.x …
```

**Authenticate (you run this interactively)** — preferred on VPS: device code:

```bash
ssh -t root@2.25.132.76 \
  'docker exec -it -u node -e HOME=/paperclip \
    paperclip-t8tg-paperclip-1 grok login --device-auth'
```

Alternative (browser OAuth if the TTY flow works for you):

```bash
ssh -t root@2.25.132.76 \
  'docker exec -it -u node -e HOME=/paperclip \
    paperclip-t8tg-paperclip-1 grok login --oauth'
```

**After login, confirm:**

```bash
ssh root@2.25.132.76 \
  'docker exec -u node -e HOME=/paperclip paperclip-t8tg-paperclip-1 grok models'
# expect: logged in / available models including grok-build
```

Optional API key path (instead of subscription login): company secret `XAI_API_KEY` as
`secret_ref` on a `grok_local` agent env — see Paperclip Grok Local docs. Prefer `grok login`
when using a SuperGrok / xAI subscription on the host.

**Note:** Hostinger image upgrades that replace the container filesystem may drop
`/usr/local/bin/grok` until you re-run install; `/paperclip/.grok` auth usually persists on the volume.

### Claude Code (`claude`)

Already used for OAuth on this host:

```bash
ssh -t root@2.25.132.76 \
  'docker exec -it -u node -e HOME=/paperclip -e ANTHROPIC_API_KEY= \
    paperclip-t8tg-paperclip-1 claude auth login'
```

---

## SendBlue plugin (local-path install — no npm)

Package: [`plugins/paperclip-plugin-sendblue`](./plugins/paperclip-plugin-sendblue) (`@jewell-labs/paperclip-plugin-sendblue`).

| | |
| --- | --- |
| **Plugin key** | `jewell-labs.sendblue` |
| **Install method** | **Core local-path only** (`isLocalPath: true`) — **not** the dashboard npm dialog, **not** npm publish |
| **Staged path (host)** | `/docker/paperclip-t8tg/data/plugins/paperclip-plugin-sendblue` |
| **Staged path (container)** | `/paperclip/plugins/paperclip-plugin-sendblue` |
| **Live status** | **ready** / healthy on Hostinger Paperclip (2026-07-16) |
| **Dev suite** | `cd plugins/paperclip-plugin-sendblue && npm run check` |

### Why not the dashboard “npm Package Name” dialog?

That UI only accepts registry packages. We deliberately **do not** publish this package to npm. Paperclip’s core CLI/API supports local absolute paths — that is the supported non-npm path.

### Board auth (required)

Instance is `deploymentMode: authenticated`. Plugin install needs board access.

1. Sign in with Hostinger bootstrap admin (`ADMIN_EMAIL` / `ADMIN_PASSWORD` in container env):

```bash
# From inside the Paperclip container. Origin MUST match PAPERCLIP_PUBLIC_URL
# (on this host: http://paperclip-t8tg.srv1829398.hstgr.cloud — not https, not localhost).
curl -sS -c /tmp/pc.ck -b /tmp/pc.ck \
  -H "Content-Type: application/json" \
  -H "Origin: http://paperclip-t8tg.srv1829398.hstgr.cloud" \
  -X POST http://127.0.0.1:3100/api/auth/sign-in/email \
  --data "{\"email\":\"$ADMIN_EMAIL\",\"password\":\"$ADMIN_PASSWORD\"}"
```

2. Mint a board API key (store **only** on the instance, never in git):

```bash
curl -sS -b /tmp/pc.ck \
  -H "Content-Type: application/json" \
  -H "Origin: http://paperclip-t8tg.srv1829398.hstgr.cloud" \
  -X POST http://127.0.0.1:3100/api/board-api-keys \
  --data '{"name":"sendblue-ops","expiresAt":null}'
# Save .token → /paperclip/instances/default/secrets/board-api-key-sendblue-ops (chmod 600)
```

### Stage + install (repeatable)

On a build machine:

```bash
cd plugins/paperclip-plugin-sendblue
npm run check   # lint, typecheck, deadcode, test, build → dist/
```

On the VPS (from monorepo checkout or rsync of the package):

```bash
# rsync package (exclude node_modules) into the data volume
rsync -az --delete --exclude node_modules --exclude .git \
  ./plugins/paperclip-plugin-sendblue/ \
  root@2.25.132.76:/docker/paperclip-t8tg/data/plugins/paperclip-plugin-sendblue/

# Or run the wrapper (on the VPS host):
# bash plugins/paperclip-plugin-sendblue/scripts/stage-and-install.sh /path/to/package
```

Inside the container — **critical**: `NODE_ENV=production` skips installs unless SDK is a real `dependencies` entry. Install runtime deps, then local-path install:

```bash
docker exec -u node -e HOME=/tmp -e NODE_ENV=development paperclip-t8tg-paperclip-1 \
  bash -c 'cd /paperclip/plugins/paperclip-plugin-sendblue && npm install --omit=dev'

TOKEN=$(docker exec paperclip-t8tg-paperclip-1 cat /paperclip/instances/default/secrets/board-api-key-sendblue-ops)

docker exec paperclip-t8tg-paperclip-1 \
  paperclipai plugin install --local /paperclip/plugins/paperclip-plugin-sendblue \
  --api-base http://127.0.0.1:3100 --api-key "$TOKEN"

# Equivalent API:
# POST /api/plugins/install
# {"packageName":"/paperclip/plugins/paperclip-plugin-sendblue","isLocalPath":true}
# Authorization: Bearer <board-api-key>

docker exec paperclip-t8tg-paperclip-1 \
  paperclipai plugin enable jewell-labs.sendblue \
  --api-base http://127.0.0.1:3100 --api-key "$TOKEN"
```

### Soft operator config (no SendBlue secrets yet)

```bash
docker exec paperclip-t8tg-paperclip-1 \
  paperclipai plugin config:set jewell-labs.sendblue \
  --api-base http://127.0.0.1:3100 --api-key "$TOKEN" \
  --payload-json '{"configJson":{"allowlist":[],"emptyMeansDeny":true,"inboundMode":"log_only","notifyOnIssueDone":false}}'
```

When SendBlue credentials exist, put them in the **company vault** and set `apiKeyRef` / `apiSecretRef` / `webhookSecretRef` / `fromNumber` / allowlist numbers in `configJson` (see package README). Live SMS is out of band of install verification.

### Verify (CLI + API + browser asset)

```bash
paperclipai plugin list --api-base http://127.0.0.1:3100 --api-key "$TOKEN"
paperclipai plugin inspect jewell-labs.sendblue --api-base http://127.0.0.1:3100 --api-key "$TOKEN"
paperclipai plugin health jewell-labs.sendblue --api-base http://127.0.0.1:3100 --api-key "$TOKEN"
# expect: status=ready, healthy=true

paperclipai plugin tools --api-base http://127.0.0.1:3100 --api-key "$TOKEN" --json
# expect: 15 tools namespaced jewell-labs.sendblue:*

# UI bundle (settings page) served by core host:
curl -sS -o /dev/null -w "%{http_code}\n" \
  http://127.0.0.1:3100/_plugins/<plugin-uuid>/ui/index.js
# expect: 200; body contains SendBlueSettingsPage
```

Webhook URL after public HTTPS (when secrets are live):

```text
https://paperclip-t8tg.srv1829398.hstgr.cloud/api/plugins/jewell-labs.sendblue/webhooks/inbound
```

### Reinstall / upgrade local path

Re-rsync `dist/` + sources, re-run `npm install --omit=dev` under `NODE_ENV=development` in the staged dir, then:

```bash
paperclipai plugin upgrade jewell-labs.sendblue --api-base http://127.0.0.1:3100 --api-key "$TOKEN"
# or disable → enable after file changes (local-path watcher also reloads dist/)
```

### Troubleshooting

| Symptom | Fix |
| --- | --- |
| `403 Board access required` | Mint board API key (above); pass `--api-key` |
| `Invalid origin` on sign-in | Use `Origin` = `PAPERCLIP_PUBLIC_URL` (http host, not localhost) |
| Worker: `Cannot find package '@paperclipai/plugin-sdk'` | `NODE_ENV=development npm install --omit=dev` in staged package; ensure SDK is in `dependencies` |
| Dashboard npm dialog fails | Expected — use CLI/API local-path, not packageName |
| Config `422 secret references disabled` | Use CLI `config:set --payload-json '{"configJson":{...}}'` without vault refs until company-scoped secrets land |

Package-level tools, allowlist, and API surface: [plugins/paperclip-plugin-sendblue/README.md](./plugins/paperclip-plugin-sendblue/README.md).

---

## Operating principles

1. **README is SSoT** — every durable procedure lands here in the same PR/session as the change.
2. **Paperclip core is not forked** — operate via UI, `paperclipai` CLI, REST, external adapters/plugins.
3. **Secrets never in git** — vault, keychain, or VPS `.env` only; document *names* and *locations*, not values.
4. **Read live docs** (`llms.txt`) before non-trivial mutations.
5. **Mind the host** — ~4 GB RAM, no swap; avoid heavy work on the VPS when possible.
6. **Build slowly** — one setup step at a time; update this file after each.

---

## Roadmap (not done yet)

Document each item here when completed:

- [x] Create first company (Jewell Labs)
- [x] Grok Build CLI installed + auth + smoke
- [x] Claude auth + smoke
- [x] Tie Onboarding project to `tyler-jewell/jewell-labs` (primary git workspace + clone)
- [x] Platform Lead verified repo via JEW-6 (done)
- [x] Board API key for automation (instance file `/paperclip/instances/default/secrets/board-api-key-sendblue-ops` — never git)
- [ ] Push local README SSoT to GitHub `main` (remote still has harness experiments)
- [ ] Model gateway (llm-provider) + reverse tunnel
- [ ] Hire/use `grok_local` agent (CLI ready; model id `grok-4.5`)
- [ ] Environments / SSH / sandboxes (if needed)
- [x] Messaging plugin package (SendBlue) developed + unit-tested in-repo
- [x] Messaging (SendBlue) live install on VPS via local-path (no npm)
- [ ] Keep `maxConcurrentRuns=1` on this VPS unless upgraded

---

## Changelog (setup log)

| Date | Change |
| --- | --- |
| 2026-07-16 | Hard-reset monorepo to README-only; Level-1 wiped Paperclip `data/` (fresh instance, 0 companies); cleaned VPS archive/caches; wrote this initial SSoT README |
| 2026-07-16 | Installed Grok Build CLI 0.2.101 in Paperclip container (`/usr/local/bin/grok`); auth pending `grok login --device-auth` |
| 2026-07-16 | Grok login complete (grok.com); models `grok-4.5` (default), `grok-composer-2.5-fast`. Smoke: `CLAUDE_OK` + `GROK_OK` as node/`HOME=/paperclip` |
| 2026-07-16 | Added `plugins/paperclip-plugin-sendblue` — community-shaped Paperclip plugin (tools, webhooks, allowlist, notify). 68 unit tests + quality gate. **Not** installed on VPS yet |
| 2026-07-16 | Plugin quality: ESLint strict type-aware, knip deadcode, `npm run check`, release/version sync, CHANGELOG shipped for npm consumers |
| 2026-07-16 | Installed SendBlue plugin on live Paperclip via **local-path** (`jewell-labs.sendblue` ready); board key + stage path + ops procedure documented here (no npm) |
| 2026-07-16 | Company Jewell Labs (`d2a2da0c-…`); project Onboarding wired to `github.com/tyler-jewell/jewell-labs` primary workspace; clone + GitHub PAT secret; agents cwd set; JEW-6 Platform Lead explained repo. Capped `maxConcurrentRuns=1` after load~50 OOM/SSH lock |
