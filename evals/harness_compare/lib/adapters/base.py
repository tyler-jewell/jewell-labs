from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from ..task import Task


@dataclass
class AdapterResult:
    status: str  # ok | skip | error
    stdout: str = ""
    stderr: str = ""
    t_ms: int = 0
    detail: str = ""
    capabilities_missing: list[str] = field(default_factory=list)
    extra: dict[str, Any] = field(default_factory=dict)


class BaseAdapter(ABC):
    name: str
    provides: set[str] = set()

    def can_run(self, task: Task) -> tuple[bool, list[str]]:
        missing = [c for c in task.capabilities if c not in self.provides]
        return (len(missing) == 0, missing)

    @abstractmethod
    def run(self, task: Task, workspace: Path, instruction: str, timeout_s: int) -> AdapterResult:
        ...


def get_adapter(name: str) -> BaseAdapter:
    name = name.strip().lower()
    if name == "hermes":
        from .hermes import HermesAdapter

        return HermesAdapter()
    if name == "jewell":
        from .jewell import JewellAdapter

        return JewellAdapter()
    if name == "dry":
        from .dry import DryAdapter

        return DryAdapter()
    raise ValueError(f"unknown harness adapter: {name}")
