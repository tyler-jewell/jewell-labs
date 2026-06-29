"""Benchmark-results dashboard plugin — backend API.

Serves normalized benchmark runs written by the runner into
``~/.hermes/plugins/benchmark-results/data/run-*.json``.

Mounted by the Hermes dashboard plugin system at
``/api/plugins/benchmark-results/``.
"""

import json
from pathlib import Path

from fastapi import APIRouter

router = APIRouter()

DATA_DIR = Path.home() / ".hermes" / "plugins" / "benchmark-results" / "data"


def _load_runs():
    runs = []
    if DATA_DIR.is_dir():
        for f in DATA_DIR.glob("run-*.json"):
            try:
                runs.append(json.loads(f.read_text()))
            except Exception:
                # skip malformed/partial files rather than 500 the whole tab
                continue
    # newest first by date (epoch seconds), falling back to filename order
    runs.sort(key=lambda r: r.get("date") or 0, reverse=True)
    return runs


@router.get("/runs")
async def runs():
    """All normalized benchmark runs, newest first."""
    return {"runs": _load_runs()}


@router.get("/latest")
async def latest():
    """Most recent benchmark run, or an empty object if none exist."""
    all_runs = _load_runs()
    return all_runs[0] if all_runs else {}
