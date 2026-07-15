#!/usr/bin/env bash
# Design-of-Experiments sweep for llama.cpp on this Mac. Uses llama-batched-bench (the
# canonical throughput/concurrency tool) to measure tokens/s across parallel loads for a
# grid of tuning flags, and writes one CSV row per (config, parallel-load) point.
#
# It does NOT touch the running llama-server instances — batched-bench loads its own model
# copy. It DOES consume GPU + unified memory transiently, so run it when the box is idle.
#
# Usage:
#   tuning/doe.sh /path/to/model.gguf [ctx] [prompt_tokens] [gen_tokens]
# Env overrides (space-separated lists):
#   UBATCH="512 1024 2048"  BATCH="2048"  KV="none q8_0"  NPL="1 2 4 8 16"
#
# Read the tune-local-llm skill for what each lever does and how to pick the winner.
set -euo pipefail

MODEL="${1:?usage: doe.sh <model.gguf> [ctx] [ppTokens] [genTokens]}"
CTX="${2:-32768}"
PP="${3:-512}"
TG="${4:-128}"

UBATCH="${UBATCH:-512 1024 2048}"
BATCH="${BATCH:-2048}"
KV="${KV:-none q8_0}"          # KV cache quant: none (f16) or q8_0
NPL="${NPL:-1 2 4 8 16}"       # parallel-request loads to sweep
NGL="${NGL:-99}"

DIR="$(cd "$(dirname "$0")" && pwd)"
STAMP="$(date +%Y%m%d-%H%M%S)"
OUT="$DIR/results/doe-$STAMP.csv"
LOG="$DIR/results/doe-$STAMP.log"
mkdir -p "$DIR/results"

echo "model,ctx,batch,ubatch,kv,fa,npl,pp_tok_s,tg_tok_s,total_tok_s" > "$OUT"
echo "DoE -> $OUT  (raw: $LOG)"
echo "model=$(basename "$MODEL") ctx=$CTX pp=$PP tg=$TG" | tee -a "$LOG"

for b in $BATCH; do
  for ub in $UBATCH; do
    [ "$ub" -gt "$b" ] && continue          # ubatch must not exceed batch
    for kv in $KV; do
      kvargs=(); [ "$kv" != "none" ] && kvargs=(-ctk "$kv" -ctv "$kv")
      npl_csv="$(echo "$NPL" | tr ' ' ',')"
      echo ">>> b=$b ub=$ub kv=$kv fa=on npl=$npl_csv" | tee -a "$LOG"
      # one invocation emits one row per npl value ("${kvargs[@]+...}" is empty-array-safe under set -u)
      out="$(llama-batched-bench -m "$MODEL" -c "$CTX" -b "$b" -ub "$ub" -fa on \
              -ngl "$NGL" ${kvargs[@]+"${kvargs[@]}"} -npp "$PP" -ntg "$TG" -npl "$npl_csv" 2>&1 | tee -a "$LOG")" || {
        echo "  (run failed — likely OOM at this config; recording nothing)" | tee -a "$LOG"; continue; }
      # parse the batched-bench table: numeric rows begin with "|" and have S_PP, S_TG, S t/s.
      rows="$(echo "$out" | awk -v m="$(basename "$MODEL")" -v ctx="$CTX" -v b="$b" -v ub="$ub" -v kv="$kv" '
        /^\|/ {
          gsub(/\|/," ");
          # columns: PP TG B N_KV T_PP S_PP T_TG S_TG T S
          if ($1 ~ /^[0-9]+$/ && NF>=10) {
            printf "%s,%s,%s,%s,%s,on,%s,%s,%s,%s\n", m,ctx,b,ub,kv,$3,$6,$8,$10
          }
        }')"
      if [ -z "$rows" ]; then
        echo "  WARN: no data rows parsed for b=$b ub=$ub kv=$kv — llama-batched-bench output format may have changed; inspect $LOG" | tee -a "$LOG"
      else
        echo "$rows" >> "$OUT"
      fi
    done
  done
done

echo "done. Top configs by total tok/s:"
# skip header, sort by last column desc, show top 8
tail -n +2 "$OUT" | sort -t, -k10 -nr | head -8 | column -t -s,
