#!/usr/bin/env python3
"""Thin wrapper: full multi-variant profile delegates to profile_run_agent.sh.

Keeps first-party LOC under the project line budget while preserving the entry
path used by docs/scripts.
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    script = root / "tools" / "profile_run_agent.sh"
    if not script.is_file():
        print(f"missing {script}", file=sys.stderr)
        return 2
    env = os.environ.copy()
    # Optional: forward --out / concurrency knobs via env for the bash harness.
    if len(sys.argv) > 1:
        env["PROFILE_EXTRA_ARGS"] = " ".join(sys.argv[1:])
    return subprocess.call(["bash", str(script)], cwd=str(root), env=env)


if __name__ == "__main__":
    raise SystemExit(main())
