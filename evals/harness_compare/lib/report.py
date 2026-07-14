from __future__ import annotations

import json
from collections import defaultdict
from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


@dataclass
class ItemResult:
    harness: str
    task_id: str
    track: str
    run_index: int
    status: str  # pass | fail | skip | error
    correct: bool
    score: float
    t_ms: int
    detail: str = ""
    metrics: dict[str, Any] = field(default_factory=dict)
    workspace: str = ""
    capabilities_missing: list[str] = field(default_factory=list)

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


def solid_base(scores: list[float]) -> float:
    """WolfBench-style floor: min score across runs (0 if empty)."""
    if not scores:
        return 0.0
    return min(scores)


def summarize(items: list[ItemResult]) -> dict[str, Any]:
    by_harness: dict[str, list[ItemResult]] = defaultdict(list)
    for it in items:
        by_harness[it.harness].append(it)

    harnesses: dict[str, Any] = {}
    for h, rows in sorted(by_harness.items()):
        scored = [r for r in rows if r.status in ("pass", "fail")]
        skips = [r for r in rows if r.status == "skip"]
        errors = [r for r in rows if r.status == "error"]
        # per task: mean over runs, then mean over tasks
        by_task: dict[str, list[ItemResult]] = defaultdict(list)
        for r in scored:
            by_task[r.task_id].append(r)
        task_avgs: list[float] = []
        task_floors: list[float] = []
        task_pass: dict[str, float] = {}
        for tid, trs in sorted(by_task.items()):
            scores = [t.score for t in trs]
            avg = sum(scores) / len(scores)
            floor = solid_base(scores)
            task_avgs.append(avg)
            task_floors.append(floor)
            task_pass[tid] = avg
        harnesses[h] = {
            "n_items": len(rows),
            "n_scored": len(scored),
            "n_skip": len(skips),
            "n_error": len(errors),
            "avg_score": (sum(task_avgs) / len(task_avgs)) if task_avgs else 0.0,
            "solid_base": (sum(task_floors) / len(task_floors)) if task_floors else 0.0,
            "pass_rate": (
                sum(1 for r in scored if r.correct) / len(scored) if scored else 0.0
            ),
            "per_task_avg": task_pass,
            "mean_t_ms": (
                int(sum(r.t_ms for r in scored) / len(scored)) if scored else 0
            ),
        }
    return {"harnesses": harnesses}


def write_report(
    run_dir: Path,
    items: list[ItemResult],
    meta: dict[str, Any],
) -> tuple[Path, Path]:
    run_dir.mkdir(parents=True, exist_ok=True)
    summary = summarize(items)
    payload = {
        "id": run_dir.name,
        "created": datetime.now(timezone.utc).isoformat(),
        "meta": meta,
        "summary": summary,
        "items": [i.to_dict() for i in items],
    }
    json_path = run_dir / "report.json"
    json_path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")

    md_lines = [
        f"# Harness compare `{run_dir.name}`",
        "",
        f"Created: {payload['created']}",
        "",
        "## Leaderboard (outcome grading)",
        "",
        "| Harness | avg_score | solid_base | pass_rate | n_scored | n_skip | n_error | mean_t_ms |",
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for h, s in summary["harnesses"].items():
        md_lines.append(
            f"| {h} | {s['avg_score']:.2f} | {s['solid_base']:.2f} | "
            f"{s['pass_rate']:.0%} | {s['n_scored']} | {s['n_skip']} | "
            f"{s['n_error']} | {s['mean_t_ms']} |"
        )
    md_lines += ["", "## Per-item", "", "| harness | task | run | status | score | t_ms | detail |", "| --- | --- | ---: | --- | ---: | ---: | --- |"]
    for it in items:
        detail = (it.detail or "").replace("|", "/")[:80]
        md_lines.append(
            f"| {it.harness} | {it.task_id} | {it.run_index} | {it.status} | "
            f"{it.score:.2f} | {it.t_ms} | {detail} |"
        )
    md_lines.append("")
    md_path = run_dir / "report.md"
    md_path.write_text("\n".join(md_lines), encoding="utf-8")
    return json_path, md_path
