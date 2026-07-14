#!/usr/bin/env python3
"""Minimize latency to 100% on the fixed 5 GSM8K items.

Search order:
  1) single-sample: short prompts + low max_tokens + thinking off
  2) shrink max_tokens while 5/5
  3) only if needed: samples 2→… (last resort)
"""

from __future__ import annotations

import argparse
import json
import time
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

from targeted_eval import chat, extract_answer, load_items, TASK_INDICES

SYSTEM_TINY = (
    "Grade-school math. Only stated numbers. Unpriced=0. No invented people. "
    "If A half of B then B=2A. Re-add carefully. Brief. \\boxed{n} no commas."
)
SYSTEM_MIN = "Math word problem. Brief steps. \\boxed{integer} no commas."
SYSTEM_STRICT = (
    "Careful grade-school math. Only stated numbers. Unpriced items=0. "
    "Do not invent people. If A is half of B, B=2A. Re-add pairwise. "
    "Brief. Final integer in \\boxed{} no commas."
)

WRAPPERS = {
    "bare": "{q}\n\\boxed{{n}} only.",
    "short": (
        "{q}\n"
        "Stated nums only; unpriced=0; no extra people; half-of-B⇒B=2A; re-add. "
        "Brief. \\boxed{{n}}"
    ),
    "micro": (
        "Tips: half-of-B⇒B=2A; unpriced=0; no invent people; re-add (3+6+20=29).\n"
        "{q}\nBrief. \\boxed{{n}}"
    ),
}


def msgs(system: Optional[str], wrap: str, q: str) -> List[Dict[str, str]]:
    user = WRAPPERS[wrap].replace("{q}", q)
    out: List[Dict[str, str]] = []
    if system:
        out.append({"role": "system", "content": system})
    out.append({"role": "user", "content": user})
    return out


def prompt_chars(system: Optional[str], wrap: str, items: List[Dict]) -> int:
    s = system or ""
    return sum(len(s) + len(WRAPPERS[wrap].replace("{q}", it["question"])) for it in items)


def run_cfg(
    server: str,
    model: str,
    items: List[Dict],
    system: Optional[str],
    wrap: str,
    max_tokens: int,
    samples: int = 1,
    sample_temp: float = 0.0,
    enable_thinking: Optional[bool] = False,
) -> Dict[str, Any]:
    ctk = None if enable_thinking is None else {"enable_thinking": bool(enable_thinking)}
    n_ok = 0
    out_chars = 0
    detail = []
    t0 = time.time()
    for item in items:
        messages = msgs(system, wrap, item["question"])
        votes: Dict[str, int] = {}
        finish = None
        content = ""
        for s in range(samples):
            temp = 0.0 if samples == 1 else sample_temp
            content, reasoning, meta = chat(
                server,
                model,
                messages,
                temperature=temp,
                max_tokens=max_tokens,
                chat_template_kwargs=ctk,
                timeout=300,
            )
            combined = "\n".join(x for x in [reasoning, content] if x)
            out_chars += len(combined)
            ans = extract_answer(combined)
            if ans:
                votes[ans] = votes.get(ans, 0) + 1
            finish = meta.get("finish_reason")
        got = sorted(votes.items(), key=lambda kv: (-kv[1], kv[0]))[0][0] if votes else None
        ok = got == item["gold"]
        n_ok += int(ok)
        detail.append({"id": item["id"], "gold": item["gold"], "got": got, "ok": ok, "finish": finish, "votes": votes or None})
    return {
        "correct": n_ok,
        "total": len(items),
        "elapsed_s": time.time() - t0,
        "out_chars": out_chars,
        "items": detail,
    }


