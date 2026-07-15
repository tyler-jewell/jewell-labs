# Eval vendors (harness config SSoT)

Drop-in multi-vendor adapters for catalog runs. **Same model** for every vendor: [`../model.toml`](../model.toml).

**Fairness policy:** [`../EVAL_FAIRNESS.md`](../EVAL_FAIRNESS.md).

## Rule of three SSoTs

| What | File |
| --- | --- |
| Task (prompt, tools, workspace, gold) | Online via `evals/catalog/sources/<src>/remote.toml` (hydrated at load) |
| Model pin | `evals/model.toml` |
| **Harness config (only knobs for a vendor)** | `evals/vendors/<id>/vendor.toml` |

Adapters must **not** wrap the task with coach text. The only legal text sent to the agent is the shared catalog instruction (plus optional mechanical `{prompt}` / `{workspace}` substitution declared in `vendor.toml`).

## Add a CLI vendor (no Rust changes)

1. Create `evals/vendors/<id>/vendor.toml`:

```toml
id = "openclaw"
name = "OpenClaw"
enabled = true
kind = "cli"
config_ssot = "evals/vendors/openclaw/vendor.toml"
capabilities = ["terminal", "write_file", "coding", "closed_form"]

[cli]
bin_candidates = ["openclaw", "evals/vendors/openclaw/bin/openclaw"]
prompt_template = "{prompt}"   # mechanical only — no coach sentences
args = ["run", "--prompt", "{prompt}"]
cwd = "workspace"
env_model_base_url = "OPENAI_BASE_URL"
env_model_name = "OPENAI_MODEL"
```

2. Ensure the binary is on `PATH` or under a candidate path.
3. Next `eval_catalog` run discovers it automatically (if `enabled = true`).

### Track-specific harness config

Optional `[cli.args_by_track]` switches **tool surface / CLI flags** by catalog track (e.g. empty toolset for BFCL format vs terminal tools for file tasks). That is allowed; rewriting the user message per track in Rust is not.

## Native Jewell

`kind = "native_jewell"` — product HTTP chat. Config SSoT:

```toml
config_ssot = "evals/vendors/jewell/vendor.toml"

[native_jewell]
agent = "core/orchestrator"
base_url = "http://127.0.0.1:3000"
eval_temperature = 0.0   # optional pin; declare here, not in adapter prose
```

Product capabilities (e.g. `fs_*` sandbox tools) belong in the **Jewell app**, not in eval coach prompts.

### Eval pins (security)

Catalog jewell sends `eval_base_url` / `eval_model` / `eval_fs_root` so the product agent uses the shared model pin and writes into the item workspace. Server:

```bash
JEWELL_ALLOW_EVAL_PINS=1 cargo run --bin rust-agent
```

## Defaults

- Enabled vendors are the default harness list (sorted by id).
- Filter with `--harnesses jewell` (or a comma list of **enabled** vendors).
- Disable a vendor with `enabled = false` without deleting it.
- **Hermes is not used** — keep `evals/vendors/hermes` disabled; do not re-enable or expand it.
