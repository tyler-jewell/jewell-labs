# history-agents

An agent defined entirely by one version-controlled file, installed,
configured, and improving itself with no human in the loop, from one line:

```sh
curl -fsSL https://raw.githubusercontent.com/tyler-jewell/jewell-labs/main/harnesses/history-agents/agent -o agent && \
curl -fsSL https://raw.githubusercontent.com/tyler-jewell/jewell-labs/main/harnesses/history-agents/history.jsonl -o history.jsonl && \
chmod +x agent && ./agent init
```

That line, run in any empty directory, downloads two files (the runner and
the seed), finds or resumably downloads a small local model, boots a
llama-server router, resolves this machine's CONFIG and CHECKS into
`history.jsonl`, and finishes by **running the agent's own evals as its
last install step** — you see, from the very first run, exactly what it can
already do:

```sh
./agent serve     # chat page: every refresh re-extracts the live system prompt
./agent auto 3    # let it run the repo: measure, improve, create new agents
```

## The idea

An agent is a folder holding one file: `history.jsonl`, the append-only log
of every raw response `llama-server -v` has ever returned for a call to it.
There is nothing else — no config files, no tools, no skills, no subagents.
Everything is re-extracted live from that file on every use:

- **system prompt** — the last call's system message
- **EVALS** — the latest user message starting `EVALS [...]`, its pinned test cases
- **CHECKS** — the latest user message starting `CHECKS [...]`, its healthchecks
- **CONFIG** — the latest user message starting `CONFIG {...}` — model paths,
  server and sampling params. An agent's CONFIG is a patch, deep-merged over
  the root agent's (this directory's own `history.jsonl`); the root's own
  CONFIG must therefore be complete, since only the latest one counts.

**A capability is what an EVALS case proves.** The system prompt says so
directly: to know what this agent can do, read its EVALS. When it improves
itself and gains a capability, the rewrite must add an EVALS case that pins
it in the same step — EVALS only grow, never shrink. This is also what keeps
`history.jsonl` itself small: nothing is asserted that isn't tested.

Every stored line keeps only the two fields the runner ever reads back —
`choices[0].message.content` (for humans skimming the log) and
`__verbose.prompt` (the system+user text extraction depends on) — not the
full llama-server dump (timings, sampler settings, token ids, reasoning
trace). That's the whole "least fields needed" rule, applied uniformly by
`agent call` to every line it appends, not just the shipped seed.

## One seed, then it runs itself

The **only version-controlled files are `agent` and `history.jsonl`.**
`history.jsonl` as shipped is a single line: one EVALS call carrying the
system prompt above. It intentionally carries **no CONFIG** — model paths
are machine-specific and can't be portable, so `agent init` always resolves
them fresh, appending a CONFIG line (and a CHECKS line) once it has found a
binary and a model. Those two extra lines are **local, not shipped** — after
`init`, `git diff history.jsonl` on your machine will show them appended;
that's expected. Nothing forces you to commit that growth, and nothing stops
you either — `history.jsonl` is exactly as append-only as it always was, it
just now travels in git instead of being gitignored.

**Every session.** Before `auto` and `serve` sessions the agent runs its own
`CHECKS` (a latest-wins list in its history). All green: go. Any red: it
re-runs `init` to set itself up, and only hard-fails if still red.

**Autonomy.** `./agent auto N` gives the agent up to N actions. It receives a
STATE report (list of any agents it has created + recent outcomes, recovered
from its own history — its memory *is* its history), and answers with one
action: `EVAL <target>`, `IMPROVE <target>`, `NEW <category>/<name>`, or
`DONE`. `self` refers to this repo's own `history.jsonl`; other targets are
`templates/<category>/<name>.jsonl` files the agent creates at runtime (not
shipped, not curated — gitignored, since they're this install's own output).
The runner validates targets against a strict whitelist, executes, and feeds
the outcome back. Nothing can be deleted; EVALS are pinned (agents never
edit their own tests); malformed actions are skipped and recorded;
overlapping runs exit immediately (`runs/auto.lock`), so it is safe to
schedule:

```sh
( crontab -l 2>/dev/null; echo '0 3 * * * cd /path/to/repo && ./agent auto 5 >> runs/auto.log 2>&1' ) | crontab -
```

All models are served by one llama-server router process holding at most
`models_max` models in memory (CONFIG key, default 1); an idle reaper kills
the server and its model workers after `idle_ttl` seconds (default 600)
without a call. A model's optional CONFIG `server {...}` object passes
extra llama-server settings straight into its preset entry.

## Commands

```sh
./agent init                                          # zero-input bootstrap (see above)
./agent new my-agent seed.jsonl                       # seed an agent from a file
./agent call my-agent "Is the sun a star?"             # talk to it (-m picks a model)
./agent eval my-agent                                  # score its embedded evals
./agent eval history.jsonl templates/*/*.jsonl         # matrix across agents, one model at a time
SEQUENTIAL=1 ./agent eval templates/*/*.jsonl           # strictly one call at a time: slower, bit-reproducible
./agent improve my-agent -m qwen3-0.6b
./agent compact my-agent                                # fold its current state into a minimal 1-3 line seed
./agent auto 5 -m qwen3-0.6b                            # let it run the repo
./agent serve my-agent 8080                             # chat page for an agent
```

`improve` is the self-improvement loop: the agent is graded on its embedded
evals across all models, shown its own failures, and asked to rewrite its
system prompt and, optionally, its sampling params (a `CONFIG` line); the
rewrite is written into `history.jsonl` in place — its EVALS carried over
unchanged — only if the score strictly improves. An agent can never rewrite
itself mid-call, only through `./agent improve`.

`serve` is the smallest possible frontend: a pure sh + netcat page. Every
refresh re-extracts the agent's *current* system prompt from its history —
if `improve` or `auto` rewrote it overnight, the page shows it — and every
chat message goes through `agent call`, so it lands in the agent's history
like any other call.

## Starter packs

A starter pack IS a seed file — one to a few lines of raw llama-server dumps
carrying an agent's system prompt, EVALS, and optional CONFIG/CHECKS. Export
by compacting, share by copying the file:

```sh
./agent compact my-agent                                    # fold current state into the seed
cp my-agent/history.jsonl commit-reviewer.jsonl              # the pack is this file
```

Import on any machine that has run `./agent init`:

```sh
./agent new commit-reviewer commit-reviewer.jsonl
./agent eval commit-reviewer.jsonl                            # trust, then verify
```

Machine facts (model paths, ports) live in the importer's root CONFIG, never
in the pack, so packs move across machines and models unchanged. Explicit
non-goal: there is no registry, no manifest, no marketplace — a pack is a
file. Containerized/portable-runtime packs (e.g. a llama.cpp Docker image)
are a natural future pack, not part of the core today.

## Layout

`agent` and `history.jsonl` are the entire version-controlled system: the
runner and the one seed message that defines it. `templates/` (agents this
install has created via `NEW`), `runs/`, and `models/` are machine-local and
gitignored — nothing there is curated or shipped; a fresh install starts
with none of it and builds what it needs.

The system was previously built up with a larger, curated tree (per-category
starter templates, a TDD quick-eval loop, a scaling DoE, a Docker portability
proof, a campaign log) — all still in this repo's git history if useful, just
no longer part of what a fresh install carries, per the "one seed file"
simplification.

MIT licensed. No Python, no build, no dependencies beyond sh + curl + jq +
[llama.cpp](https://github.com/ggml-org/llama.cpp).
