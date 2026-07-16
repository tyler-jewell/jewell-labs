# AGENTS.md — paperclip-plugin-sendblue

System prompt for any agent working in this package. Follow it exactly.

## What this is

**SendBlue (iMessage/SMS/RCS) connector** for [Paperclip](https://github.com/paperclipai/paperclip).

| | |
| --- | --- |
| **npm** | `@jewell-labs/paperclip-plugin-sendblue` |
| **plugin id** | `jewell-labs.sendblue` |
| **Install** | Local-path on Jewell Labs Paperclip |
| **Host** | Paperclip plugin worker + settings UI |

**Does:**

- Agent tools for SendBlue APIs (send, list, contacts, webhooks, …)
- Inbound webhooks → allowlist → optional `create_issue` + agent wakeup + SMS reply loop
- Board event notify SMS (issue done/created, approval, agent error)
- Settings UI + healthcheck (`settings-overview` / `onHealth`)

**Does not:** custom agent harness, Hermes, monorepo pollers, hardcoded production secrets,
or **plugin-level company/agent UUID routing**.

## Secrets (required) vs soft config

### Vault secrets (multi-company)

Canonical keys in `SECRET_KEYS` (`src/constants.ts`):

| Key | Use |
| --- | --- |
| `sendblue-api-key` | API key id |
| `sendblue-api-secret` | API secret |
| `sendblue-webhook-secret` | Inbound webhook shared secret |

- Create on **each company** that should use SendBlue (same names → multi-company),
  or provision the same values via env / ops files for webhook workers.
- Worker resolves (first-class): plain → **env** → **ops files** → vault ref → canonical vault key.
- Board inbound actions (resolve / create / wake) use the **board REST client** + board API key.
- Never log resolved values. Never commit secrets.

### Soft instance config only

Allowlist, `fromNumber`, `notifyNumber`, `inboundMode`, notify toggles ( **`notifyOnIssueDone` defaults off** — conversation SMS reply is the primary user path), optional vault **refs**.

**Never** store `defaultCompanyId` / `defaultProjectId` / `defaultAssigneeAgentId` in plugin config.
Board routing is resolved live (`resolveCompanyAndAgent`) and reported by **healthcheck**.

## Architecture

```
src/
  api/           # Pure HTTP prepare/parse/execute
  tools/         # names, schemas, meta (agent-facing descriptions), buildToolRequest
  runtime/       # executeTool, inbound plan, notify, conversation SMS
  webhooks/      # normalize, verify, handle
  worker/        # config-load, health, bridge, tools, on-webhook, wake, inbound-effects
  ui/            # Settings form (soft fields) + API console
  ops-paths.ts   # Provisioned env/file paths for secrets + board API key
  config.ts      # validateConfig (soft + credential presence)
  worker.ts      # definePlugin + runWorker
```

**Hard boundaries:**

1. Unit tests never import `worker.ts`.
2. Pure layers (`api/`, `tools/`, `runtime/`, `webhooks/`) have no Paperclip SDK.
3. Secrets never appear in logs or test fixtures as production values.
4. Allowlist: empty + `emptyMeansDeny` blocks all SMS; missing inbound E.164 always denied.
5. ≤300 lines per `src/` / `tests/` file.

## Operator skill

```text
.grok/skills/plugin-configuration/SKILL.md
```

Slash: `/plugin-configuration`. **Secrets first**, then soft config, then health + `test_api`.

## Develop / test / deploy

```bash
cd plugins/paperclip-plugin-sendblue
npm install
npm run check   # version + lint + typecheck + knip + test + build
```

Deploy: rsync package → stage `npm install --omit=dev` → plugin disable/enable.
Soft config: instance `POST /api/plugins/jewell-labs.sendblue/config` without UUID routing fields.

## Do / Don't

| Do | Don't |
| --- | --- |
| Provision `SECRET_KEYS` on each company | Put company UUIDs in plugin config |
| Use healthcheck for system go/no-go | Assume create_issue works without board routing |
| Agent-friendly tool meta + SMS-safe reply formatting | Dump raw markdown tables into SMS |
| Keep pure unit tests (mock fetch) | Hit live SendBlue in CI |
