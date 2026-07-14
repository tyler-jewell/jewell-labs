#!/usr/bin/env python3
"""Thin launcher for upstream llama.cpp llama-eval (not vendored here).

Use the official script from a llama.cpp checkout:

  python3 examples/llama-eval/llama-eval.py --help

Or set LLAMA_EVAL to an absolute path to that file.
This stub keeps first-party LOC slim while documenting the real entry.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path


def main() -> int:
    override = os.environ.get("LLAMA_EVAL")
    candidates = []
    if override:
        candidates.append(Path(override))
    # Common local checkout locations relative to repo root / CWD
    here = Path(__file__).resolve().parent.parent
    for base in (here, Path.cwd(), Path.home() / "src" / "llama.cpp"):
        candidates.append(base / "examples" / "llama-eval" / "llama-eval.py")
        candidates.append(base / "llama.cpp" / "examples" / "llama-eval" / "llama-eval.py")

    for path in candidates:
        if path.is_file():
            # Re-exec so argparse/sys.argv match the upstream script.
            os.execv(sys.executable, [sys.executable, str(path), *sys.argv[1:]])

    print(
        "llama-eval.py: upstream script not found.\n"
        "Clone llama.cpp and run:\n"
        "  python3 examples/llama-eval/llama-eval.py ...\n"
        "Or export LLAMA_EVAL=/path/to/llama-eval.py",
        file=sys.stderr,
    )
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
