#!/usr/bin/env bash
# Stage built package onto a Paperclip host data volume and install via
# core local-path API (no npm registry).
#
# Run on the VPS host (root) OR adapt paths. Requires:
#   - Docker container paperclip-t8tg-paperclip-1 (or set PAPERCLIP_CONTAINER)
#   - Built package with dist/ (run npm run check on a build machine first)
#   - ADMIN_EMAIL / ADMIN_PASSWORD already in container env (Hostinger bootstrap)
#
# Does NOT print secrets. Board API key is stored only under:
#   /paperclip/instances/default/secrets/board-api-key-sendblue-ops
#
set -euo pipefail

PAPERCLIP_CONTAINER="${PAPERCLIP_CONTAINER:-paperclip-t8tg-paperclip-1}"
HOST_PLUGIN_DIR="${HOST_PLUGIN_DIR:-/docker/paperclip-t8tg/data/plugins/paperclip-plugin-sendblue}"
CONTAINER_PLUGIN_DIR="${CONTAINER_PLUGIN_DIR:-/paperclip/plugins/paperclip-plugin-sendblue}"
API_BASE="${API_BASE:-http://127.0.0.1:3100}"
# Must match PAPERCLIP_PUBLIC_URL origin trusted by better-auth
AUTH_ORIGIN="${AUTH_ORIGIN:-http://paperclip-t8tg.srv1829398.hstgr.cloud}"
PLUGIN_KEY="${PLUGIN_KEY:-jewell-labs.sendblue}"
SRC_DIR="${1:-}"

if [[ -n "$SRC_DIR" ]]; then
  echo "staging from $SRC_DIR -> $HOST_PLUGIN_DIR"
  mkdir -p "$HOST_PLUGIN_DIR"
  rsync -a --delete \
    --exclude node_modules \
    --exclude .git \
    "$SRC_DIR"/ "$HOST_PLUGIN_DIR"/
fi

docker exec -u root "$PAPERCLIP_CONTAINER" bash -c "
  set -euo pipefail
  chown -R node:node '$CONTAINER_PLUGIN_DIR'
  cd '$CONTAINER_PLUGIN_DIR'
  test -f dist/worker.js
  test -f dist/manifest.js
  test -f package.json
  # Hostinger image sets NODE_ENV=production; that skips installs unless deps are real dependencies
  export NODE_ENV=development
  npm install --omit=dev --no-fund --no-audit
  test -f node_modules/@paperclipai/plugin-sdk/package.json
"

docker exec "$PAPERCLIP_CONTAINER" bash -c '
set -euo pipefail
COOKIE=$(mktemp)
trap "rm -f $COOKIE" EXIT
ORIGIN="'"$AUTH_ORIGIN"'"
BASE="'"$API_BASE"'"
PLUGIN_PATH="'"$CONTAINER_PLUGIN_DIR"'"
KEYFILE=/paperclip/instances/default/secrets/board-api-key-sendblue-ops
mkdir -p "$(dirname "$KEYFILE")"

curl -sS -c "$COOKIE" -b "$COOKIE" \
  -H "Content-Type: application/json" \
  -H "Origin: $ORIGIN" \
  -X POST "$BASE/api/auth/sign-in/email" \
  --data "{\"email\":\"$ADMIN_EMAIL\",\"password\":\"$ADMIN_PASSWORD\"}" >/dev/null

KEY_JSON=$(curl -sS -b "$COOKIE" \
  -H "Content-Type: application/json" \
  -H "Origin: $ORIGIN" \
  -X POST "$BASE/api/board-api-keys" \
  --data "{\"name\":\"sendblue-ops-$(date +%s)\",\"expiresAt\":null}")
TOKEN=$(node -e "const d=JSON.parse(process.argv[1]); if(!d.token) process.exit(2); process.stdout.write(d.token)" "$KEY_JSON")
umask 077
printf "%s" "$TOKEN" > "$KEYFILE"
chmod 600 "$KEYFILE"

INSTALL=$(curl -sS -w "\n%{http_code}" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -H "Origin: $ORIGIN" \
  -X POST "$BASE/api/plugins/install" \
  --data "{\"packageName\":\"$PLUGIN_PATH\",\"isLocalPath\":true}")
CODE=$(echo "$INSTALL" | tail -n1)
BODY=$(echo "$INSTALL" | sed "\$d")
echo "install_http=$CODE"
if [[ "$CODE" != "201" && "$CODE" != "200" ]]; then
  # May already be installed — try enable
  echo "$BODY" | head -c 400
fi

# Enable (idempotent)
curl -sS -o /dev/null -w "enable_http=%{http_code}\n" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -H "Origin: $ORIGIN" \
  -X POST "$BASE/api/plugins/'"$PLUGIN_KEY"'/enable" || true

# Soft config (no SendBlue API secrets)
paperclipai plugin config:set "'"$PLUGIN_KEY"'" \
  --api-base "$BASE" --api-key "$TOKEN" \
  --payload-json "{\"configJson\":{\"allowlist\":[],\"emptyMeansDeny\":true,\"inboundMode\":\"log_only\",\"notifyOnIssueDone\":false,\"notifyOnIssueCreated\":false,\"notifyOnApprovalCreated\":false,\"notifyOnAgentError\":false}}" \
  --json >/dev/null

paperclipai plugin list --api-base "$BASE" --api-key "$TOKEN"
paperclipai plugin health "'"$PLUGIN_KEY"'" --api-base "$BASE" --api-key "$TOKEN"
'

echo "done. board key file (container): /paperclip/instances/default/secrets/board-api-key-sendblue-ops"
echo "plugin key: $PLUGIN_KEY"
