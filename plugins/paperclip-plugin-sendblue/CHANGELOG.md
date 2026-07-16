# Changelog

All notable changes to `@jewell-labs/paperclip-plugin-sendblue` are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Paperclip installs use the package (and manifest) **version** field. Ship this file
in the npm tarball so operators and marketplaces can show release notes when the
plugin is updated.

## [Unreleased]

### Changed

- **Secrets:** canonical company vault keys `sendblue-api-key` / `sendblue-api-secret` /
  `sendblue-webhook-secret` (multi-company by shared names); skill requires provision first
- **Removed plugin config UUIDs** (`defaultCompanyId` / project / assignee) — board routing is
  live-only; healthcheck reports credentials + `boardRouting.ok`
- Inbound board path is **board REST only** (resolve / create / wake) with provisioned board API key
- Credential resolve order: plain → env → ops files → vault (first-class; vault miss silent)
- Typing + SMS-safe agent replies
- Agent tool `TOOL_META` descriptions for LLM-friendly tool context
- **Typing:** `state`/`max_duration_ms`, mark-read before pulse, validate SendBlue
  ERROR bodies, use conversation `fromLine`, refresh on agent run/checkout, stop before reply
- **`notifyOnIssueDone` defaults to false** — optional board SMS on done; agent reply SMS is the primary path

### Added

- Quality gate: ESLint (strict type-aware), knip dead-code, unified `npm run check`
- Release tooling: `npm run release` bumps package + manifest version + changelog
- CI workflow for the plugin package on push/PR
- Settings UI: load/save real plugin config via host API (vault refs, allowlist, inbound, notify)
- Settings UI API console: **Run test** for every core SendBlue tool (read + write)
- Worker bridge: `settings-overview` data + `test_api` action for the console
- Package `AGENTS.md` system prompt (develop / test / release / architecture)
- Module layout: `api/`, `tools/`, `runtime/`, `webhooks/`, `worker/`, `ui/` with ≤300 lines/file
- Grok skill `.grok/skills/plugin-configuration` — interactive setup from live introspection + dashboard tests

### Changed

- Split large modules into focused packages; barrels preserve public import paths
- Inbound conversation loop: create issue + store sender, typing on agent run start, SMS reply on agent comments (needs API credentials)
- Runtime resolve company/agent when config UUIDs cannot be saved on host
- Skip activity.log when companyId is not a real UUID

## [0.1.0] - 2026-07-16

### Added

- Initial SendBlue (iMessage/SMS) Paperclip plugin
- Agent tools: send message/group/reaction/typing, mark-read, create-group, list lines/messages/contacts, get status, evaluate service, account webhook CRUD helpers
- Inbound webhook endpoint with secret verification, E.164 allowlist (deny missing/invalid senders), optional create_issue + wakeup
- Board notify: issue done/created, approval created, agent run failed
- Pure HTTP client layer with fixture unit tests (no network)
- Settings UI page scaffold
