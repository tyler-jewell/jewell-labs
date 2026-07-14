#!/usr/bin/env python3
from __future__ import annotations

import json
import sys
from pathlib import Path

ws = Path(sys.argv[1])
if (ws / "host_eval_ok.txt").is_file():
    print(json.dumps({"correct": True, "score": 1.0, "detail": "dry marker"}))
    raise SystemExit(0)

code_path = ws / "host_exit_code.txt"
if not code_path.is_file():
    print(json.dumps({"correct": False, "score": 0.0, "detail": "no host_exit_code"}))
    raise SystemExit(0)
try:
    code = int(code_path.read_text(encoding="utf-8").strip())
except ValueError:
    code = -1
print(
    json.dumps(
        {
            "correct": code == 0,
            "score": 1.0 if code == 0 else 0.0,
            "metrics": {"exit_code": code},
            "detail": f"team host exit={code}",
        }
    )
)
