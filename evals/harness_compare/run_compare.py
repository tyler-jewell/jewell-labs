#!/usr/bin/env python3
"""ARCHIVED — use the Rust runner instead:

  cargo run -q --bin eval_compare -- --harnesses dry,jewell,hermes --tasks all

This file remains only as historical reference. Prefer:
  rust-agent/src/eval/compare/ + bin/eval_compare.rs
  Evals tab → “Compare harnesses”

Examples (legacy):
  python3 run_compare.py --harnesses dry --tasks all
"""
from __future__ import annotations

import argparse
import json
import sys
import traceback
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from lib.adapters import get_adapter  # noqa: E402
from lib.report import ItemResult, write_report  # noqa: E402
from lib.score import run_grader  # noqa: E402
from lib.task import discover_tasks  # noqa: E402


def main() -> int:
    ap = argparse.ArgumentParser(description="Compare agent harnesses on shared tasks")
    ap.add_argument(
        "--harnesses",
        default="dry",
        help="comma list: dry,hermes,jewell",
    )
    ap.add_argument(
        "--tasks",
        default="all",
        help="all | track names | task ids (comma-separated)",
    )
    ap.add_argument("--n-runs", type=int, default=None, help="overrides task n_runs_default")
    ap.add_argument(
        "--run-id",
        default=None,
        help="artifact folder name under runs/",
    )
    ap.add_argument(
        "--tasks-dir",
        type=Path,
        default=ROOT / "tasks",
    )
    ap.add_argument(
        "--runs-dir",
        type=Path,
        default=ROOT / "runs",
    )
    args = ap.parse_args()

    harnesses = [h.strip() for h in args.harnesses.split(",") if h.strip()]
    tasks = discover_tasks(args.tasks_dir, args.tasks)
    if not tasks:
        print(f"no tasks matched --tasks {args.tasks!r} under {args.tasks_dir}", file=sys.stderr)
        return 2

    run_id = args.run_id or datetime.now(timezone.utc).strftime("compare-%Y%m%dT%H%M%SZ")
    run_dir = args.runs_dir / run_id
    run_dir.mkdir(parents=True, exist_ok=True)

    adapters = {h: get_adapter(h) for h in harnesses}
    items: list[ItemResult] = []

    print(f"run_id={run_id}")
    print(f"harnesses={harnesses}")
    print(f"tasks={[t.id for t in tasks]}")

    for harness in harnesses:
        adapter = adapters[harness]
        for task in tasks:
            n_runs = args.n_runs if args.n_runs is not None else task.n_runs_default
            n_runs = max(1, n_runs)
            for run_i in range(1, n_runs + 1):
                item_dir = run_dir / harness / task.id / f"run-{run_i}"
                item_dir.mkdir(parents=True, exist_ok=True)
                workspace = task.materialize_workspace(item_dir)
                instruction = task.instruction()

                meta = {
                    "harness": harness,
                    "task_id": task.id,
                    "track": task.track,
                    "run_index": run_i,
                    "timeout_s": task.timeout_s,
                    "capabilities": task.capabilities,
                }
                print(f"→ {harness} {task.id} run={run_i}/{n_runs} …", flush=True)

                try:
                    ok, missing = adapter.can_run(task)
                    if not ok:
                        result_status = "skip"
                        ar_detail = f"missing capabilities: {missing}"
                        ar_t = 0
                        ar_stdout = ""
                        ar_stderr = ""
                        grade_correct = False
                        grade_score = 0.0
                        grade_metrics: dict = {}
                        grade_detail = ar_detail
                    else:
                        ar = adapter.run(task, workspace, instruction, task.timeout_s)
                        (item_dir / "agent_stdout.txt").write_text(ar.stdout, encoding="utf-8")
                        (item_dir / "agent_stderr.txt").write_text(ar.stderr, encoding="utf-8")
                        ar_t = ar.t_ms
                        ar_stdout = ar.stdout
                        ar_stderr = ar.stderr
                        ar_detail = ar.detail
                        missing = ar.capabilities_missing

                        if ar.status == "skip":
                            result_status = "skip"
                            grade_correct = False
                            grade_score = 0.0
                            grade_metrics = {}
                            grade_detail = ar.detail
                        elif ar.status == "error":
                            result_status = "error"
                            grade_correct = False
                            grade_score = 0.0
                            grade_metrics = {}
                            grade_detail = ar.detail
                            # still try grader? no — agent never finished cleanly
                        else:
                            # For host_eval jewell path: copy exit signal into workspace for grader
                            if ar.extra.get("exit_code") is not None:
                                (workspace / "host_exit_code.txt").write_text(
                                    str(ar.extra["exit_code"]), encoding="utf-8"
                                )
                            if ar.stdout and not (workspace / "agent_answer.txt").exists():
                                # closed_form may already have written it
                                if task.track == "closed_form":
                                    (workspace / "agent_answer.txt").write_text(
                                        ar.stdout, encoding="utf-8"
                                    )

                            grade = run_grader(task.check_script, workspace)
                            (item_dir / "grade.json").write_text(
                                json.dumps(grade.to_dict(), indent=2) + "\n",
                                encoding="utf-8",
                            )
                            grade_correct = grade.correct
                            grade_score = grade.score
                            grade_metrics = grade.metrics
                            grade_detail = grade.detail or ar.detail
                            result_status = "pass" if grade.correct else "fail"

                except Exception as e:
                    result_status = "error"
                    grade_correct = False
                    grade_score = 0.0
                    grade_metrics = {}
                    grade_detail = f"runner exception: {e}"
                    ar_t = 0
                    missing = []
                    (item_dir / "agent_stderr.txt").write_text(
                        traceback.format_exc(), encoding="utf-8"
                    )

                meta["status"] = result_status
                meta["detail"] = grade_detail
                (item_dir / "meta.json").write_text(
                    json.dumps(meta, indent=2) + "\n", encoding="utf-8"
                )

                item = ItemResult(
                    harness=harness,
                    task_id=task.id,
                    track=task.track,
                    run_index=run_i,
                    status=result_status,
                    correct=grade_correct,
                    score=grade_score if result_status in ("pass", "fail") else 0.0,
                    t_ms=ar_t,
                    detail=grade_detail,
                    metrics=grade_metrics,
                    workspace=str(workspace),
                    capabilities_missing=list(missing) if missing else [],
                )
                items.append(item)
                mark = {"pass": "PASS", "fail": "FAIL", "skip": "SKIP", "error": "ERR"}[
                    result_status
                ]
                print(f"  [{mark}] score={item.score:.2f} t_ms={item.t_ms} {item.detail[:100]}")

    meta_run = {
        "harnesses": harnesses,
        "tasks": [t.id for t in tasks],
        "n_runs_flag": args.n_runs,
        "tasks_dir": str(args.tasks_dir),
    }
    json_path, md_path = write_report(run_dir, items, meta_run)
    print(f"\nwrote {json_path}")
    print(f"wrote {md_path}")

    # exit 0 if dry all pass; else 0 always for compare (report is the product)
    if harnesses == ["dry"]:
        bad = [i for i in items if i.status != "pass"]
        return 1 if bad else 0
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
