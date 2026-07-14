#!/usr/bin/env python3
"""Meticulous review of a harness_compare run against hard expectations.

Exit codes:
  0 — all validity checks passed
  1 — one or more validity failures
  2 — usage / missing report
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent


# Expectations for the default sample suite.
# status: required status for that (harness, task)
# score: expected score when status is pass/fail (None = any)
# notes: human rationale
EXPECTATIONS: dict[tuple[str, str], dict] = {
    # dry = gold solutions / markers → must all pass at 1.0
    ("dry", "coding_fix_bug"): {"status": "pass", "score": 1.0, "rationale": "workspace_gold has correct sum_range"},
    ("dry", "coding_fizzbuzz"): {"status": "pass", "score": 1.0, "rationale": "workspace_gold implements fizzbuzz"},
    ("dry", "closed_form_ratio"): {"status": "pass", "score": 1.0, "rationale": "expected.txt is \\boxed{18}"},
    ("dry", "multi_step_report"): {"status": "pass", "score": 1.0, "rationale": "workspace_gold has notes+summary"},
    ("dry", "agent_os_introspection"): {"status": "pass", "score": 1.0, "rationale": "dry writes host_eval_ok marker"},
    ("dry", "agent_os_team"): {"status": "pass", "score": 1.0, "rationale": "dry writes host_eval_ok marker"},
    # jewell coding must SKIP (no terminal/write_file capability)
    ("jewell", "coding_fix_bug"): {
        "status": "skip",
        "score": 0.0,
        "rationale": "jewell adapter does not claim terminal/write_file/coding",
    },
    ("jewell", "coding_fizzbuzz"): {
        "status": "skip",
        "score": 0.0,
        "rationale": "jewell adapter does not claim terminal/write_file/coding",
    },
    # jewell host gates should pass if cargo works
    ("jewell", "agent_os_introspection"): {
        "status": "pass",
        "score": 1.0,
        "rationale": "eval_introspection tool_plan gate must be green",
    },
    ("jewell", "agent_os_team"): {
        "status": "pass",
        "score": 1.0,
        "rationale": "team_collaboration_green host test must be green",
    },
    # closed_form: pass preferred; allow fail if model wrong; forbid skip
    ("jewell", "closed_form_ratio"): {
        "status_in": ("pass", "fail", "error"),
        "forbid_status": ("skip",),
        "gold_answer": "18",
        "rationale": "48 - 18 - 12 = 18; grader extracts last boxed/int",
    },
    ("hermes", "closed_form_ratio"): {
        "status_in": ("pass", "fail", "error", "skip"),
        "gold_answer": "18",
        "rationale": "same gold; skip only if hermes CLI missing",
    },
    # hermes coding: pass/fail/error if CLI present; skip only if missing binary
    ("hermes", "coding_fix_bug"): {
        "status_in": ("pass", "fail", "error", "skip"),
        "rationale": "requires real file edit; weak local models often fail",
    },
    ("hermes", "coding_fizzbuzz"): {
        "status_in": ("pass", "fail", "error", "skip"),
        "rationale": "requires creating fizzbuzz.py",
    },
    # hermes must SKIP host_eval tasks (no host_eval capability)
    ("hermes", "agent_os_introspection"): {
        "status": "skip",
        "rationale": "hermes adapter does not claim host_eval",
    },
    ("hermes", "agent_os_team"): {
        "status": "skip",
        "rationale": "hermes adapter does not claim host_eval",
    },
}


def load_report(run_dir: Path) -> dict:
    p = run_dir / "report.json"
    if not p.is_file():
        raise FileNotFoundError(p)
    return json.loads(p.read_text(encoding="utf-8"))


def extract_answer(text: str) -> str:
    boxes = re.findall(r"\\boxed\{([^}]*)\}", text)
    if boxes:
        return re.sub(r"[^\d-]", "", boxes[-1].strip())
    nums = re.findall(r"-?\d+", text)
    return nums[-1] if nums else ""


def check_item(it: dict, run_dir: Path) -> list[str]:
    """Return list of failure strings (empty = ok)."""
    fails: list[str] = []
    h, tid = it["harness"], it["task_id"]
    key = (h, tid)
    exp = EXPECTATIONS.get(key)
    status = it["status"]
    score = float(it.get("score") or 0.0)

    # Structural integrity
    item_dir = run_dir / h / tid / f"run-{it['run_index']}"
    if not item_dir.is_dir():
        fails.append(f"missing item dir {item_dir}")
        return fails
    meta_p = item_dir / "meta.json"
    if not meta_p.is_file():
        fails.append("missing meta.json")
    else:
        meta = json.loads(meta_p.read_text(encoding="utf-8"))
        if meta.get("status") != status:
            fails.append(f"meta.status {meta.get('status')!r} != report status {status!r}")

    if status in ("pass", "fail"):
        grade_p = item_dir / "grade.json"
        if not grade_p.is_file():
            fails.append("missing grade.json for scored item")
        else:
            g = json.loads(grade_p.read_text(encoding="utf-8"))
            if bool(g.get("correct")) != bool(it.get("correct")):
                fails.append(
                    f"grade.correct={g.get('correct')} != item.correct={it.get('correct')}"
                )
            if abs(float(g.get("score", 0)) - score) > 1e-6:
                fails.append(f"grade.score={g.get('score')} != item.score={score}")

    # Score/status consistency
    if status == "pass" and not it.get("correct"):
        fails.append("status=pass but correct=false")
    if status == "pass" and score <= 0:
        fails.append("status=pass but score<=0")
    if status == "fail" and it.get("correct"):
        fails.append("status=fail but correct=true")
    if status in ("skip", "error") and score != 0.0:
        fails.append(f"status={status} but score={score} (must be 0)")

    if exp is None:
        return fails  # no hard expectation

    if "status" in exp and status != exp["status"]:
        fails.append(
            f"expected status={exp['status']!r} got {status!r} ({exp.get('rationale','')})"
        )
    if "status_in" in exp and status not in exp["status_in"]:
        fails.append(
            f"status {status!r} not in allowed {exp['status_in']} ({exp.get('rationale','')})"
        )
    if "forbid_status" in exp and status in exp["forbid_status"]:
        fails.append(f"forbidden status {status!r} ({exp.get('rationale','')})")
    if "score" in exp and status in ("pass", "fail") and abs(score - float(exp["score"])) > 1e-6:
        fails.append(f"expected score={exp['score']} got {score}")

    # Closed-form gold re-check
    if exp.get("gold_answer") and status == "pass":
        ans_path = item_dir / "workspace" / "agent_answer.txt"
        if ans_path.is_file():
            got = extract_answer(ans_path.read_text(encoding="utf-8"))
            if got != str(exp["gold_answer"]):
                fails.append(
                    f"pass but extracted answer {got!r} != gold {exp['gold_answer']!r}"
                )

    # Re-run grader for coding/multi_step pass items to confirm reproducibility
    if status == "pass" and it.get("track") in ("coding", "multi_step", "closed_form"):
        check = ROOT / "tasks" / tid / "tests" / "check.py"
        ws = item_dir / "workspace"
        if check.is_file() and ws.is_dir():
            import subprocess

            proc = subprocess.run(
                [sys.executable, str(check), str(ws)],
                capture_output=True,
                text=True,
                timeout=30,
            )
            try:
                payload = json.loads(proc.stdout.strip().splitlines()[-1])
            except Exception:
                fails.append(f"re-grade non-JSON: {proc.stdout[:200]!r}")
            else:
                if not payload.get("correct"):
                    fails.append(f"re-grade FAIL after reported pass: {payload}")
                if abs(float(payload.get("score", 0)) - score) > 1e-6:
                    fails.append(
                        f"re-grade score {payload.get('score')} != reported {score}"
                    )

    return fails


def arithmetic_gold_check() -> list[str]:
    """Independent proof that closed_form gold is 18."""
    apples = 48
    mon = apples * 3 // 8  # 18
    tue = apples // 4  # 12
    remain = apples - mon - tue  # 18
    if remain != 18:
        return [f"gold arithmetic broken: remain={remain}"]
    return []


def sum_range_gold_check() -> list[str]:
    fails = []
    # buggy seed
    def buggy(lo, hi):
        total = 0
        x = lo
        while x < hi:
            total += x
            x += 1
        return total

    def fixed(lo, hi):
        total = 0
        x = lo
        while x <= hi:
            total += x
            x += 1
        return total

    cases = [((1, 1), 1), ((1, 3), 6), ((0, 10), 55), ((-2, 2), 0)]
    for (lo, hi), want in cases:
        if fixed(lo, hi) != want:
            fails.append(f"fixed sum_range({lo},{hi})!={want}")
        if (lo, hi) != (1, 1) and buggy(lo, hi) == want:
            # for inclusive ranges with hi>lo, buggy should differ
            if hi > lo and buggy(lo, hi) == want:
                fails.append(f"buggy sum_range unexpectedly correct for ({lo},{hi})")
    if buggy(1, 3) == 6:
        fails.append("buggy seed would already pass tests — task invalid")
    # Buggy loop uses x < hi → sum(1,3) must be 1+2=3 (excludes 3)
    if buggy(1, 3) != 3:
        fails.append(f"buggy seed unexpected: sum_range(1,3)={buggy(1,3)}, want 3")
    return fails


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--run-dir", type=Path, required=True)
    args = ap.parse_args()
    run_dir = args.run_dir
    if not run_dir.is_dir():
        print(f"missing run dir {run_dir}", file=sys.stderr)
        return 2

    print(f"=== Review {run_dir.name} ===\n")
    issues: list[str] = []
    issues.extend(arithmetic_gold_check())
    issues.extend(sum_range_gold_check())
    if issues:
        print("GOLD/TASK DESIGN FAILURES:")
        for i in issues:
            print(f"  ✗ {i}")
        return 1
    print("✓ closed_form gold arithmetic: 48 - 18 - 12 = 18")
    print("✓ coding_fix_bug seed is wrong (excludes hi); gold inclusive sum is correct")

    report = load_report(run_dir)
    items = report.get("items") or []
    print(f"\nReport items: {len(items)}")
    print("Summary harnesses:")
    for h, s in (report.get("summary") or {}).get("harnesses", {}).items():
        print(
            f"  {h}: avg={s['avg_score']:.2f} solid_base={s['solid_base']:.2f} "
            f"pass_rate={s['pass_rate']:.0%} scored={s['n_scored']} "
            f"skip={s['n_skip']} err={s['n_error']}"
        )

    print("\n--- Per-item audit ---")
    n_fail = 0
    for it in items:
        key = f"{it['harness']}/{it['task_id']}#run{it['run_index']}"
        fails = check_item(it, run_dir)
        mark = "PASS" if not fails else "INVALID"
        print(
            f"[{mark}] {key} status={it['status']} score={it['score']} "
            f"detail={str(it.get('detail',''))[:80]}"
        )
        for f in fails:
            print(f"       ✗ {f}")
            n_fail += 1

        # Extra: dump hermes/jewell artifacts when error for human review
        if it["status"] in ("error", "fail") and it["harness"] in ("hermes", "jewell"):
            item_dir = run_dir / it["harness"] / it["task_id"] / f"run-{it['run_index']}"
            for name in ("agent_stderr.txt", "agent_stdout.txt", "grade.json"):
                p = item_dir / name
                if p.is_file():
                    text = p.read_text(encoding="utf-8", errors="replace")
                    print(f"       · {name} ({len(text)} bytes):")
                    for line in text.strip().splitlines()[:12]:
                        print(f"         {line[:120]}")

    print("\n--- Validity verdict ---")
    # Leaderboard must not count skips as failures in avg over empty — already handled
    # Dry must be perfect if present
    dry_items = [i for i in items if i["harness"] == "dry"]
    if dry_items and not all(i["status"] == "pass" for i in dry_items):
        print("✗ dry harness is the grader sanity check — any non-pass means broken tasks/graders")
        n_fail += 1
    else:
        if dry_items:
            print(f"✓ dry harness: {len(dry_items)}/{len(dry_items)} pass (graders + gold valid)")

    if n_fail == 0:
        print("✓ All audited items consistent with expectations and re-grades.")
        print("\nInterpretation guide:")
        print("  • dry PASS  → task design + grader correct")
        print("  • jewell skip on coding → expected (capability honesty)")
        print("  • jewell pass on agent_os → structural gates green")
        print("  • hermes skip on agent_os → expected (no host_eval)")
        print("  • hermes coding pass/fail → real agent skill on this model/ctx")
        print("  • closed_form pass → extracted answer matched gold 18")
        return 0

    print(f"✗ {n_fail} validity issue(s) — do not trust leaderboard until fixed")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
