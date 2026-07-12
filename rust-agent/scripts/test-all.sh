#!/usr/bin/env bash
# Full quality gate: line budget + cargo tests + JS SSE + core orchestrator eval (tool_plan).
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== line budget (max 300 per source file) =="
over=0
while read -r count path; do
  [[ "$path" == "total" ]] && continue
  if [[ "${count// /}" -gt 300 ]]; then
    echo "OVER 300: $count $path"
    over=1
  fi
done < <(find src tools static tests -type f \( -name '*.rs' -o -name '*.js' -o -name '*.css' -o -name '*.mjs' \) -print0 | xargs -0 wc -l)

if [[ "$over" -ne 0 ]]; then
  echo "FAIL: files exceed 300 lines"
  exit 1
fi
echo "OK: all source files ≤ 300 lines"

echo "== cargo test =="
cargo test

echo "== frontend SSE unit tests =="
node --test tests/js/sse.test.mjs

echo "== core orchestrator tool-plan eval =="
cargo run -q --bin eval_introspection

echo "== all gates green =="
