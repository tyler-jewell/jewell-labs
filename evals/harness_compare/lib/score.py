from __future__ import annotations

import json
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


@dataclass
class Grade:
    correct: bool
    score: float
    metrics: dict[str, Any] = field(default_factory=dict)
    detail: str = ""
    grader_ok: bool = True
    raw_stdout: str = ""
    raw_stderr: str = ""

    def to_dict(self) -> dict[str, Any]:
        return {
            "correct": self.correct,
            "score": self.score,
            "metrics": self.metrics,
            "detail": self.detail,
            "grader_ok": self.grader_ok,
        }


def run_grader(check_script: Path, workspace: Path, timeout_s: int = 60) -> Grade:
    """Post-hoc grader: check.py workspace_path → JSON on stdout."""
    if not check_script.is_file():
        return Grade(
            correct=False,
            score=0.0,
            detail=f"missing grader {check_script}",
            grader_ok=False,
        )
    try:
        proc = subprocess.run(
            [sys.executable, str(check_script), str(workspace)],
            capture_output=True,
            text=True,
            timeout=timeout_s,
            cwd=str(check_script.parent),
        )
    except subprocess.TimeoutExpired:
        return Grade(
            correct=False,
            score=0.0,
            detail="grader timeout",
            grader_ok=False,
        )
    except OSError as e:
        return Grade(
            correct=False,
            score=0.0,
            detail=f"grader spawn failed: {e}",
            grader_ok=False,
        )

    stdout = proc.stdout or ""
    stderr = proc.stderr or ""
    # last JSON object on stdout
    payload: dict[str, Any] | None = None
    for line in reversed(stdout.strip().splitlines() or [""]):
        line = line.strip()
        if not line:
            continue
        try:
            payload = json.loads(line)
            break
        except json.JSONDecodeError:
            continue
    if payload is None:
        try:
            payload = json.loads(stdout.strip()) if stdout.strip() else None
        except json.JSONDecodeError:
            payload = None

    if payload is None:
        return Grade(
            correct=False,
            score=0.0,
            detail=f"grader non-JSON exit={proc.returncode}",
            grader_ok=proc.returncode == 0,
            raw_stdout=stdout,
            raw_stderr=stderr,
        )

    correct = bool(payload.get("correct", False))
    score = float(payload.get("score", 1.0 if correct else 0.0))
    return Grade(
        correct=correct,
        score=score,
        metrics=dict(payload.get("metrics") or {}),
        detail=str(payload.get("detail") or ""),
        grader_ok=True,
        raw_stdout=stdout,
        raw_stderr=stderr,
    )
