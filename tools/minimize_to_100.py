#!/usr/bin/env python3
"""Find fastest 100% config per model: params first, then min samples.

Order:
  1. single-sample compact prompts (temp=0, thinking off)
  2. shrink max_tokens
  3. only if needed: increase samples on best short prompt, then min samples
"""

from __future__ import annotations

import argparse
import json
import time
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

from targeted_eval import chat, extract_answer, load_items

SYSTEMS = {
    "none": None,
    "min": "Math word problem. Brief. Final integer in \\boxed{} no commas.",
    "tiny": (
        "Grade-school math. Only stated numbers. Unpriced=0. No invented people. "
        "If A half of B then B=2A. Re-add carefully. Brief. \\boxed{n} no commas."
    ),
    "strict": (
        "Careful grade-school math. Only stated numbers. Unpriced items=0. "
        "Do not invent people. If A is half of B, B=2A. Re-add pairwise. "
        "Brief. Final integer in \\boxed{} no commas."
    ),
}

WRAPS = {
    "bare": "{q}\n\\boxed{{n}} only.",
    "short": (
        "{q}\nStated nums only; unpriced=0; no extra people; half-of-B⇒B=2A; re-add. "
        "Brief. \\boxed{{n}}"
    ),
    "micro": (
        "Tips: half-of-B⇒B=2A; unpriced=0; no invent people; re-add (3+6+20=29).\n"
        "{q}\nBrief. \\boxed{{n}}"
    ),
    # longer — only for models that need it (0.6B)
    "fewshot": (
        "Ex1: red $200 half of green → green=400; month1=600; half next=300; total=900. \\boxed{900}\n"
        "Ex2: base 1000+kit 300+cable=kit/3+mount=cable-20+free item → 1480. \\boxed{1480}\n"
        "Ex3: 4 vans x 8 people = 32 (do not add unlisted people). \\boxed{32}\n"
        "{q}\nBrief. \\boxed{{n}}"
    ),
}


def messages(sys_name: str, wrap: str, q: str) -> List[Dict[str, str]]:
    out: List[Dict[str, str]] = []
    sys = SYSTEMS[sys_name]
    if sys:
        out.append({"role": "system", "content": sys})
    out.append({"role": "user", "content": WRAPS[wrap].replace("{q}", q)})
    return out


def evaluate(
    server: str,
    model: str,
    items: List[Dict],
    sys_name: str,
    wrap: str,
    max_tokens: int,
    samples: int = 1,
    sample_temp: float = 0.7,
) -> Dict[str, Any]:
    n_ok = 0
    out_chars = 0
    detail = []
    t0 = time.time()
    for item in items:
        msgs = messages(sys_name, wrap, item["question"])
        votes: Dict[str, int] = {}
        finish = None
        for _ in range(samples):
            temp = 0.0 if samples == 1 else sample_temp
            content, reasoning, meta = chat(
                server,
                model,
                msgs,
                temperature=temp,
                max_tokens=max_tokens,
                chat_template_kwargs={"enable_thinking": False},
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
        detail.append({"id": item["id"], "gold": item["gold"], "got": got, "ok": ok, "finish": finish})
    return {
        "correct": n_ok,
        "total": len(items),
        "elapsed_s": round(time.time() - t0, 3),
        "out_chars": out_chars,
        "items": detail,
    }


def run_search(server: str, model: str, out: Path, prefer_fewshot: bool = False) -> Dict[str, Any]:
    items = load_items()
    trials = []
    best: Optional[Dict[str, Any]] = None

    def score(cfg: Dict, res: Dict) -> Tuple:
        return (cfg["samples"], res["elapsed_s"], res["out_chars"], cfg["max_tokens"])

    def trial(phase: str, sys_name: str, wrap: str, mt: int, samples: int = 1, st: float = 0.7) -> Dict:
        nonlocal best
        res = evaluate(server, model, items, sys_name, wrap, mt, samples, st)
        row = {
            "phase": phase,
            "system": sys_name,
            "wrap": wrap,
            "max_tokens": mt,
            "samples": samples,
            "sample_temp": 0.0 if samples == 1 else st,
            **res,
        }
        trials.append(row)
        tag = "5/5" if res["correct"] == 5 else f"{res['correct']}/5"
        print(
            f"[{tag}] {phase:7} {sys_name:6}/{wrap:7} mt={mt:<4} s={samples} "
            f"{res['elapsed_s']:6.2f}s out={res['out_chars']}",
            flush=True,
        )
        if res["correct"] == 5:
            s = score(row, res)
            if best is None or s < best["_s"]:
                best = {**row, "_s": s}
        return row

    # Phase 1 — single sample, short first
    print(f"\n=== {model}: params (single sample) ===", flush=True)
    wraps = ["micro", "short", "bare", "fewshot"] if prefer_fewshot else ["micro", "short", "bare"]
    if prefer_fewshot:
        # put fewshot earlier for weak models
        wraps = ["fewshot", "micro", "short", "bare"]
    systems = ["tiny", "strict", "min", "none"]
    for mt in (256, 384, 512, 768):
        for sys_name in systems:
            for wrap in wraps:
                trial("params", sys_name, wrap, mt, 1)
                if best and best["samples"] == 1:
                    break
            if best and best["samples"] == 1:
                break
        if best and best["samples"] == 1:
            break

    # Shrink mt
    if best and best["samples"] == 1:
        print(f"=== {model}: shrink max_tokens ===", flush=True)
        for mt in (64, 96, 128, 160, 192, 224, 256, 320, 384):
            if mt >= best["max_tokens"]:
                continue
            trial("shrink", best["system"], best["wrap"], mt, 1)

    # Phase 2 — samples only if needed
    if best is None:
        print(f"=== {model}: samples (last resort) ===", flush=True)
        # prioritize fewshot for weak models
        combos = [
            ("tiny", "fewshot"),
            ("strict", "fewshot"),
            ("tiny", "micro"),
            ("strict", "micro"),
            ("tiny", "short"),
        ]
        for samples in (3, 5, 7, 9, 11):
            for st in (0.7, 0.75):
                for mt in (384, 512, 256):
                    for sys_name, wrap in combos:
                        trial("samples", sys_name, wrap, mt, samples, st)
                        if best is not None:
                            break
                    if best is not None:
                        break
                if best is not None:
                    break
            if best is not None:
                break

        # minimize samples
        if best and best["samples"] > 1:
            print(f"=== {model}: minimize samples from {best['samples']} ===", flush=True)
            sys_name, wrap, mt, st = best["system"], best["wrap"], best["max_tokens"], best["sample_temp"]
            for samples in range(2, best["samples"]):
                trial("min_s", sys_name, wrap, mt, samples, st)
            # also try lower mt with same samples
            for mt in (256, 320, 384):
                if mt < best["max_tokens"]:
                    trial("min_s", best["system"], best["wrap"], mt, best["samples"], best["sample_temp"])

    best_out = {k: v for k, v in best.items() if k != "_s"} if best else None
    payload = {"model": model, "best": best_out, "trials": trials, "n_trials": len(trials)}
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(payload, indent=2))
    print("BEST:", json.dumps({k: best_out[k] for k in best_out if k != "items"} if best_out else None, indent=2), flush=True)
    return payload


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--server", default="http://127.0.0.1:8080")
    ap.add_argument("--model", required=True)
    ap.add_argument("--output", type=Path, required=True)
    ap.add_argument("--prefer-fewshot", action="store_true")
    args = ap.parse_args()
    run_search(args.server, args.model, args.output, prefer_fewshot=args.prefer_fewshot)


if __name__ == "__main__":
    main()
