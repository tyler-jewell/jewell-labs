#!/bin/sh
# Scaling DoE: how the system behaves as an agent's history (template ∪ log)
# grows. Builds throwaway agents in runs/ whose history is one real dump line
# replicated N times, then measures per N: bytes on disk, extraction latency
# (the user-message scan behind latest CONFIG/EVALS/CHECKS + the last-line
# system-prompt read that every call performs), end-to-end call latency on the
# default model, and prompt tokens per call. Prompt tokens should stay flat:
# a call sends only [system, user], never the history. Results go in README.
cd "$(dirname "$0")" || exit 1
UMSG='.__verbose.prompt | split("<|im_start|>user\n")[1] // "" | split("<|im_end|>")[0]'
ms() { jq -n 'now*1000|floor'; }

rm -f runs/scale-seed.jsonl
./agent call runs/scale-seed.jsonl "Reply with the word ok." \
  --system "You reply with the word ok." >/dev/null || exit 1
line=$(tail -1 runs/scale-seed.jsonl)

printf '%-10s %-12s %-12s %-10s %-14s\n' messages bytes extract-ms call-ms prompt-tokens
for n in 1 10 100 1000 5000; do
  d=runs/scale-$n; rm -rf "$d"; mkdir -p "$d"
  printf '%s\n' "$line" > "$d/template.history.jsonl"
  i=1; while [ "$i" -lt "$n" ]; do printf '%s\n' "$line"; i=$((i+1)); done > "$d/history.jsonl"
  b=$(cat "$d/template.history.jsonl" "$d/history.jsonl" | wc -c | tr -d ' ')
  t0=$(ms)
  cat "$d/template.history.jsonl" "$d/history.jsonl" | jq -r "$UMSG" >/dev/null
  cat "$d/template.history.jsonl" "$d/history.jsonl" | tail -n 1 \
    | jq -r '.__verbose.prompt' >/dev/null
  t1=$(ms)
  ./agent call "$d" "Reply with the word ok." >/dev/null || exit 1
  t2=$(ms)
  pt=$(tail -1 "$d/history.jsonl" | jq -r '.usage.prompt_tokens')
  printf '%-10s %-12s %-12s %-10s %-14s\n' "$n" "$b" "$((t1 - t0))" "$((t2 - t1))" "$pt"
done
