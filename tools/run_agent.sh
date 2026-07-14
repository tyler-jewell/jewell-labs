#!/usr/bin/env bash
# Minimal agent runner (bash+curl): agents/{name}.md → llama-server chat.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
AGENTS="$ROOT/agents"
REGISTRY="$ROOT/models/registry.yaml"

DRY_PARSE=0
SERVE_ONLY=0
MODEL_OVERRIDE=""
AGENT=""
PROMPT='What is 2+2? Put answer in \boxed{}.'

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-parse) DRY_PARSE=1; shift ;;
    --serve-only) SERVE_ONLY=1; shift ;;
    --model) MODEL_OVERRIDE="$2"; shift 2 ;;
    *)
      if [[ -z "$AGENT" ]]; then AGENT="$1"
      else PROMPT="$1"
      fi
      shift
      ;;
  esac
done

if [[ -z "$AGENT" ]]; then
  echo "usage: run_agent.sh <agent> [prompt] [--dry-parse] [--serve-only] [--model KEY]" >&2
  exit 1
fi

AGENT_PATH="$AGENTS/${AGENT}.md"
[[ -f "$AGENT_PATH" ]] || { echo "missing $AGENT_PATH" >&2; exit 1; }

# Extract YAML frontmatter body and fields with awk/sed (no pyyaml)
FM=$(awk 'BEGIN{p=0} /^---$/{p++; next} p==1{print} p>=2{exit}' "$AGENT_PATH")
BODY=$(awk 'BEGIN{p=0} /^---$/{p++; next} p>=2{print}' "$AGENT_PATH")

default_model=$(printf '%s\n' "$FM" | awk -F': *' '/^default_model:/{gsub(/["'\'']/,"",$2); print $2; exit}')
model_key="${MODEL_OVERRIDE:-$default_model}"
[[ -n "$model_key" ]] || { echo "missing default_model" >&2; exit 1; }

# Resolve from registry
model_path=$(awk -v k="$model_key" '
  $0 ~ "^  "k":" {found=1; next}
  found && /^    path:/ {gsub(/^    path:[[:space:]]*/,""); gsub(/["'\'']/ ,""); print; exit}
  found && /^  [A-Za-z0-9_.-]+:/ {exit}
' "$REGISTRY")
model_path="${model_path/#\~/$HOME}"
[[ -n "$model_path" && -f "$model_path" ]] || { echo "unknown model $model_key" >&2; exit 1; }

alias=$(awk -v k="$model_key" '
  $0 ~ "^  "k":" {found=1; next}
  found && /^    alias:/ {gsub(/^    alias:[[:space:]]*/,""); gsub(/["'\'']/ ,""); print; exit}
  found && /^  [A-Za-z0-9_.-]+:/ {exit}
' "$REGISTRY")
alias="${alias:-local}"

host=$(printf '%s\n' "$FM" | awk -F': *' '/^  host:/{gsub(/["'\'']/,"",$2); print $2; exit}')
port=$(printf '%s\n' "$FM" | awk -F': *' '/^  port:/{print $2; exit}')
ctx=$(printf '%s\n' "$FM" | awk -F': *' '/^  ctx:/{print $2; exit}')
reasoning=$(printf '%s\n' "$FM" | awk -F': *' '/^  reasoning:/{gsub(/["'\'']/,"",$2); print $2; exit}')
temp=$(printf '%s\n' "$FM" | awk -F': *' '/^  temperature:/{print $2; exit}')
max_tokens=$(printf '%s\n' "$FM" | awk -F': *' '/^  max_tokens:/{print $2; exit}')

host="${host:-127.0.0.1}"
port="${port:-8080}"
ctx="${ctx:-2048}"
reasoning="${reasoning:-off}"
temp="${temp:-0}"
max_tokens="${max_tokens:-256}"
base="http://${host}:${port}"

if [[ "$DRY_PARSE" -eq 1 ]]; then
  python3 -c "import json; print(json.dumps({'agent':'$AGENT','model_key':'$model_key','path':'$model_path','system_chars':len('''$BODY'''),'port':$port}))" 2>/dev/null \
    || echo "{\"agent\":\"$AGENT\",\"model_key\":\"$model_key\",\"path\":\"$model_path\",\"port\":$port}"
  exit 0
fi

if ! curl -sf "$base/v1/models" >/dev/null 2>&1; then
  llama-server -m "$model_path" --host "$host" --port "$port" -c "$ctx" -np 1 \
    --ctx-checkpoints 0 --reasoning "$reasoning" \
    > /tmp/run-agent-llama.log 2>&1 &
  for i in $(seq 1 60); do
    curl -sf "$base/v1/models" >/dev/null 2>&1 && break
    sleep 0.5
  done
  curl -sf "$base/v1/models" >/dev/null 2>&1 || { echo "server failed" >&2; exit 1; }
fi

if [[ "$SERVE_ONLY" -eq 1 ]]; then
  echo "$base"
  exit 0
fi

# Build JSON safely with python for escaping
payload=$(BODY="$BODY" PROMPT="$PROMPT" ALIAS="$alias" TEMP="$temp" MT="$max_tokens" python3 - <<'PY'
import json, os
print(json.dumps({
  "model": os.environ["ALIAS"],
  "messages": [
    {"role": "system", "content": os.environ["BODY"]},
    {"role": "user", "content": os.environ["PROMPT"]},
  ],
  "temperature": float(os.environ["TEMP"]),
  "max_tokens": int(os.environ["MT"]),
}))
PY
)

curl -s "$base/v1/chat/completions" \
  -H 'Content-Type: application/json' \
  -d "$payload" | python3 -c "import sys,json; m=json.load(sys.stdin)['choices'][0]['message']; print(m.get('content') or m.get('reasoning_content') or '')"
