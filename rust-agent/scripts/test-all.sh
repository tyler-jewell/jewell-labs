#!/usr/bin/env bash
# Full quality gate: line budget + cargo tests + wasm build + no product JS + core eval.
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== line budget (max 300 per source file) =="
over=0
while read -r count path; do
  [[ "$path" == "total" ]] && continue
  # skip generated wasm-bindgen glue
  [[ "$path" == *"/static/pkg/"* ]] && continue
  if [[ "${count// /}" -gt 300 ]]; then
    echo "OVER 300: $count $path"
    over=1
  fi
done < <(find src tools console_core console_wasm static tests -type f \( -name '*.rs' -o -name '*.js' -o -name '*.css' -o -name '*.mjs' \) ! -path 'static/pkg/*' -print0 2>/dev/null | xargs -0 wc -l)

if [[ "$over" -ne 0 ]]; then
  echo "FAIL: files exceed 300 lines"
  exit 1
fi
echo "OK: all source files ≤ 300 lines"

echo "== no hand-authored product JS =="
# Allow only static/pkg/* (wasm-bindgen generated)
bad=0
while IFS= read -r f; do
  case "$f" in
    static/pkg/*) ;;
    *)
      echo "FAIL product JS: $f"
      bad=1
      ;;
  esac
done < <(find static -type f \( -name '*.js' -o -name '*.mjs' \) 2>/dev/null | sort)
if [[ "$bad" -ne 0 ]]; then
  exit 1
fi
echo "OK: only generated static/pkg JS"

echo "== cargo test (workspace) =="
cargo test --workspace

echo "== wasm client build =="
./scripts/build-wasm.sh

echo "== core orchestrator tool-plan eval =="
cargo run -q --bin eval_introspection

echo "== all gates green =="
