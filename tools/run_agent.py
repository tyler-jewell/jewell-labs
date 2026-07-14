#!/usr/bin/env python3
"""Minimal agent runner: agents/{name}.md + models/registry.yaml → llama-server chat."""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Dict, Optional, Tuple

ROOT = Path(__file__).resolve().parents[1]
AGENTS = ROOT / "agents"
REGISTRY = ROOT / "models" / "registry.yaml"


def parse_frontmatter(text: str) -> Tuple[Dict[str, Any], str]:
    if not text.startswith("---"):
        return {}, text
    parts = text.split("---", 2)
    if len(parts) < 3:
        return {}, text
    raw_fm, body = parts[1], parts[2].lstrip("\n")
    # minimal YAML subset: key: value, nested via indentation (1 level of maps)
    data: Dict[str, Any] = {}
    stack: list[tuple[int, Dict[str, Any]]] = [(0, data)]
    for line in raw_fm.splitlines():
        if not line.strip() or line.strip().startswith("#"):
            continue
        indent = len(line) - len(line.lstrip(" "))
        m = re.match(r"^(\s*)([A-Za-z0-9_]+):\s*(.*)$", line)
        if not m:
            continue
        key, val = m.group(2), m.group(3).strip()
        while len(stack) > 1 and indent <= stack[-1][0]:
            stack.pop()
        cur = stack[-1][1]
        if val == "" or val is None:
            cur[key] = {}
            stack.append((indent, cur[key]))
        else:
            if (val.startswith('"') and val.endswith('"')) or (
                val.startswith("'") and val.endswith("'")
            ):
                val = val[1:-1]
            elif val.lower() in ("true", "false"):
                val = val.lower() == "true"
            elif re.fullmatch(r"-?\d+", val):
                val = int(val)
            elif re.fullmatch(r"-?\d+\.\d+", val):
                val = float(val)
            elif val.lower() == "null":
                val = None
            cur[key] = val
    return data, body


def load_registry() -> Dict[str, Any]:
    text = REGISTRY.read_text()
    # very small parser for our registry shape
    models: Dict[str, Any] = {}
    cur: Optional[str] = None
    in_defaults = False
    for line in text.splitlines():
        if re.match(r"^models:\s*$", line):
            continue
        m = re.match(r"^  ([A-Za-z0-9_.-]+):\s*$", line)
        if m:
            cur = m.group(1)
            models[cur] = {}
            in_defaults = False
            continue
        if cur is None:
            continue
        if re.match(r"^    defaults:\s*$", line):
            models[cur]["defaults"] = {}
            in_defaults = True
            continue
        m = re.match(r"^    ([A-Za-z0-9_]+):\s*(.+)$", line)
        if m and not in_defaults:
            models[cur][m.group(1)] = m.group(2).strip().strip('"').strip("'")
            continue
        m = re.match(r"^      ([A-Za-z0-9_]+):\s*(.+)$", line)
        if m and in_defaults:
            v = m.group(2).strip().strip('"').strip("'")
            if re.fullmatch(r"-?\d+", v):
                v = int(v)
            models[cur].setdefault("defaults", {})[m.group(1)] = v
    return models


def resolve_model(key: str, registry: Dict[str, Any]) -> Dict[str, Any]:
    if key in registry:
        spec = dict(registry[key])
        spec["key"] = key
        path = os.path.expanduser(spec.get("path", ""))
        spec["path"] = path
        return spec
    path = os.path.expanduser(key)
    if path.endswith(".gguf") and Path(path).exists():
        return {"key": key, "path": path, "alias": Path(path).stem, "defaults": {}}
    raise SystemExit(f"Unknown model '{key}' (not in registry and not a .gguf path)")


def server_up(base: str) -> bool:
    try:
        with urllib.request.urlopen(base + "/v1/models", timeout=1) as r:
            return r.status == 200
    except Exception:
        return False


def ensure_server(path: str, host: str, port: int, ctx: int, reasoning: str) -> str:
    base = f"http://{host}:{port}"
    if server_up(base):
        return base
    cmd = [
        "llama-server",
        "-m",
        path,
        "--host",
        host,
        "--port",
        str(port),
        "-c",
        str(ctx),
        "-np",
        "1",
        "--ctx-checkpoints",
        "0",
        "--reasoning",
        reasoning,
    ]
    subprocess.Popen(
        cmd,
        stdout=open("/tmp/run-agent-llama.log", "w"),
        stderr=subprocess.STDOUT,
        start_new_session=True,
    )
    for _ in range(60):
        if server_up(base):
            return base
        time.sleep(0.5)
    raise SystemExit("llama-server failed to become ready")


def chat(base: str, model: str, system: str, user: str, sampling: Dict[str, Any]) -> str:
    body = {
        "model": model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user},
        ],
        "temperature": sampling.get("temperature", 0),
        "max_tokens": sampling.get("max_tokens", 256),
    }
    if sampling.get("seed") is not None:
        body["seed"] = sampling["seed"]
    req = urllib.request.Request(
        base + "/v1/chat/completions",
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=120) as r:
        data = json.loads(r.read().decode())
    msg = data["choices"][0]["message"]
    content = msg.get("content") or ""
    reasoning = msg.get("reasoning_content") or ""
    return content if content else reasoning


def main() -> None:
    ap = argparse.ArgumentParser(description="Run agents/{name}.md against llama.cpp")
    ap.add_argument("agent", help="agent name (agents/{name}.md)")
    ap.add_argument("prompt", nargs="?", default="What is 2+2? Put answer in \\boxed{}.")
    ap.add_argument("--model", help="override default_model")
    ap.add_argument("--dry-parse", action="store_true", help="only parse+resolve, no server/chat")
    ap.add_argument("--serve-only", action="store_true", help="ensure server then exit")
    args = ap.parse_args()

    agent_path = AGENTS / f"{args.agent}.md"
    if not agent_path.exists():
        raise SystemExit(f"missing {agent_path}")
    fm, body = parse_frontmatter(agent_path.read_text())
    model_key = args.model or fm.get("model") or fm.get("default_model")
    if not model_key:
        raise SystemExit("agent frontmatter missing default_model")
    registry = load_registry()
    spec = resolve_model(str(model_key), registry)
    server = fm.get("server") or {}
    sampling = fm.get("sampling") or {}
    host = server.get("host", "127.0.0.1")
    port = int(server.get("port", 8080))
    ctx = int(server.get("ctx", (spec.get("defaults") or {}).get("ctx", 2048)))
    reasoning = str(server.get("reasoning", (spec.get("defaults") or {}).get("reasoning", "off")))

    if args.dry_parse:
        print(
            json.dumps(
                {
                    "agent": args.agent,
                    "model_key": model_key,
                    "path": spec["path"],
                    "system_chars": len(body),
                    "port": port,
                }
            )
        )
        return

    base = ensure_server(spec["path"], host, port, ctx, reasoning)
    if args.serve_only:
        print(base)
        return

    out = chat(base, spec.get("alias") or "local", body, args.prompt, sampling)
    print(out)


if __name__ == "__main__":
    main()
