---
name: plugin-configuration
description: >
  Interactively configure the SendBlue Paperclip plugin (jewell-labs.sendblue): interview
  the operator for setup goals, provision company vault secrets (required), write soft
  instance config (no company UUIDs), then run healthcheck + dashboard API tests until
  systems are go. Triggers: configure SendBlue, setup plugin, plugin settings, allowlist
  SMS, vault secrets, Run test console, /plugin-configuration.
---

# SendBlue plugin configuration (interactive)

Operator-facing workflow for **`jewell-labs.sendblue`**. Read package `AGENTS.md` first.
Reuse **`paperclip-admin`** for board auth / SSH when targeting the live VPS.

**Never hardcode** config field lists, tool names, inbound modes, or “current” config values.
Always **introspect** the installed package + live host, then ask the user using **live options**.

---

## Phase 0 — Preconditions

1. Package root: `plugins/paperclip-plugin-sendblue` (or staged path on the host).
2. Auth: board Bearer key (`paperclip-admin` `pc-api.sh` or container
   `board-api-key-sendblue-ops`). Do not print secrets.
3. Discover base URL / container from live env.

If the plugin is missing: install/upgrade via monorepo root `README.md` local-path procedure.

---

## Phase 1 — Introspect (mandatory)

### 1A. Package source of truth

| What | How |
| --- | --- |
| Plugin id / version | `PLUGIN_ID` / `PLUGIN_VERSION` from `src/constants.ts` + `src/version.ts` |
| **Canonical vault keys** | `SECRET_KEYS` in `src/constants.ts` (`sendblue-api-key`, `sendblue-api-secret`, `sendblue-webhook-secret`) |
| Soft config fields | `SendBluePluginConfig` in `src/config.ts` — **no company/agent UUIDs** |
| Agent tools | `TOOL_NAMES` + `TOOL_META` |
| Dashboard tests | `API_TEST_DEFS` in `src/ui/api-test-defs.ts` |
| Health report | `probePluginHealth` / bridge `settings-overview` |

### 1B. Live host

```text
GET  /api/plugins
GET  /api/plugins/jewell-labs.sendblue/health     → plugin system go/no-go
GET  /api/plugins/jewell-labs.sendblue/config     → soft config only
POST /api/plugins/jewell-labs.sendblue/bridge/data
     body: {"key":"settings-overview","params":{}}
GET  /api/companies
GET  /api/companies/{id}/secrets                  → confirm vault keys exist
```

### 1C. Host quirks

- **Plugin config does not store company/project/assignee UUIDs.** Routing is resolved live
  from the board (active company + engineer/CEO agent). Healthcheck reports routing status.
- Instance `POST …/config` may **422** if body includes secret refs or UUIDs — soft fields only.
- Credentials: company **vault secrets** with canonical keys (required for multi-company).
  Optional ops file/env fallbacks exist for bootstrap only — prefer vault.

---

## Phase 2 — Secrets first (required for production)

**Before soft config interview, ensure vault secrets exist** on each company that will use SendBlue.
Same key names → one or many companies can share one SendBlue account (or use different values).

Canonical keys (from live `SECRET_KEYS` — do not invent):

| Key | Purpose |
| --- | --- |
| `sendblue-api-key` | SendBlue API key id |
| `sendblue-api-secret` | SendBlue API secret |
| `sendblue-webhook-secret` | Optional shared secret for inbound webhook verify |

### Create / update (board API)

```text
POST /api/companies/{companyId}/secrets
{"key":"sendblue-api-key","name":"SendBlue API Key","value":"<from operator out-of-band>"}

POST /api/companies/{companyId}/secrets
{"key":"sendblue-api-secret","name":"SendBlue API Secret","value":"…"}

POST /api/companies/{companyId}/secrets
{"key":"sendblue-webhook-secret","name":"SendBlue Webhook Secret","value":"…"}
```

- Never paste secret values into chat, commits, or skill files.
- Operator provides values out-of-band (password manager / `~/.sendblue/credentials.json` on ops Mac).
- For **multiple companies**: create the same three keys on each company (values may match).

### Pass criterion

Worker can resolve credentials: `settings-overview.credentialsConfigured === true`
and `secrets.apiKey` / `secrets.apiSecret` true. If false, do not claim live SMS is ready.

---

## Phase 3 — Soft config interview

Use **`ask_user_question`** with options from Phase 1. Topics (map to live `SendBluePluginConfig` only):

1. **Goal** — soft ops / full live SMS / inbound issues / notify only / diagnose.
2. **From number** — E.164 SendBlue line.
3. **Allowlist** + `emptyMeansDeny`.
4. **Notify number** (must be on allowlist when non-empty).
5. **Inbound mode** — `create_issue` | `log_only` | `ignore` (no UUID fields).
6. **Notify toggles** — `notifyOn*`.
7. **Verification depth** — read tests only · also write APIs · UI only.

**Do not** ask for company UUID / project UUID / assignee UUID as plugin config.
Explain: routing is automatic via board healthcheck.

### Soft config write

```text
POST /api/plugins/jewell-labs.sendblue/config
{"configJson":{ allowlist, fromNumber, notifyNumber, inboundMode, notifyOn*, emptyMeansDeny }}
```

Omit empty `apiKeyRef` / `apiSecretRef` / `webhookSecretRef` unless host accepts them.
Strip any legacy `defaultCompanyId` / `defaultProjectId` / `defaultAssigneeAgentId` from saved config.

---

## Phase 4 — Healthcheck + API tests

### 4A. System go

```text
GET  /api/plugins/jewell-labs.sendblue/health
POST …/bridge/data  {"key":"settings-overview","params":{}}
```

Expect roughly:

| Field | Go |
| --- | --- |
| `credentialsConfigured` | `true` |
| `secrets.apiKey` / `apiSecret` | `true` |
| `boardRouting.ok` | `true` (company resolved) |
| `status` | `ok` or `degraded` (not `error`) |
| `warnings` | empty or non-blocking |

### 4B. Run tests

```text
POST …/bridge/action
{"key":"test_api","params":{"api":"list_lines","params":{}}}
```

Default suite: all `mutates: false` tools from live `API_TEST_DEFS`. No write SMS unless approved.

### 4C. Pass criteria

- Vault secrets provisioned on target companies.
- Soft config saved; no UUID routing keys in config.
- Health: credentials + board routing ok.
- At least `list_lines` returns ok via bridge or UI Run test.

---

## Phase 5 — Report

1. Plugin version + health status  
2. Companies with vault keys (names only, not values)  
3. Soft config (redacted)  
4. Test matrix  
5. Webhook URL: `https://<public-host>/api/plugins/jewell-labs.sendblue/webhooks/inbound`  
6. Remaining blockers  

---

## Safety

- Never paste API secrets into chat, commits, or skills.
- Never widen allowlist without confirmation.
- Prefer `log_only` until secrets + health are green.
- Unit tests (`npm run check`) complement dashboard tests — not a substitute.

---

## Related

| Resource | Role |
| --- | --- |
| Package `AGENTS.md` | System prompt |
| Package `README.md` | Architecture flowchart + ops |
| `paperclip-admin` | SSH, board key, REST |
| `SECRET_KEYS` in `constants.ts` | Canonical vault key names |
