#!/usr/bin/env python3
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ws = Path(sys.argv[1])
notes = ws / "notes.md"
summary = ws / "summary.txt"
if not notes.is_file() or not summary.is_file():
    print(
        json.dumps(
            {
                "correct": False,
                "score": 0.0,
                "detail": f"notes={notes.is_file()} summary={summary.is_file()}",
            }
        )
    )
    raise SystemExit(0)

ntext = notes.read_text(encoding="utf-8")
bullets = [ln for ln in ntext.splitlines() if ln.strip().startswith("-")]
words_ok = all(w in ntext.lower() for w in ("alpha", "beta", "gamma"))
bullets_ok = len(bullets) >= 3

stext = summary.read_text(encoding="utf-8").strip()
sum_ok = stext == "topics: alpha, beta, gamma"

score = (0.4 if bullets_ok else 0) + (0.3 if words_ok else 0) + (0.3 if sum_ok else 0)
print(
    json.dumps(
        {
            "correct": bullets_ok and words_ok and sum_ok,
            "score": score,
            "metrics": {
                "bullets": len(bullets),
                "words_ok": words_ok,
                "sum_ok": sum_ok,
            },
            "detail": "ok" if score == 1.0 else f"bullets={len(bullets)} words={words_ok} sum={sum_ok}",
        }
    )
)
