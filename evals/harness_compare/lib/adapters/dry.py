from __future__ import annotations

import shutil
import time
from pathlib import Path

from ..task import Task
from .base import AdapterResult, BaseAdapter


class DryAdapter(BaseAdapter):
    """No agent: apply gold solution if present, else leave workspace as seeded.

    Use to validate graders. If `workspace_gold/` exists next to `workspace/`,
    copy it over the run workspace so check.py should PASS.
    """

    name = "dry"
    provides = {
        "terminal",
        "write_file",
        "read_file",
        "coding",
        "closed_form",
        "multi_step",
        "agent_os",
        "host_eval",
    }

    def run(self, task: Task, workspace: Path, instruction: str, timeout_s: int) -> AdapterResult:
        t0 = time.perf_counter()
        gold = task.path / "workspace_gold"
        if gold.is_dir():
            # replace workspace contents with gold
            for child in workspace.iterdir():
                if child.is_dir():
                    shutil.rmtree(child)
                else:
                    child.unlink()
            for item in gold.iterdir():
                dest = workspace / item.name
                if item.is_dir():
                    shutil.copytree(item, dest)
                else:
                    shutil.copy2(item, dest)
            detail = "applied workspace_gold"
        else:
            # closed_form: write expected answer file if tests/expected.txt exists
            expected = task.path / "tests" / "expected.txt"
            if expected.is_file():
                (workspace / "agent_answer.txt").write_text(
                    expected.read_text(encoding="utf-8"), encoding="utf-8"
                )
                detail = "wrote expected answer for closed_form"
            else:
                detail = "no gold; left seed workspace"
        # host_eval dry: write a success marker
        if "host_eval" in task.capabilities:
            (workspace / "host_eval_ok.txt").write_text("ok\n", encoding="utf-8")
            detail = "host_eval dry marker"
        t_ms = int((time.perf_counter() - t0) * 1000)
        _ = instruction, timeout_s
        return AdapterResult(status="ok", stdout=detail, t_ms=t_ms, detail=detail)
