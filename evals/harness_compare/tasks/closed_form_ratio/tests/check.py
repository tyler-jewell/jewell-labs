#!/usr/bin/env python3
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ws = Path(sys.argv[1])
text = ""
for name in ("agent_answer.txt", "agent_transcript.txt"):
    p = ws / name
    if p.is_file():
        text = p.read_text(encoding="utf-8")
        break
if not text:
    # hermes may only leave stdout in parent; runner also writes agent_answer for closed_form
    print(json.dumps({"correct": False, "score": 0.0, "detail": "no agent_answer.txt"}))
    raise SystemExit(0)

# Prefer last non-empty \boxed{...}; ignore empty \\boxed{} artifacts from streams
boxes = [b.strip() for b in re.findall(r"\\boxed\{([^}]*)\}", text) if b.strip()]
if boxes:
    extracted = boxes[-1]
else:
    nums = re.findall(r"-?\d+", text)
    extracted = nums[-1] if nums else ""

gold = "18"
extracted_n = re.sub(r"[^\d-]", "", extracted)
correct = extracted_n == gold or extracted.strip() == gold
print(
    json.dumps(
        {
            "correct": correct,
            "score": 1.0 if correct else 0.0,
            "metrics": {"extracted": extracted, "gold": gold},
            "detail": f"extracted={extracted!r}",
        }
    )
)
