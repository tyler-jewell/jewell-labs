#!/bin/sh
# Quick TDD eval: every known-failing item as a one-line derived template in
# quick/ (source template's system prompt, EVALS = just that item), plus one
# fast representative template per category. Iterate against this; run the
# full matrix once at the end.
#
# Derive/refresh a quick item from source template $SRC, failing case $i:
#   sys=$(tail -1 "$SRC" | jq -r '.__verbose.prompt
#     | split("<|im_start|>system\n")[1] | split("<|im_end|>")[0]')
#   ev=$(jq -r '.__verbose.prompt | split("<|im_start|>user\n")[1] // ""
#     | split("<|im_end|>")[0]' "$SRC" | grep '^EVALS ' | tail -1 \
#     | cut -c7- | jq -c "[.[$i]]")
#   rm -f quick/NAME.jsonl
#   ./agent call quick/NAME.jsonl "EVALS $ev" --system "$sys" -m qwen3-0.6b
cd "$(dirname "$0")" || exit 1
exec ./agent eval quick/*.jsonl template.history.jsonl \
  templates/format/yes-no.jsonl templates/extract/email-extract.jsonl \
  templates/classify/sentiment-json.jsonl templates/transform/pluralize.jsonl \
  "$@"
