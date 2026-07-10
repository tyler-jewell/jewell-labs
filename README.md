# history-agents

Agents defined entirely by llama.cpp's own logs. An agent is a folder:

- `template.history.jsonl` — seed messages; the only thing an agent may change to improve itself
- `history.jsonl` — append-only log of every call it has ever made

Every line of both files is the raw verbose dump `llama-server -v` returns for
one call. Everything is re-extracted live from the union of those files on
every use: the system prompt (last message), the agent's evals (latest user
message `EVALS [...]`) and the entire configuration — model paths, server,
sampling params — (latest user message `CONFIG {...}`, an agent's merged over
the core agent's). There is nothing else: no tools, no skills, no subagents,
no config files.

## Setup

Needs `sh`, `curl`, `jq`, and [llama.cpp](https://github.com/ggml-org/llama.cpp)
(`brew install llama.cpp` / winget / apt; Windows: run under Git Bash).
Verified end-to-end on macOS (zsh/sh) and in a clean Ubuntu container (dash).

1. Copy `agent` and `template.history.jsonl` into your repo.
2. Register your machine's local GGUF models by making one self-configuring
   call (the call reads its own CONFIG message, so nothing needs to exist yet):

```sh
./agent call template.history.jsonl 'CONFIG {"default_model": "mini", "models":
  {"mini": {"path": "/path/to/model.gguf", "ctx": 8192,
            "params": {"temperature": 0, "max_tokens": 2048, "seed": 42}}}}'
```

All models are served by one llama-server router process with at most one
model in memory at a time; an idle reaper kills the server and its model
workers after `idle_ttl` seconds (default 600) without a call.

## Use

```sh
./agent new my-agent templates/format/yes-no.jsonl   # seed an agent from a template
./agent call my-agent "Is the sun a star?"           # talk to it (-m picks a model)
./agent eval templates/swe/locate-0.jsonl            # score its embedded evals
./agent eval template.history.jsonl templates/*/*.jsonl   # full matrix, all models in parallel
./agent improve templates/classify/language-detect.jsonl -m qwen3.6-35b
```

`improve` is the self-improvement loop: the agent is graded on its embedded
evals across all models, shown its own failures, and asked to rewrite its
system prompt and, optionally, its sampling params (a `CONFIG` line); the
rewrite is written into template.history.jsonl — its evals carried over
unchanged — only if the score strictly improves. An agent can never change
its history.jsonl, only the template that seeds it.

Create a brand-new agent from one prompt, using a local model itself:

```sh
./agent call new-agent/template.history.jsonl "EVALS []" --system "$(./agent call template.history.jsonl "Write a system prompt for an agent that reviews commit messages. Reply with the prompt only." -m qwen3.6-35b)"
```

`template.history.jsonl` in this repo root is the core agent: it designs other
agents and carries this machine's CONFIG. `templates/{category}/{name}.jsonl`
are portable one-call starter histories (prompt + evals, no machine facts);
`runs/` holds eval instantiations and may be deleted at any time.
