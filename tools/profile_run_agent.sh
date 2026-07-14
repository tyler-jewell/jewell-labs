#!/usr/bin/env bash
# Profile run_agent implementations: dry-parse (runner only) + warm chat (model already up).
set -euo pipefail
cd "$(dirname "$0")/.."
OUT="evals/runs/run_agent_profile.json"
PROMPT='What is 2+2? Answer with only \boxed{4}.'
N_WARM=5
N_DRY=20

# macOS time -l prints maxrss in bytes on recent macOS? actually kilobytes historically
# Use a python harness for consistent timing + peak RSS via resource/ps

python3 - <<'PY'
import json, os, re, subprocess, time, statistics, resource
from pathlib import Path

ROOT = Path(".").resolve()
PROMPT = r"What is 2+2? Answer with only \boxed{4}."
N_DRY = 20
N_CHAT = 5

variants = [
    ("python", ["python3", "tools/run_agent.py"]),
    ("bun", ["bun", "tools/run_agent.ts"]),
    ("bash", ["bash", "tools/run_agent.sh"]),
    # Node alternative: run TS with bun is primary; add node via process spawn of bun's transpile? 
    # Use node with --experimental-strip-types if available, else skip
]

# Detect node strip-types
node_ok = False
try:
    r = subprocess.run(["node", "--experimental-strip-types", "-e", "console.log(1)"], capture_output=True, text=True, timeout=5)
    node_ok = r.returncode == 0
except Exception:
    pass
if node_ok:
    variants.append(("node-strip-types", ["node", "--experimental-strip-types", "tools/run_agent.ts"]))

# bunx cold: install nothing, use `bunx --bun` with a one-off package - measure bunx hello
# For apples-to-apples agent run via bunx: not supported for local path; measure bunx -e overhead separately

def run_once(cmd, extra_args, env=None):
    full = cmd + extra_args
    t0 = time.perf_counter()
    # child process so we can sample RSS
    p = subprocess.Popen(full, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=env or os.environ.copy())
    # sample RSS via ps while running
    peak_rss_kb = 0
    while True:
        ret = p.poll()
        try:
            ps = subprocess.run(["ps", "-o", "rss=", "-p", str(p.pid)], capture_output=True, text=True)
            if ps.stdout.strip():
                peak_rss_kb = max(peak_rss_kb, int(ps.stdout.strip()))
        except Exception:
            pass
        if ret is not None:
            break
        time.sleep(0.005)
    out, err = p.communicate()
    dt = time.perf_counter() - t0
    return {
        "ok": p.returncode == 0,
        "seconds": dt,
        "peak_rss_kb": peak_rss_kb,
        "stdout_bytes": len(out.encode()),
        "stderr_bytes": len(err.encode()),
        "stdout_preview": out.strip()[:120],
        "stderr_preview": err.strip()[:200],
    }

results = {"variants": {}, "notes": []}

# Cold start: first invocation after brief sleep
for name, cmd in variants:
    # dry-parse many times
    dry_times = []
    dry_rss = []
    for i in range(N_DRY):
        r = run_once(cmd, ["math-tutor", "--dry-parse"])
        if not r["ok"] and i == 0:
            results["variants"][name] = {"error": r["stderr_preview"] or "failed", "dry": r}
            break
        dry_times.append(r["seconds"])
        dry_rss.append(r["peak_rss_kb"])
    else:
        # warm chat
        chat_times = []
        chat_rss = []
        chat_ok = 0
        for i in range(N_CHAT):
            r = run_once(cmd, ["math-tutor", PROMPT])
            chat_times.append(r["seconds"])
            chat_rss.append(r["peak_rss_kb"])
            if r["ok"] and "4" in r["stdout_preview"]:
                chat_ok += 1
        results["variants"][name] = {
            "dry_parse": {
                "n": N_DRY,
                "mean_ms": round(statistics.mean(dry_times) * 1000, 2),
                "median_ms": round(statistics.median(dry_times) * 1000, 2),
                "min_ms": round(min(dry_times) * 1000, 2),
                "max_ms": round(max(dry_times) * 1000, 2),
                "stdev_ms": round(statistics.pstdev(dry_times) * 1000, 2) if len(dry_times) > 1 else 0,
                "mean_peak_rss_kb": round(statistics.mean(dry_rss), 1),
                "max_peak_rss_kb": max(dry_rss),
            },
            "warm_chat": {
                "n": N_CHAT,
                "mean_ms": round(statistics.mean(chat_times) * 1000, 2),
                "median_ms": round(statistics.median(chat_times) * 1000, 2),
                "min_ms": round(min(chat_times) * 1000, 2),
                "max_ms": round(max(chat_times) * 1000, 2),
                "stdev_ms": round(statistics.pstdev(chat_times) * 1000, 2) if len(chat_times) > 1 else 0,
                "mean_peak_rss_kb": round(statistics.mean(chat_rss), 1),
                "max_peak_rss_kb": max(chat_rss),
                "success": chat_ok,
            },
        }

# bunx overhead: package resolution cold start (not full agent)
bunx_times = []
bunx_rss = []
for i in range(5):
    r = run_once(["bunx", "--bun", "-e", "console.log('ok')"], [])
    bunx_times.append(r["seconds"])
    bunx_rss.append(r["peak_rss_kb"])
results["bunx_e_overhead"] = {
    "n": 5,
    "mean_ms": round(statistics.mean(bunx_times) * 1000, 2),
    "median_ms": round(statistics.median(bunx_times) * 1000, 2),
    "mean_peak_rss_kb": round(statistics.mean(bunx_rss), 1),
    "note": "bunx -e only; local agent file is not a registry package so bunx cannot run tools/run_agent.ts as a package",
}

# pure interpreter cold starts
for label, cmd in [
    ("python_cold", ["python3", "-c", "print(1)"]),
    ("bun_cold", ["bun", "-e", "console.log(1)"]),
    ("bash_cold", ["bash", "-c", "echo 1"]),
    ("node_cold", ["node", "-e", "console.log(1)"]),
]:
    times, rss = [], []
    for _ in range(15):
        r = run_once(cmd, [])
        times.append(r["seconds"]); rss.append(r["peak_rss_kb"])
    results.setdefault("runtime_cold_start", {})[label] = {
        "mean_ms": round(statistics.mean(times) * 1000, 2),
        "median_ms": round(statistics.median(times) * 1000, 2),
        "mean_peak_rss_kb": round(statistics.mean(rss), 1),
    }

# server memory (llama-server)
try:
    ps = subprocess.run(["pgrep", "-x", "llama-server"], capture_output=True, text=True)
    pids = ps.stdout.split()
    server_rss = []
    for pid in pids:
        r = subprocess.run(["ps", "-o", "rss=", "-p", pid], capture_output=True, text=True)
        if r.stdout.strip():
            server_rss.append(int(r.stdout.strip()))
    results["llama_server_rss_kb"] = server_rss
except Exception as e:
    results["llama_server_rss_kb"] = str(e)

results["prompt"] = PROMPT
results["agent"] = "math-tutor"
results["model"] = "qwen3-0.6b (server pre-started)"
results["methodology"] = {
    "dry_parse": "parse frontmatter + resolve model only; no HTTP",
    "warm_chat": "POST /v1/chat/completions with server already loaded",
    "rss": "peak sampled via ps RSS of child process during run (KB)",
}

Path("evals/runs").mkdir(parents=True, exist_ok=True)
Path(OUT := "evals/runs/run_agent_profile.json").write_text(json.dumps(results, indent=2))
print(json.dumps(results, indent=2))
print(f"\nWrote {OUT}")
PY
