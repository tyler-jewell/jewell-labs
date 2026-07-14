# Eval catalog (online datasets only)

Modular registry of **public agentic benchmarks**. Task bodies are **not** stored in this repo.

Each source has:

```text
evals/catalog/sources/<source-id>/
  source.toml    # id, name, links, tags, license, enabled
  remote.toml    # remote_kind + URLs / official ids / file names only
```

At load time the catalog hydrates items from the network (with a local cache under `evals/catalog/.cache/`, gitignored). Hard-coded `items.jsonl` task bodies are **rejected**.

Product structural gates (`tool_plan` / team collaboration) stay in `rust-agent/src/eval/` and are **not** catalog items.

## Default sources

| id | Online origin | Links |
| --- | --- | --- |
| `bfcl` | Gorilla BFCL v4 JSONL (GitHub raw) | [leaderboard](https://gorilla.cs.berkeley.edu/leaderboard.html) |
| `terminal-bench` | Official `original-tasks/*/task.yaml` | [tbench.ai](https://www.tbench.ai/) |
| `swe-bench` | SWE-bench Lite instance ids → GitHub PR text | [swebench.com](https://www.swebench.com/) |

There is **no** `legacy-local` source and no synthetic in-repo task pack.

## Run

```bash
cd rust-agent
cargo run -q --bin eval_catalog -- --list-sources
cargo run -q --bin eval_catalog -- --list-vendors
cargo run -q --bin eval_catalog -- --sample-n 20 --seed 42
cargo run -q --bin eval_catalog -- --harnesses jewell --sample-n 5
cargo run -q --bin eval_catalog -- --tags tool_call --sample-n 5
```

Shared model pin: `evals/model.toml`. Vendors: `evals/vendors/*/vendor.toml`.

Sandbox-only items (full SWE-bench / Terminal-Bench Docker) are **excluded from sampling by default**. Pass `--include-sandbox` to keep them (they skip with a reason).

## Adding a source

1. Create `sources/<id>/source.toml` with public links + tags.
2. Create `remote.toml` with `remote_kind` and **pointers only** (dataset URL, official file names, instance ids). Do **not** commit prompts, gold answers, or workspace files.
3. Implement hydration in `rust-agent/src/eval/catalog/remote.rs` if adding a new `remote_kind`.
4. Set `enabled = true`.

## Grading honesty

Many public tasks need Docker/Harbor. Hydrated items mark `requires_sandbox` / `grade.kind = sandbox_skip` and are skipped with an explicit reason (not auto-passed).
