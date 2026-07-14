# Multi-vendor eval fairness (poka-yoke)

This document is the policy for catalog multi-vendor runs (`eval_catalog`).

## Research basis (summary)

| Source | Takeaway |
| --- | --- |
| **Terminal-Bench / Harbor** | Task (instruction + env + tests) is agent-agnostic; agent adapters are thin invokers. Do not rewrite the task per agent. |
| **Harness-Bench (arXiv 2605.27922)** | Scores measure model × harness. Make harness **config** explicit and comparable; do not hide coaching in adapters. |
| **BFCL** | Tools are provided as structured schemas; scoring is on emitted calls — not on free-form coach text unique to one scaffold. |
| **SWE-bench agent literature** | Scaffold / harness effects dominate if unchecked; fix harness config, not the problem statement, when comparing products. |

## Allowed surfaces

| Surface | SSoT | Purpose |
| --- | --- | --- |
| **Task prompt + tools + workspace files** | `evals/catalog/sources/<src>/items.jsonl` | Identical bytes to every vendor |
| **Gold / grade kind** | same item `grade` object | Post-hoc only; never pasted into the user message by adapters |
| **Shared model pin** | `evals/model.toml` | Same weights / endpoint for all vendors |
| **Vendor harness config** | `evals/vendors/<id>/vendor.toml` (`config_ssot`) | **Only** place to change toolsets, max-turns, bin, cwd, home, temperature pin, agent id |

## Forbidden (cheating)

1. **Per-vendor coach wrappers** in Rust (`format!("You are under evaluation… Critical: …")`).
2. **Gold leakage** into prompts (e.g. “write ERR-3 not 3”, “keep b,80 and c,50”) unless that text is part of a **shared** catalog item and accepted as dataset design.
3. **Grade-kind injection** in the runner that teaches the answer format differently from the catalog item text.
4. **Asymmetric sampling** hard-coded in one adapter only — declare `eval_temperature` (or CLI sampling) in `vendor.toml` if used.
5. Inventing per-fictional-tool plugins for one vendor solely to pass BFCL JSON format items (use track-level toolset config instead).

## What may live in `vendor.toml`

- Binary path, CLI argv, `{prompt}` / `{workspace}` mechanical templates  
- Track-specific **tool surface** (`args_by_track`, e.g. Hermes `-t none` for format-only BFCL vs `-t terminal,file` for terminal)  
- Home template / model pin env keys  
- Jewell agent stem, product base URL, optional `eval_temperature`  
- Declared capabilities (honest skip gate)

## Product vs harness

| Change | Where |
| --- | --- |
| Real agent capability (e.g. agent sandbox `fs_*`) | Jewell product (`rust-agent/tools`, agent markdown) |
| How we **invoke** Hermes/Jewell for a track | `evals/vendors/*/vendor.toml` only |
| What the task says | Catalog items only |

## Enforcement

- `build_instruction` returns prompt + tool schemas only (unit-tested: no coach phrases).  
- CLI/native adapters pass `instruction` through (optional mechanical `prompt_template`).  
- Manifest load rejects coach phrases in `prompt_template` and requires `config_ssot`.  
- Catalog meta records `fairness.no_vendor_prompt_wrappers = true`.

## Honest scores after this policy

Scores may drop vs coach-inflated runs. That is expected. Report failures as:

1. **Product gap** (fix with real tools / agent behavior), or  
2. **Harness config gap** (fix `vendor.toml` toolsets/timeouts/cwd only), or  
3. **Dataset gap** (clarify shared item prompt without gold spoiler),  

never as “add a per-vendor coach sentence.”
