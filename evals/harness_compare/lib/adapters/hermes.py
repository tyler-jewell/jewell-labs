from __future__ import annotations

import os
import shutil
import subprocess
import time
from pathlib import Path

from ..task import Task
from .base import AdapterResult, BaseAdapter


def _default_hermes_bin() -> str:
    """Prefer repo-vendored Hermes, then HERMES_BIN, then PATH."""
    if os.environ.get("HERMES_BIN"):
        return os.environ["HERMES_BIN"]
    # evals/harness_compare/lib/adapters → parents[2] = harness_compare
    vendor = Path(__file__).resolve().parents[2] / "vendor" / "bin" / "hermes"
    if vendor.is_file() or vendor.is_symlink():
        return str(vendor)
    return "hermes"


class HermesAdapter(BaseAdapter):
    """Drive Nous Hermes Agent CLI in the task workspace."""

    name = "hermes"
    provides = {
        "terminal",
        "write_file",
        "read_file",
        "coding",
        "closed_form",
        "multi_step",
        "agent_os",  # can shell into the monorepo if instruction says so
    }

    def __init__(self) -> None:
        self.bin = _default_hermes_bin()
        self.mode = os.environ.get("HERMES_MODE", "chat").lower()  # chat | z
        # Vendored install keeps config under vendor/hermes-home
        vendor_home = Path(__file__).resolve().parents[2] / "vendor" / "hermes-home"
        if "HERMES_HOME" not in os.environ and vendor_home.is_dir():
            os.environ["HERMES_HOME"] = str(vendor_home)

    def run(self, task: Task, workspace: Path, instruction: str, timeout_s: int) -> AdapterResult:
        bin_path = Path(self.bin)
        on_path = shutil.which(self.bin) is not None
        if not on_path and not bin_path.is_file() and not bin_path.is_symlink():
            return AdapterResult(
                status="skip",
                detail=(
                    f"`{self.bin}` not found — run "
                    "evals/harness_compare/install_and_compare.sh"
                ),
                capabilities_missing=["hermes_cli"],
            )

        # Nudge Hermes to stay inside the workspace (WildClaw-style isolation intent).
        prompt = (
            f"You are being evaluated on an isolated workspace.\n"
            f"Working directory: {workspace}\n"
            f"Only create/edit files under that directory unless the instruction says otherwise.\n"
            f"Do not read tests/ or grade scripts outside the workspace.\n\n"
            f"{instruction.strip()}\n"
        )

        if self.mode == "z":
            cmd = [self.bin, "-z", prompt]
        else:
            cmd = [self.bin, "chat", "-q", prompt]
            model = os.environ.get("HERMES_MODEL")
            if model:
                cmd.extend(["-m", model])

        env = os.environ.copy()
        # Always prefer vendored HERMES_HOME when present
        vendor_home = Path(__file__).resolve().parents[2] / "vendor" / "hermes-home"
        if vendor_home.is_dir():
            env["HERMES_HOME"] = str(vendor_home)

        t0 = time.perf_counter()
        try:
            proc = subprocess.run(
                cmd,
                cwd=str(workspace),
                capture_output=True,
                text=True,
                timeout=timeout_s,
                env=env,
            )
        except subprocess.TimeoutExpired as e:
            t_ms = int((time.perf_counter() - t0) * 1000)
            return AdapterResult(
                status="error",
                stdout=(e.stdout or "") if isinstance(e.stdout, str) else "",
                stderr=(e.stderr or "") if isinstance(e.stderr, str) else "timeout",
                t_ms=t_ms,
                detail=f"hermes timeout after {timeout_s}s",
            )
        except OSError as e:
            return AdapterResult(status="error", detail=f"hermes spawn: {e}")

        t_ms = int((time.perf_counter() - t0) * 1000)
        if proc.returncode != 0:
            return AdapterResult(
                status="error",
                stdout=proc.stdout or "",
                stderr=proc.stderr or "",
                t_ms=t_ms,
                detail=f"hermes exit {proc.returncode}",
            )
        return AdapterResult(
            status="ok",
            stdout=proc.stdout or "",
            stderr=proc.stderr or "",
            t_ms=t_ms,
            detail="hermes completed",
        )