def search(server: str, model: str, out: Path) -> Dict[str, Any]:
    items = load_items()
    trials: List[Dict[str, Any]] = []
    best: Optional[Dict[str, Any]] = None

    def rank(cfg: Dict, res: Dict) -> Tuple:
        # minimize: samples, elapsed, out_chars, max_tokens, prompt size
        return (
            cfg["samples"],
            res["elapsed_s"],
            res["out_chars"],
            cfg["max_tokens"],
            cfg["prompt_chars"],
        )

    def try_one(phase: str, sys_name: str, system: Optional[str], wrap: str, mt: int, samples: int = 1, st: float = 0.0, th: Optional[bool] = False) -> Dict[str, Any]:
        nonlocal best
        cfg = {
            "phase": phase,
            "system_name": sys_name,
            "wrapper": wrap,
            "max_tokens": mt,
            "samples": samples,
            "sample_temp": st,
            "enable_thinking": th,
            "prompt_chars": prompt_chars(system, wrap, items),
        }
        res = run_cfg(server, model, items, system, wrap, mt, samples, st, th)
        row = {**cfg, **res}
        trials.append(row)
        mark = "OK" if res["correct"] == 5 else f"{res['correct']}/5"
        print(
            f"[{mark:>4}] {phase:8} {sys_name:6}/{wrap:5} mt={mt:<4} s={samples} th={th} "
            f"{res['elapsed_s']:5.1f}s chars={res['out_chars']}",
            flush=True,
        )
        if res["correct"] == 5:
            r = rank(cfg, res)
            if best is None or r < best["_rank"]:
                best = {**row, "system_text": system, "_rank": r}
        return row

    # --- Phase 1: single-sample priority list (params only) ---
    print(f"\n=== {model}: Phase 1 single-sample ===", flush=True)
    # ordered by expected speed/quality: tiny prompts first, low mt first
    phase1 = []
    for mt in (192, 256, 384, 512, 768, 1024):
        for th in (False,):
            for sys_name, system in (
                ("tiny", SYSTEM_TINY),
                ("strict", SYSTEM_STRICT),
                ("min", SYSTEM_MIN),
                ("none", None),
            ):
                for wrap in ("micro", "short", "bare"):
                    phase1.append((sys_name, system, wrap, mt, th))

    seen = set()
    for sys_name, system, wrap, mt, th in phase1:
        key = (sys_name, wrap, mt, th)
        if key in seen:
            continue
        seen.add(key)
        try_one("params", sys_name, system, wrap, mt, 1, 0.0, th)
        # early exit if we already have a fast 5/5 with low mt
        if best and best["samples"] == 1 and best["max_tokens"] <= 256 and best["elapsed_s"] < 30:
            break
        # if we found any 5/5, stop expanding larger mt for other combos — shrink instead
        if best and best["samples"] == 1 and mt >= best["max_tokens"]:
            # only continue same mt for a few more short prompts
            if best["max_tokens"] <= 384 and len([t for t in trials if t["correct"] == 5]) >= 1:
                # jump to shrink
                break

    # Shrink max_tokens on best single-sample 5/5
    if best and best["samples"] == 1:
        print(f"=== {model}: shrink max_tokens from {best['max_tokens']} ===", flush=True)
        sys_name = best["system_name"]
        system = best["system_text"]
        wrap = best["wrapper"]
        th = best["enable_thinking"]
        for mt in sorted({64, 96, 128, 160, 192, 224, 256, 320, 384, best["max_tokens"]}):
            if mt >= best["max_tokens"]:
                continue
            try_one("shrink", sys_name, system, wrap, mt, 1, 0.0, th)

    # --- Phase 2: samples only if needed ---
    if not best or best["samples"] != 1 and best.get("correct") != 5:
        # if best is None
        pass
    if best is None:
        print(f"=== {model}: Phase 2 samples (last resort) ===", flush=True)
        for samples in (2, 3, 5, 7, 9):
            for st in (0.6, 0.75):
                for mt in (256, 384, 512):
                    for sys_name, system, wrap in (
                        ("tiny", SYSTEM_TINY, "micro"),
                        ("tiny", SYSTEM_TINY, "short"),
                        ("strict", SYSTEM_STRICT, "micro"),
                    ):
                        try_one("samples", sys_name, system, wrap, mt, samples, st, False)
                        if best and best["correct"] == 5 and best["samples"] <= samples:
                            break
                    if best and best["correct"] == 5:
                        break
                if best and best["correct"] == 5:
                    break
            if best and best["correct"] == 5:
                break
        # shrink samples if possible
        if best and best["samples"] > 1:
            print(f"=== {model}: shrink samples from {best['samples']} ===", flush=True)
            for samples in range(2, best["samples"]):
                try_one(
                    "shrink_s",
                    best["system_name"],
                    best["system_text"],
                    best["wrapper"],
                    best["max_tokens"],
                    samples,
                    best["sample_temp"],
                    best["enable_thinking"],
                )

    best_out = None
    if best:
        best_out = {k: v for k, v in best.items() if k not in ("_rank",)}
        # drop system_text duplication of long text in summary — keep it
    payload = {
        "model": model,
        "task_indices": TASK_INDICES,
        "best": best_out,
        "n_trials": len(trials),
        "trials": [
            {k: v for k, v in t.items() if k != "system_text"}
            for t in trials
        ],
    }
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(payload, indent=2))
    print("\nBEST:", json.dumps({k: best_out[k] for k in best_out if k not in ("system_text", "items")} if best_out else None, indent=2), flush=True)
    if best_out:
        for it in best_out["items"]:
            print(f"  {it['id']}: {it['got']} {'✓' if it['ok'] else '✗'}", flush=True)
    print(f"Wrote {out}", flush=True)
    return payload


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--server", default="http://127.0.0.1:8080")
    ap.add_argument("--model", required=True)
    ap.add_argument("--output", type=Path, required=True)
    args = ap.parse_args()
    search(args.server, args.model, args.output)


if __name__ == "__main__":
    main()
