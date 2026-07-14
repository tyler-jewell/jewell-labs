#!/usr/bin/env python3
"""Post-hoc: import fizzbuzz and check values + optional __main__ run."""
from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
from pathlib import Path

ws = Path(sys.argv[1])
path = ws / "fizzbuzz.py"
if not path.is_file():
    print(json.dumps({"correct": False, "score": 0.0, "metrics": {}, "detail": "missing fizzbuzz.py"}))
    raise SystemExit(0)

spec = importlib.util.spec_from_file_location("fizzbuzz_under_test", path)
mod = importlib.util.module_from_spec(spec)
assert spec.loader is not None
try:
    spec.loader.exec_module(mod)
except Exception as e:
    print(json.dumps({"correct": False, "score": 0.0, "metrics": {}, "detail": f"import error: {e}"}))
    raise SystemExit(0)

if not hasattr(mod, "fizzbuzz"):
    print(json.dumps({"correct": False, "score": 0.0, "metrics": {}, "detail": "no fizzbuzz()"}))
    raise SystemExit(0)

got = mod.fizzbuzz(15)
want = [
    "1", "2", "Fizz", "4", "Buzz", "Fizz", "7", "8", "Fizz", "Buzz",
    "11", "Fizz", "13", "14", "FizzBuzz",
]
fn_ok = got == want

main_ok = False
try:
    proc = subprocess.run(
        [sys.executable, str(path)],
        capture_output=True,
        text=True,
        timeout=10,
        cwd=str(ws),
    )
    main_ok = proc.returncode == 0 and proc.stdout.strip().splitlines() == want
except Exception:
    main_ok = False

score = (0.7 if fn_ok else 0.0) + (0.3 if main_ok else 0.0)
print(
    json.dumps(
        {
            "correct": fn_ok and main_ok,
            "score": score,
            "metrics": {"fn_ok": fn_ok, "main_ok": main_ok},
            "detail": "fizzbuzz(15)" if fn_ok and main_ok else f"fn_ok={fn_ok} main_ok={main_ok}",
        }
    )
)
