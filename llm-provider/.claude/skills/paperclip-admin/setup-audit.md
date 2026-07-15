# Paperclip setup audit — 2026-07-15 (live review via paperclip-admin skill)

Scored against Paperclip's latest docs (`https://paperclip.ing/llms.txt` →
`deployment/production.md`, `deployment/security.md`, `deployment/configuration.md`) using the
skill's tooling: `recon.sh`, `paperclipai doctor`, and live `pc-api.sh` reads (instance
settings, adapters, agents, budgets, costs, admin users, activity). Instance: `paperclip-t8tg`
on `root@2.25.132.76`, Paperclip `2026.626.0`, company `Flag Seeker`.

**Overall: B− / ~70% aligned.** Transport, auth-mode, secrets-at-rest, and backups are
correct. Gaps: metered LLM billing still live, secrets not in strict mode, exposure set to
`private` on a public domain, no cost guardrails, and one agent is erroring.

## Live state observed
- **Agents (2):** `Chief of staff` (ceo) → adapter `claude_local`, model **`claude-opus-4-8`**, idle.
  `Founding Engineer` (engineer) → adapter `grok_local`, **status = error**.
- **Billing:** `ANTHROPIC_API_KEY` present in container env → the CEO agent's Opus calls bill
  **metered**, not via OAuth. `grok_local` uses the grok OIDC subscription.
- **Budgets:** company `budgetMonthlyCents=0`, both agents `0`, no budget policies → **no cost cap**.
- **Auth:** `authenticated` mode; env `PAPERCLIP_AUTH_BASE_URL_MODE=explicit` but
  `EXPOSURE=private` (doctor also reports auth-URL-mode `auto` — env vs config.json disagree; reconcile).
- **Secrets:** local_encrypted, master key file on disk ✓; strict mode **not set (off)** — inline secrets allowed.
- **Backups:** retention configured (daily 7 / weekly 4 / monthly 1) ✓.
- **DB/storage:** embedded PostgreSQL (:54329) + local-disk storage.
- **Admin user:** `tyler.p.jewell@gmail.com` — instance admin, **email UNVERIFIED**.
- **Audit log:** active (100+ recent entries). **Host:** 1 vCPU / 3.8 GB / no swap; disk 24%.

## Scorecard

| Area | Best practice | Ours (live) | Score |
|------|---------------|-------------|-------|
| TLS / reverse proxy | HTTPS behind proxy | Traefik + Let's Encrypt | ✅ |
| Deployment mode | `authenticated` | `authenticated` | ✅ |
| Secrets at rest | encrypted, master key in file | local_encrypted, key file | ✅ |
| Backups | scheduled + retention | daily 7 / weekly 4 / monthly 1 | ✅ |
| Audit log | immutable activity log | active | ✅ |
| Metered LLM key | OAuth / vault refs | `ANTHROPIC_API_KEY` inline; CEO runs Opus on it | ❌ |
| Secrets strict mode | `true` (block inline keys) | off | ❌ |
| Exposure + base URL | `public` + explicit URL (internet-facing) | `private`; `EXPOSURE`/URL-mode inconsistent | ⚠️ |
| Cost guardrails | per-company/agent budgets | all `0`, no policies | ⚠️ |
| Agent health | agents idle/running | `Founding Engineer` (grok_local) = **error** | ⚠️ |
| Company deletion | `false` | default (enabled) | ⚠️ |
| Admin identity | email verified | UNVERIFIED | ⚠️ |
| Database | hosted PostgreSQL 17+ | embedded (managed default) | ⚠️ |
| File storage | S3 | local disk (managed default) | ⚠️ |
| Host resources | — | 1 vCPU / 3.8 GB / no swap | ⚠️ |

## Fix list (prioritized)

