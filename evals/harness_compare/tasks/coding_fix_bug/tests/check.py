#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

ws = Path(sys.argv[1])
path = ws / "sum_range.py"
if not path.is_file():
    print(json.dumps({"correct": False, "score": 0.0, "detail": "missing sum_range.py"}))
    raise SystemExit(0)

spec = importlib.util.spec_from_file_location("sum_range_ut", path)
mod = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(mod)

cases = [((1, 1), 1), ((1, 3), 6), ((0, 10), 55), ((-2, 2), 0)]
ok = 0
for (lo, hi), want in cases:
    try:
        got = mod.sum_range(lo, hi)
    except Exception:
        got = None
    if got == want:
        ok += 1

score = ok / len(cases)
print(
    json.dumps(
        {
            "correct": ok == len(cases),
            "score": score,
            "metrics": {"cases_ok": ok, "cases_total": len(cases)},
            "detail": f"{ok}/{len(cases)} cases",
        }
    )
)
