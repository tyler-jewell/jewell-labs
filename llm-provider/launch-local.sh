#!/usr/bin/env bash
# Start a llama.cpp server from a tuned profile in config.toml ([[local_runtime.profiles]]).
# The config is the single source of truth for the flags — no more hand-typed launch lines.
#
#   launch-local.sh [profile-name]     # default: the first profile
set -euo pipefail

DIR="$(cd "$(dirname "$0")" && pwd)"
CFG="${LLM_PROVIDER_DIR:-$DIR}/config.toml"
PROFILE="${1:-}"

# Parse the profile ONCE. Emit model, then port, then one flag per line, so `read` keeps
# paths/values with spaces intact (line-delimited, not word-split). python3 3.11+ has tomllib.
MODEL="" ; PORT="" ; FLAGS=()
{
  read -r MODEL
  read -r PORT
  while IFS= read -r line; do FLAGS+=("$line"); done
} < <(python3 - "$CFG" "$PROFILE" <<'PY'
import sys, tomllib
cfg = tomllib.load(open(sys.argv[1], "rb"))
want = sys.argv[2]
profs = cfg.get("local_runtime", {}).get("profiles", [])
if not profs:
    sys.exit("no [[local_runtime.profiles]] in config.toml")
p = next((x for x in profs if x.get("name") == want), profs[0])
print(p["model"])
print(p["port"])
for f in p.get("flags", []):
    print(f)
PY
)

echo "launching llama-server: model=$(basename "$MODEL") port=$PORT flags=${FLAGS[*]}"
exec llama-server -m "$MODEL" --host 127.0.0.1 --port "$PORT" "${FLAGS[@]}"
