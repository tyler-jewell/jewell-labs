#!/bin/sh -e
# Runs inside the container (see Dockerfile). Proves a fresh directory plus
# the `agent` script alone becomes a working system: bootstrap CONFIG into an
# empty core history, seed an agent from a read-only template, call it, eval.
[ -f /repo/agent ] || { echo "mount the repo read-only at /repo" >&2; exit 1; }
[ -f /model.gguf ] || { echo "mount a small chat GGUF at /model.gguf" >&2; exit 1; }
mkdir /work && cd /work && cp /repo/agent .

CFG='{"binary":"/app/llama-server","default_model":"probe-model","models":
  {"probe-model":{"path":"/model.gguf","ctx":4096,
    "params":{"temperature":0,"max_tokens":2048,"seed":42}}}}'
echo "== bootstrap CONFIG into an empty core history"
./agent call template.history.jsonl "CONFIG $(printf '%s' "$CFG" | jq -c .)" \
  --system "You are the core agent of a fresh history-agents checkout."

echo "== new + call"
./agent new probe /repo/templates/format/yes-no.jsonl
./agent call probe "Is water wet?"

echo "== eval"
out=$(./agent eval /repo/templates/format/yes-no.jsonl)
echo "$out"
echo "$out" | grep -q 'yes-no *4/4' || { echo "PORTABILITY TEST FAILED" >&2; exit 1; }
echo "PORTABILITY TEST PASSED"
