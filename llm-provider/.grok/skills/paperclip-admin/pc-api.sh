#!/usr/bin/env bash
# pc-api.sh — call the Paperclip board REST API with the keychain-stored board API key.
#
#   pc-api.sh GET  /api/adapters
#   pc-api.sh GET  "/api/companies/$PAPERCLIP_COMPANY/agents"
#   pc-api.sh POST "/api/companies/$PAPERCLIP_COMPANY/agents" '{"name":"...","role":"engineer",...}'
#   pc-api.sh PATCH "/api/agents/<id>" '{"budgetMonthlyCents":500000}'
#
# Auth: instance-admin board API key (Bearer). Reads work; mutations include an Origin header
# (board mutations are CSRF-guarded to a trusted origin — API-key + Origin satisfies it).
# Env overrides: PAPERCLIP_API, PAPERCLIP_COMPANY. Add --json-pretty as a trailing flag to pretty-print.
set -euo pipefail
API="${PAPERCLIP_API:-https://paperclip-t8tg.srv1829398.hstgr.cloud}"
COMPANY="${PAPERCLIP_COMPANY:-05bc506f-ceb7-49b3-b914-e866a9326064}"; export PAPERCLIP_COMPANY="$COMPANY"
BK="$(security find-generic-password -s paperclip-board-api-key -a "$COMPANY" -w 2>/dev/null)" \
  || { echo "no board API key in keychain (service paperclip-board-api-key) — see SKILL.md § Authenticate" >&2; exit 1; }

method="${1:?usage: pc-api.sh <GET|POST|PATCH|DELETE> <path> [json-body]}"
path="${2:?path required, e.g. /api/adapters}"
body="${3:-}"
args=(-sS -X "$method" -H "Authorization: Bearer $BK" -H "Origin: $API")
[ -n "$body" ] && args+=(-H 'content-type: application/json' --data "$body")
curl "${args[@]}" "$API$path"
