# history-agents

Agents defined entirely by llama.cpp's own logs — installed, configured, and
improving themselves with no human in the loop, from one line:

```sh
curl -fsSL https://raw.githubusercontent.com/tyler-jewell/jewell-labs/main/harnesses/history-agents/agent -o agent && chmod +x agent && ./agent init
```

That single line, run in any directory, finds (or resumably downloads) a small
local model, boots one llama-server router, seeds a **core agent** and a
**main/ orchestrator agent**, arms healthchecks, and prints your next moves:

```sh
./agent serve     # chat page: every refresh re-extracts the live system prompt
./agent auto 3    # let main run the repo: measure, improve, create agents
```

## The idea

An agent is a folder:

- `template.history.jsonl` — seed messages; the only thing an agent may change to improve itself
- `history.jsonl` — append-only log of every call it has ever made

Every line of both files is the raw verbose dump `llama-server -v` returns for
one call. Everything is re-extracted live from the union of those files on
every use: the system prompt (last message), the agent's evals (latest user
message `EVALS [...]`), its healthchecks (latest `CHECKS [...]`) and the
entire configuration — model paths, server, sampling params — (latest user
message `CONFIG {...}`, an agent's deep-merged over the core agent's). There
is nothing else: no tools, no skills, no subagents, no config files. Needs
only `sh`, `curl`, `jq`, and [llama.cpp](https://github.com/ggml-org/llama.cpp).

Only the latest CONFIG message counts. The core agent's must therefore be
complete — to change one core setting, send the whole config again with that
setting changed. An individual agent's CONFIG is the opposite: a patch,
deep-merged over the core's, so it carries only its overrides (e.g. one
model's sampling params).

## Two installs, then it runs itself

**Install one — the line above.** `./agent init` is idempotent: it checks for
`curl`/`jq`/`llama-server` (telling you the one command to run if missing),
reuses any local `Qwen3-0.6B-Q8_0.gguf` or downloads it resumably, then seeds
core CONFIG + EVALS + CHECKS and the `main/` orchestrator — each step skipped
if already present.

**Install two — every session.** Before `auto` and `serve` sessions the agent
runs its own `CHECKS` (a latest-wins list in its history). All green: go. Any
red: it re-runs `init` to set itself up, and only hard-fails if still red.

**Autonomy.** `./agent auto N` gives `main` up to N actions. It receives a
STATE report (template list + recent outcomes recovered from its own history —
its memory *is* its history), and answers with one action: `EVAL <template>`,
`IMPROVE <template>`, `NEW <category>/<name>`, or `DONE`. The runner validates
targets against a strict whitelist, executes, and feeds the outcome back.
Nothing can be deleted; evals are pinned (agents never edit their own tests);
malformed actions are skipped and recorded; overlapping runs exit immediately
(`runs/auto.lock`), so it is safe to schedule:

```sh
( crontab -l 2>/dev/null; echo '0 3 * * * cd /path/to/repo && ./agent auto 5 >> runs/auto.log 2>&1' ) | crontab -
```

All models are served by one llama-server router process holding at most
`models_max` models in memory (CONFIG key, default 1 — set it to the number
of registered models if RAM allows to skip load/unload between rounds); an
idle reaper kills the server and its model workers after `idle_ttl` seconds
(default 600) without a call. A model's optional CONFIG `server {...}` object
passes extra llama-server settings (e.g. `parallel`, `flash-attn`) straight
into its preset entry.

## Commands

```sh
./agent init                                         # zero-input bootstrap (see above)
./agent new my-agent templates/format/yes-no.jsonl   # seed an agent from a template
./agent call my-agent "Is the sun a star?"           # talk to it (-m picks a model)
./agent eval templates/swe/locate-0.jsonl            # score its embedded evals
./agent eval template.history.jsonl templates/*/*.jsonl   # full matrix: agents in parallel, one model at a time
SEQUENTIAL=1 ./agent eval templates/*/*.jsonl        # strictly one call at a time: slower, bit-reproducible
./agent improve templates/classify/language-detect.jsonl -m qwen3.6-35b
./agent compact my-agent                             # fold its current state into a minimal 1–3 line template
./agent auto 5 -m qwen3.6-35b                        # orchestrator session, brain on a bigger model
./agent serve main 8080                              # chat page for an agent
```

`improve` is the self-improvement loop: the agent is graded on its embedded
evals across all models, shown its own failures, and asked to rewrite its
system prompt and, optionally, its sampling params (a `CONFIG` line); the
rewrite is written into template.history.jsonl — its evals carried over
unchanged — only if the score strictly improves. An agent can never change
its history.jsonl, only the template that seeds it.

`serve` is the smallest possible frontend: a pure sh + netcat page. Every
refresh re-extracts the agent's *current* system prompt from its history — if
`improve` or `auto` rewrote the agent overnight, the page shows it — and
every chat message goes through `agent call`, so it lands in the agent's
history like any other call.

## Scaling

Measured with `./scaling-test.sh` (one real dump line replicated N times as an
agent's history; Apple Silicon, Qwen3-0.6B):

| messages in history | bytes | extraction ms | end-to-end call ms | prompt tokens |
|---:|---:|---:|---:|---:|
| 1 | 3.5 KB | 10 | 1028 | 26 |
| 10 | 35 KB | 12 | 846 | 26 |
| 100 | 349 KB | 22 | 1020 | 26 |
| 1000 | 3.5 MB | 112 | 975 | 26 |
| 5000 | 17.4 MB | 512 | 1404 | 26 |

Prompt tokens are **flat**: a call sends only `[system, user]`; history never
enters the context, so model cost is O(1) in history length. Extraction (the
jq scan behind every latest-wins read) is linear in file bytes — ~30 ms/MB,
irrelevant below a thousand messages and ~0.5 s at five thousand. When a
history gets heavy, `./agent compact` folds the agent's current state back to
a 1–3 line template.

## Evals, quick loop, benchmarks

Each agent carries its own eval cases in its history (`EVALS [...]`, graded
by regex). The committed benchmark set spans extract / classify / format /
transform / swe (SWE-bench-Verified file and file::function localization) /
reason (GSM8K). Full three-model matrix, byte-identical across two
consecutive fresh runs with `SEQUENTIAL=1`:

| | qwen3-0.6b | qwen3.6-35b | qwen3-4b |
|---|---|---|---|
| 115 cases | 105 | 113 | 104 |

For iteration there is `./quick-eval.sh` (~2 min for all three models, ~10 s
single-model): every known-failing case lives as a one-line derived template
in `quick/`, plus one fast representative per category — a TDD loop where
known failures stay red until genuinely fixed. Sampled decoding on a shared
GPU is workload-sensitive at the margins; benchmark claims should always use
`SEQUENTIAL=1`.

## Starter packs

A starter pack IS a compacted template file — one to three lines of raw
llama-server dumps carrying an agent's system prompt, evals, and optional
CONFIG/CHECKS. Export by compacting, share by copying the file:

```sh
./agent compact my-agent                        # fold current state into the seed
cp my-agent/template.history.jsonl commit-reviewer.jsonl   # the pack is this file
```

Import on any machine that has run `./agent init`:

```sh
./agent new commit-reviewer commit-reviewer.jsonl
./agent eval commit-reviewer.jsonl              # trust, then verify
```

Machine facts (model paths, ports) live in the importer's core history, never
in the pack, so packs move across machines and models unchanged. Explicit
non-goal: there is no registry, no manifest, no marketplace — a pack is a
file.

## Portability

`Dockerfile` + `portability-test.sh` prove the whole system from a clean
checkout inside the llama.cpp `:full` image: bootstrap CONFIG into an empty
core history → seed from a read-only template → call → eval. Verified on
macOS (zsh/sh) and Ubuntu (dash).

## Layout

`template.history.jsonl` at this directory's root is the core agent: it
designs other agents and carries this machine's CONFIG and CHECKS. `main/` is
the orchestrator `auto` drives. `templates/{category}/{name}.jsonl` are
portable one-call starter histories (prompt + evals, no machine facts);
`quick/` holds the known-failure TDD canaries; `runs/`, `models/` and any
`history.jsonl` are machine-local and gitignored. `GOAL.md` is the campaign
log of how this system was built and verified.

MIT licensed. No Python, no build, no dependencies beyond sh + curl + jq +
llama.cpp.
