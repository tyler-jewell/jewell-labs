# Eval catalog (multi-source)

Modular registry of **public-style** agentic benchmark sources. Product structural gates (`tool_plan` / team collaboration) stay in `rust-agent/src/eval/` and are **not** catalog items.

## Layout

```text
evals/catalog/
  sources/
    <source-id>/
      source.toml      # id, name, links, tags, license, version, enabled
      items.jsonl      # one CatalogItem JSON per line
```

## Default sources

| id | Focus | Links |
| --- | --- | --- |
| `swe-bench` | Real issue→patch coding / agent autonomy | [swebench.com](https://www.swebench.com/) |
| `terminal-bench` | Multi-step CLI/terminal autonomy | [tbench.ai](https://www.tbench.ai/) |
| `bfcl` | Tool/function calling for assistants | [BFCL leaderboard](https://gorilla.cs.berkeley.edu/leaderboard.html) |
| `legacy-local` | Old harness_compare smokes (**disabled**) | — |

## Run

Default harnesses = **all enabled vendors** under `evals/vendors/*/vendor.toml` (currently jewell + hermes). Same seeded sample for each. Shared LLM pin: `evals/model.toml`.

```bash
cd rust-agent
cargo run -q --bin eval_catalog -- --list-sources
cargo run -q --bin eval_catalog -- --list-vendors
cargo run -q --bin eval_catalog -- --sample-n 20 --seed 42
cargo run -q --bin eval_catalog -- --harnesses jewell --sample-n 5
cargo run -q --bin eval_catalog -- --tags tool_call --sample-n 5
```

UI: **Evals → Catalog sample**.

Sandbox-only items (full SWE-bench / Terminal-Bench Docker tasks) are **excluded from sampling by default**. Pass `--include-sandbox` to keep them (they skip with a reason).

## Vendors

See [`../vendors/README.md`](../vendors/README.md). Adding a CLI agent is one TOML file; the next catalog run picks it up automatically.

## Tool calling (BFCL)

BFCL-style items include a `tools` array (JSON schemas). The catalog runner injects those schemas into the instruction at runtime. Grading checks for a parseable `{"name","arguments"}` tool call — not free-form prose or product-tool names.

## Adding a source

1. Create `sources/<id>/source.toml` with links + tags.
2. Add `items.jsonl` rows matching the `CatalogItem` schema (see `rust-agent/src/eval/catalog/types.rs`).
3. Set `enabled = true`.

## Grading honesty

Many public tasks need Docker/Harbor. Items mark `requires_sandbox` / `grade.kind = sandbox_skip` and are skipped with an explicit reason (not auto-passed).