**P1 — security / billing (your stated goal)**
1. **[DONE 2026-07-15] Removed `ANTHROPIC_API_KEY`.** Root cause: the metered key was inline on
   line 5 of the managed `/docker/paperclip-t8tg/.env`; Claude Code (`claude_local`) prefers
   `ANTHROPIC_API_KEY` over its OAuth login whenever present, so every Opus run billed metered.
   The CEO agent's own `adapterConfig.env.ANTHROPIC_API_KEY=""` did NOT help — Paperclip treats
   an empty agent-env value as *inherit*, so the container env won. Fix: commented the line in
   `.env` (backup `.env.pre-oauth.bak`), `docker compose up -d`. Verified: `claude -p` returns
   OK with no key, and the **Max-subscription OAuth token auto-refreshed** (`subscriptionType:
   max`). `claude_local` has no billing/apiKey field in this version — the env var was the only lever.
   - **Durability caveat:** `.env` is Hostinger-managed; a panel re-provision could re-add the
     key. There is no reliable *agent-level* guard (empty = inherit). The systemic guard is #2.
2. **[DONE 2026-07-15] Robust metered guard + strict mode.** Verified from the compiled source:
   - `claude_local` (`adapter-claude-local/dist/server/execute.js:43`) picks billing by
     `hasNonEmptyEnvValue(effectiveEnv,"ANTHROPIC_API_KEY") ? "api" : "subscription"`, where
     `effectiveEnv = {...process.env, ...agentEnv}` and **empty agent-env bindings are dropped**.
     So the only lever is the container process env — an agent-level `ANTHROPIC_API_KEY:""` does
     nothing (empty = inherit). **Strict mode does NOT touch this** — `services/secrets.js:517`
     only throws on *non-empty inline sensitive values in agent/company secret bindings*, not the
     deployment env.
   - **Robust guard applied at the compose layer:** added `ANTHROPIC_API_KEY: ""` to the
     paperclip service `environment:` block, which **overrides `env_file`**, so even a
     re-provisioned `.env` with a real key is forced empty → billing `subscription`. Verified:
     container `ANTHROPIC_API_KEY=[]`, `claude -p` → OAUTH_OK on the Max subscription.
   - **Strict mode enabled** (`PAPERCLIP_SECRETS_STRICT_MODE=true`, also in the `environment:`
     block; authenticated mode defaults it on, Hostinger had explicitly disabled it). Safe:
     both agents scanned, no non-empty inline sensitive env bindings. Backups:
     `docker-compose.yml.pre-oauth.bak`, `.env.pre-oauth.bak`.
   - **Residual risk:** a *full* Hostinger re-provision that rewrites `docker-compose.yml` itself
     would drop the guard. Re-check after any Hostinger panel redeploy:
     `docker exec … printenv ANTHROPIC_API_KEY` (want empty) — see the skill's monitoring note.

**P2 — correctness / governance**
3. **Set cost guardrails** — company + per-agent `budgetMonthlyCents` (esp. while any metered
   path exists): `pc-api.sh PATCH /api/companies/<c>/budgets '{"budgetMonthlyCents":N}'`.
4. **Fix the `Founding Engineer` error** — `pc-api.sh GET /api/agents/<id>/configuration` +
   `GET /api/agents/<id>/runtime-state`; likely grok auth/model. Then `clear-error`/`resume`.
5. **Reconcile exposure/URL** — it's internet-facing on a public domain: set
   `EXPOSURE=public` + `PAPERCLIP_AUTH_PUBLIC_BASE_URL=https://paperclip-t8tg.srv1829398.hstgr.cloud`
   consistently (env and config.json currently disagree on URL mode).
6. **Verify the admin email**; **disable company deletion** (`PAPERCLIP_ENABLE_COMPANY_DELETION=false`).

**P3 — durability (optional on a single personal VPS; may fight managed defaults)**
7. Hosted PostgreSQL 17+ · 8. S3 storage · 9. add swap / resize (3.8 GB, no swap).

## Caveats
- Items 5, 7, 8 touch Hostinger-managed `.env`/compose — a panel action can overwrite them;
  prefer the Hostinger dashboard and snapshot first.
- Re-run this review with the skill after changes: `recon.sh`, `paperclipai doctor`, the
  `pc-api.sh` reads above. Re-check `llms.txt` for renamed flags before mutating.
