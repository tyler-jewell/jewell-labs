# @jewell-labs/paperclip-plugin-sendblue

SendBlue (iMessage / SMS / RCS) connector plugin for [Paperclip](https://github.com/paperclipai/paperclip).

Outbound agent tools, inbound webhooks, E.164 allowlists, and board event notifications. Issues remain the system of record.

**Status:** installed on Jewell Labs Paperclip via **local-path** (plugin key `jewell-labs.sendblue`, status ready). No npm registry publish required. Full ops procedure is in the monorepo root **`README.md`** (SSoT).

## Features

| Area | What |
| --- | --- |
| **Outbound tools** | `send_message`, `send_group_message`, `send_reaction`, `send_typing_indicator`, `mark_read`, `create_group`, `list_lines`, `list_messages`, `get_status`, `list_contacts`, `add_contact`, `evaluate_service`, account webhook list/create/delete |
| **Inbound** | `webhooks.receive` endpoint `inbound` — normalize receive / status / call_log payloads, verify shared secret, allowlist sender, optional `create_issue` + agent wakeup |
| **Notify** | iMessage on `issue.created`, `issue.updated` (done), `approval.created`, `agent.run.failed` |
| **Safety** | Strict E.164, empty-allowlist-deny by default, pure request builders (no accidental live HTTP in unit tests) |
| **Secrets** | `apiKeyRef` / `apiSecretRef` / `webhookSecretRef` (Paperclip vault) or plain keys for tests |

## Install (when you are ready)

Do **not** install into production Paperclip until you have run the local suite and reviewed config.

### Local path (production path for this repo)

```bash
cd plugins/paperclip-plugin-sendblue
npm install
npm run check    # lint + typecheck + deadcode + test + build
```

Install into Paperclip with **core** local-path only (see monorepo root `README.md`):

```bash
# package must be readable inside the container; runtime deps must be installed
# (NODE_ENV=production skips peer-only installs — keep SDK in dependencies)
paperclipai plugin install --local /paperclip/plugins/paperclip-plugin-sendblue \
  --api-base http://127.0.0.1:3100 --api-key "$BOARD_API_KEY"
```

Helper: `scripts/stage-and-install.sh` (run on VPS host after rsync).

### Package identity

| Field | Value |
| --- | --- |
| npm name | `@jewell-labs/paperclip-plugin-sendblue` |
| plugin id | `jewell-labs.sendblue` |
| worker | `./dist/worker.js` |
| manifest | `./dist/manifest.js` |
| UI | `./dist/ui/` |

## Configuration

### Vault secrets (recommended)

| Vault key (example) | Config field |
| --- | --- |
| `sendblue-api-key` | `apiKeyRef` |
| `sendblue-api-secret` | `apiSecretRef` |
| `sendblue-webhook-secret` | `webhookSecretRef` |

### Instance config fields

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `apiKey` / `apiSecret` | string | — | Plain credentials (tests only) |
| `apiKeyRef` / `apiSecretRef` | string | — | Vault refs for production |
| `fromNumber` | E.164 | — | Default SendBlue line |
| `webhookSecret` / `webhookSecretRef` | string | — | Shared secret for inbound |
| `allowlist` | string[] or CSV | `[]` | Outbound recipients + inbound senders |
| `emptyMeansDeny` | bool | `true` | Empty allowlist blocks all SMS |
| `notifyNumber` | E.164 | — | Must be on allowlist |
| `notifyOnIssueDone` | bool | `true` | |
| `notifyOnIssueCreated` | bool | `false` | |
| `notifyOnApprovalCreated` | bool | `true` | |
| `notifyOnAgentError` | bool | `true` | |
| `inboundMode` | `create_issue` \| `log_only` \| `ignore` | `create_issue` | |
| `defaultCompanyId` | UUID | — | Required for issue creation |
| `defaultProjectId` | UUID | — | Optional |
| `defaultAssigneeAgentId` | UUID | — | Assign + `requestWakeup` |

### Example config JSON

```json
{
  "apiKeyRef": "sendblue-api-key",
  "apiSecretRef": "sendblue-api-secret",
  "webhookSecretRef": "sendblue-webhook-secret",
  "fromNumber": "+16452067656",
  "allowlist": ["+15551112222"],
  "notifyNumber": "+15551112222",
  "notifyOnIssueDone": true,
  "notifyOnApprovalCreated": true,
  "inboundMode": "create_issue",
  "defaultCompanyId": "<company-uuid>",
  "defaultAssigneeAgentId": "<agent-uuid>"
}
```

## Webhooks

After install, register with SendBlue (HTTPS only):

```
POST https://api.sendblue.com/api/account/webhooks
{
  "webhooks": [{
    "url": "https://<paperclip-host>/api/plugins/jewell-labs.sendblue/webhooks/inbound",
    "secret": "<same as webhookSecret>"
  }],
  "type": "receive"
}
```

SendBlue should send the secret in `x-webhook-secret` or `x-sendblue-secret`. This plugin also accepts HMAC hex in `x-sendblue-signature` over the raw body.

| Kind | Handling |
| --- | --- |
| Inbound message (`is_outbound: false`) | Valid E.164 `from_number` **required** + allowlist → create issue / log. Missing or non-E.164 sender → **403** (never creates issues) |
| Outbound status | Normalized + logged |
| `call_log` | Normalized + logged |

Bad secret → handler throws (host fails delivery). Allowlist deny → ack without processing (no retry storm).

## Agent tools

Tools are namespaced by plugin id at runtime. Parameters are E.164 where applicable; outbound numbers are allowlist-enforced.

Auth headers on every SendBlue call:

- `sb-api-key-id`
- `sb-api-secret-key`

Base URL: `https://api.sendblue.com`

## Development / quality gate

Single entrypoint for CI and humans:

```bash
npm run check
```

| Step | Script | What |
| --- | --- | --- |
| 1 | `version:check` | `package.json` version === `src/version.ts` (manifest) |
| 2 | `lint` | ESLint 9 + typescript-eslint **strictTypeChecked** |
| 3 | `typecheck` | `tsc --noEmit` (strict + noUncheckedIndexedAccess, noUnused*) |
| 4 | `deadcode` | [knip](https://github.com/webpro-nl/knip) unused files/deps |
| 5 | `test` | vitest (68 unit tests, no network) |
| 6 | `build` | `tsc` + UI esbuild → `dist/` |

Also available:

| Script | Purpose |
| --- | --- |
| `npm run lint:fix` | Auto-fix ESLint where safe |
| `npm run deadcode:all` | Knip including unused export report |
| `npm run release -- patch\|minor\|major` | Bump version + changelog |
| `npm run prepublishOnly` | Runs `check` before `npm publish` |

GitHub Actions: `.github/workflows/paperclip-plugin-sendblue.yml` runs `npm run check` on push/PR to this package.

### Architecture

```
src/
  version.ts        # PLUGIN_VERSION (synced with package.json)
  sendblue-api.ts   # pure request builders + parsers + optional Client
  allowlist.ts      # E.164 allow / deny
  e164.ts
  webhooks.ts       # normalize + verify (pure)
  tools.ts          # agent tool schemas + buildToolRequest
  config.ts         # validateConfig
  notify-format.ts  # short SMS bodies
  worker-logic.ts   # executeTool / inbound plan / notify (no runWorker)
  worker.ts         # definePlugin + runWorker (SDK only)
  manifest.ts
  ui/index.tsx
```

**Contract:** unit tests never import `worker.ts` (avoids `runWorker` side effects). Host wiring is covered via `worker-logic` with mock `fetch`.

## Versioning & releasing (Paperclip consumers)

Paperclip production installs use **npm packages** (`packageName`), not git URLs. Consumers pick up updates when they **update/reinstall** the package version.

| Field | Where | Role |
| --- | --- | --- |
| npm `version` | `package.json` | Registry / install target (`@jewell-labs/paperclip-plugin-sendblue@x.y.z`) |
| Manifest `version` | `src/version.ts` → `PLUGIN_VERSION` | Shown by Paperclip after install |
| Release notes | `CHANGELOG.md` (Keep a Changelog) | Shipped in the npm tarball (`files`) |

### Cut a release

```bash
# 1. Write notes under ## [Unreleased] in CHANGELOG.md
# 2. Bump (updates package.json + version.ts + moves Unreleased → dated section)
npm run release -- patch   # or minor | major | 1.2.3

# 3. Quality gate
npm run check

# 4. Commit + tag (from monorepo root is fine)
git add plugins/paperclip-plugin-sendblue
git commit -m "release(sendblue): v$(node -p "require('./plugins/paperclip-plugin-sendblue/package.json').version")"
git tag "paperclip-plugin-sendblue-v$(node -p "require('./plugins/paperclip-plugin-sendblue/package.json').version")"

# 5. Publish to npm (from this package directory)
npm publish --access public

# 6. Push code + tag
git push && git push --tags
```

### How Paperclip consumers get the update

1. **npm install path:** operator updates the plugin package (UI “update plugin” / reinstall `packageName` at the new semver). Paperclip reads the new manifest `version`.
2. **Release notes in Paperclip:** the host does **not** currently guarantee a first-class release-notes panel for community plugins. Shipping `CHANGELOG.md` in the package (and linking it from the npm package page / GitHub Releases) is the portable way for operators to see notes. If Paperclip later surfaces package readme/changelog, it will already be present.
3. **localPath installs:** reinstall/rebuild from this directory after pull — version comes from the built package.

### Optional live smoke (manual)

```bash
export SENDBLUE_API_KEY=...
export SENDBLUE_API_SECRET=...
export SENDBLUE_FROM_NUMBER=+1...
# not part of CI — use a small node -e or REPL against SendBlueClient
```

## Comparison to Paperclip chat plugins

Modeled after community messaging plugins (Telegram/Slack/Discord) and the GitHub Issues plugin package contract:

- `package.json` → `paperclipPlugin.{manifest,worker,ui}`
- `definePlugin` + `runWorker`
- vault secret refs
- allowlists
- board event → notify
- inbound → issue / activity

SendBlue uses **webhooks** (not long-poll) and **E.164 phones** instead of chat IDs.

## License

MIT — see [LICENSE](./LICENSE).

## References

- [SendBlue API v2](https://docs.sendblue.com/api-v2/)
- [SendBlue webhooks](https://docs.sendblue.com/getting-started/webhooks/)
- [Paperclip PLUGIN_SPEC](https://github.com/paperclipai/paperclip/blob/master/doc/plugins/PLUGIN_SPEC.md)
- [plugin-sdk](https://www.npmjs.com/package/@paperclipai/plugin-sdk)
