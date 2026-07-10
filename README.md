# history-agents

Agents defined entirely by llama.cpp's own logs. An agent is a folder:

- `template.history.jsonl` — seed messages; the only thing an agent may change to improve itself
- `history.jsonl` — append-only log of every call it has ever made

Every line of both files is the raw verbose dump `llama-server -v` returns for
one call. The agent's system prompt and its evals (the latest user message
starting with `EVALS`) are extracted live from the union of those files on
every use. There is nothing else: no tools, no skills, no subagents.

## Setup

Needs `sh`, `curl`, `jq`, and [llama.cpp](https://github.com/ggml-org/llama.cpp)
(`brew install llama.cpp` / winget / apt; Windows: run under Git Bash).

1. Copy `agent` and `config.yaml` into your repo.
2. Edit `config.yaml`: one entry per local GGUF model (path, port, ctx, params).
   The file is the system's only tuning surface.

## Use

```sh
./agent new my-agent templates/format/yes-no.jsonl   # seed an agent from a template
./agent call my-agent "Is the sun a star?"           # talk to it (-m picks a model)
./agent eval templates/swe/locate-0.jsonl            # score its embedded evals
./agent eval template.history.jsonl templates/*/*.jsonl   # full matrix, all models in parallel
```

Create a brand-new agent from one prompt, using a local model itself:

```sh
./agent call new-agent/template.history.jsonl "EVALS []" --system "$(./agent call template.history.jsonl "Write a system prompt for an agent that reviews commit messages. Reply with the prompt only." -m qwen3.6-35b)"
```

`template.history.jsonl` in this repo root is the core agent: it designs other
agents. `templates/{category}/{name}.jsonl` are one-line starter histories with
their evals embedded; `runs/` holds eval instantiations and may be deleted at
any time.
