from __future__ import annotations

import re
import shutil
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


def _parse_simple_toml(text: str) -> dict[str, Any]:
    """Tiny TOML subset: key = value (str/int/float/bool/list-of-str)."""
    data: dict[str, Any] = {}
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if "=" not in line:
            continue
        key, val = line.split("=", 1)
        key = key.strip()
        val = val.strip()
        if val.startswith("[") and val.endswith("]"):
            inner = val[1:-1].strip()
            if not inner:
                data[key] = []
            else:
                items = []
                for part in re.findall(r'"([^"]*)"|\'([^\']*)\'|([^,\s]+)', inner):
                    items.append(next(p for p in part if p))
                data[key] = items
        elif (val.startswith('"') and val.endswith('"')) or (
            val.startswith("'") and val.endswith("'")
        ):
            data[key] = val[1:-1]
        elif val.lower() in ("true", "false"):
            data[key] = val.lower() == "true"
        elif re.fullmatch(r"-?\d+", val):
            data[key] = int(val)
        elif re.fullmatch(r"-?\d+\.\d+", val):
            data[key] = float(val)
        else:
            data[key] = val
    return data


@dataclass
class Task:
    id: str
    path: Path
    track: str
    timeout_s: int = 300
    capabilities: list[str] = field(default_factory=list)
    n_runs_default: int = 1
    title: str = ""

    @property
    def instruction_path(self) -> Path:
        return self.path / "instruction.md"

    @property
    def workspace_src(self) -> Path:
        return self.path / "workspace"

    @property
    def check_script(self) -> Path:
        return self.path / "tests" / "check.py"

    def instruction(self) -> str:
        return self.instruction_path.read_text(encoding="utf-8")

    def materialize_workspace(self, dest: Path) -> Path:
        dest.mkdir(parents=True, exist_ok=True)
        ws = dest / "workspace"
        if self.workspace_src.is_dir():
            if ws.exists():
                shutil.rmtree(ws)
            shutil.copytree(self.workspace_src, ws)
        else:
            ws.mkdir(parents=True, exist_ok=True)
        return ws


def load_task(task_dir: Path) -> Task:
    toml_path = task_dir / "task.toml"
    if not toml_path.is_file():
        raise FileNotFoundError(f"missing task.toml in {task_dir}")
    meta = _parse_simple_toml(toml_path.read_text(encoding="utf-8"))
    tid = str(meta.get("id") or task_dir.name)
    return Task(
        id=tid,
        path=task_dir,
        track=str(meta.get("track", "coding")),
        timeout_s=int(meta.get("timeout_s", 300)),
        capabilities=list(meta.get("capabilities") or []),
        n_runs_default=int(meta.get("n_runs_default", 1)),
        title=str(meta.get("title") or tid),
    )


def discover_tasks(tasks_root: Path, filter_spec: str = "all") -> list[Task]:
    """filter_spec: 'all' | comma tracks | comma task ids | single id."""
    if not tasks_root.is_dir():
        return []
    tasks: list[Task] = []
    for child in sorted(tasks_root.iterdir()):
        if not child.is_dir() or child.name.startswith("_"):
            continue
        if not (child / "task.toml").is_file():
            continue
        tasks.append(load_task(child))

    spec = filter_spec.strip()
    if not spec or spec == "all":
        return tasks

    parts = [p.strip() for p in spec.split(",") if p.strip()]
    tracks = {p for p in parts if p in {t.track for t in tasks}}
    ids = {p for p in parts if p not in tracks}

    out: list[Task] = []
    for t in tasks:
        if t.track in tracks or t.id in ids:
            out.append(t)
    # if user passed unknown tokens, try prefix match on id
    if not out:
        for t in tasks:
            if any(t.id.startswith(p) or p in t.id for p in parts):
                out.append(t)
    return out
