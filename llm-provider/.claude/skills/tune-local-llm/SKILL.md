---
name: tune-local-llm
description: >-
  Tune the local llama.cpp runtime behind llm-provider for max GPU utilization, max
  tokens/s, and max concurrent models on this Mac (Apple M1 Max, 32 GB). Use when changing
  llama-server flags, editing config.toml [[local_runtime.profiles]], running a
  Design-of-Experiments sweep, or diagnosing slow/low-concurrency local inference. Explains
  every lever, when to pull it, the memory math, and how to run tuning/doe.sh and pick a winner.
---

# Tune the local llama.cpp runtime

The local models behind `llm-provider` are served by `llama-server`. Its command-line flags
decide throughput and how many requests/models fit at once. This skill is the full playbook.

## Hardware envelope (this machine)

Apple **M1 Max, 32 GB unified memory, 24 GPU cores, Metal 4**. The binding constraint is
**memory, not compute** — `vm_stat` shows the compressor already holding several GB under
load. "Max concurrent models" is a memory-budget question; pushing past it spills to
compressor/swap and *tanks* tokens/s. Always watch memory during a sweep.

## The levers (what each does, when to pull it)

Always on — no reason to leave off:
- `-fa on` — Flash Attention. No downside on Metal, and it is **required** for KV-cache
  quantization (without it, llama.cpp dequantizes the KV cache every step, which is slower).
- `-cb` — continuous (dynamic) batching. Merges decode steps from different requests into
  one forward pass. This is where concurrency throughput comes from.

Prefill (prompt-processing) throughput:
- `-ub` (ubatch, physical micro-batch) — **the primary prefill lever on Apple Silicon.**
  Each micro-batch is one Metal kernel dispatch; bigger amortizes launch overhead. Try
  512 → 1024 → 2048. Must divide `-b`.
- `-b` (batch, logical) — prompt tokens per forward pass. Widen to 4096/8192 only when
  `-np` is high and you are prefill-bound; otherwise 2048 is fine.

Concurrency:
- `-np N` — parallel slots = how many requests run at once. More slots = more aggregate
  throughput **only if** prefill has enough tokens/pass to feed them. Raising `-np` alone
  just spreads the same throughput thinner (latency ↑, aggregate flat). Find the knee.

Memory (the 32 GB constraint):
- `-ctk q8_0 -ctv q8_0` — quantize the KV cache (needs `-fa`). Roughly halves KV memory,
  letting you raise `-np`/`-c` or fit a second model. Small quality cost.
- `--cache-reuse 256` — reuse KV for byte-identical prompt prefixes. Big win for agent
  loops with a stable system prompt (verify via `timings.cache_n` in responses). Needs a
  prompt with no timestamps/UUIDs.
- `-ncmoe N` / `-cmoe` — offload MoE expert layers to CPU. The lever for the
  **Qwen3.6-35B-A3B** (16.6 GB MoE) on 32 GB: keep active/attention on GPU, spill experts.
- `-ngl 99` — all layers on GPU (default target on this box; everything fits in VRAM).
- `--mlock` — pin weights (no compression/swap of the model itself).
- Avoid `--swa-full` — large KV memory cost for almost no benefit.
- `iogpu.wired_limit_mb` (sysctl) — raise the GPU's wired-memory ceiling if a big model is
  refused; do this deliberately, it steals from the OS.

KV memory rule of thumb: per-token KV ≈ (2 × n_layers × n_kv_heads × head_dim × bytes).
q8_0 KV halves the bytes. For hybrid/MoE models count **attention** layers only.

## Running the DoE

`tuning/doe.sh` wraps `llama-batched-bench` (the canonical throughput/concurrency tool). It
loads its own model copy, so it does not disturb a running `llama-server`, but it consumes
GPU + memory — run it when the box is otherwise idle.

```sh
# full sweep on the dense 4B (ubatch × KV × parallel-load grid)
tuning/doe.sh ~/.llama/models/Qwen3-4B-Instruct-2507-Q8_0.gguf 32768 512 128

# bounded/quick sweep via env overrides
UBATCH="512 1024" BATCH="2048" KV="none q8_0" NPL="1 2 4 8 16" \
  tuning/doe.sh <model.gguf> <ctx> <ppTokens> <genTokens>

# MoE model: add a CPU-offload run (edit doe.sh to add -ncmoe, or bench manually)
```

Output: `tuning/results/doe-<stamp>.csv` with columns
`model,ctx,batch,ubatch,kv,fa,npl,pp_tok_s,tg_tok_s,total_tok_s`, plus a raw `.log`. The
script prints the top configs by `total_tok_s` at the end. See `tuning/grid.toml` for the
factor rationale.

## Reading results & picking a winner

1. Sort by `total_tok_s` (aggregate). Higher = better throughput under load.
2. Find the **knee** in `npl`: the point where `total_tok_s` stops climbing as you add
   parallelism. That `-np` is your concurrency sweet spot — going higher only adds latency.
3. Cross-check `tg_tok_s` (generation speed) if single-request latency matters to you.
4. Confirm the winning config did **not** spill memory: while it ran, `vm_stat` compressor
   pages and swapouts should be stable. A config that "wins" by swapping is a loser in prod.
5. For "max concurrent models": pick per-model configs whose combined resident memory (model
   weights + KV at chosen `-np`/`-c`) stays under ~28 GB (leave headroom for the OS).

Example from a real bounded run on the 4B (ctx 4096): at `-np 4` aggregate hit ~422 tok/s
vs ~218 at `-np 1` — continuous batching + concurrency nearly doubled throughput. That gap
is exactly what the default `-np 1` servers leave on the table.

## Applying the winner

Edit `config.toml` → `[[local_runtime.profiles]]`: set the profile's `flags` to the winning
combination. Then start the tuned server from config (single source of truth):

```sh
./launch-local.sh <profile-name>      # reads model/port/flags from config.toml
```

Re-run one bench point after applying to confirm the live server matches the DoE numbers.
Keep the CSV in `tuning/results/` as the evidence trail for why the config is what it is.
