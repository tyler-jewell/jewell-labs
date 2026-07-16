# Changelog

All notable changes to `@jewell-labs/paperclip-plugin-sendblue` are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Paperclip installs use the package (and manifest) **version** field. Ship this file
in the npm tarball so operators and marketplaces can show release notes when the
plugin is updated.

## [Unreleased]

### Added

- Quality gate: ESLint (strict type-aware), knip dead-code, unified `npm run check`
- Release tooling: `npm run release` bumps package + manifest version + changelog
- CI workflow for the plugin package on push/PR

## [0.1.0] - 2026-07-16

### Added

- Initial SendBlue (iMessage/SMS) Paperclip plugin
- Agent tools: send message/group/reaction/typing, mark-read, create-group, list lines/messages/contacts, get status, evaluate service, account webhook CRUD helpers
- Inbound webhook endpoint with secret verification, E.164 allowlist (deny missing/invalid senders), optional create_issue + wakeup
- Board notify: issue done/created, approval created, agent run failed
- Pure HTTP client layer with fixture unit tests (no network)
- Settings UI page scaffold
