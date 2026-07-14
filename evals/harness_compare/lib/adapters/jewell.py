from __future__ import annotations

import json
import os
import subprocess
import time
import urllib.error
import urllib.request
from pathlib import Path

from ..task import Task
from .base import AdapterResult, BaseAdapter

# Monorepo root: evals/harness_compare/lib/adapters/this → parents[4]
REPO_ROOT = Path(__file__).resolve().parents[4]
RUST_AGENT = REPO_ROOT / "rust-agent"


class JewellAdapter(BaseAdapter):
    """Drive jewell-labs rust-agent (host evals + chat stream)."""

    name = "jewell"
    provides = {
        "host_eval",
        "agent_os",
        "closed_form",
        "multi_step",
        # coding/terminal intentionally NOT claimed — honest capability surface
    }

    def __init__(self) -> None:
        self.base = os.environ.get("JEWELL_BASE", "http://127.0.0.1:3000").rstrip("/")
        # API expects category/name (see load_agent), not bare stem.
        self.agent = os.environ.get("JEWELL_AGENT", "core/orchestrator")
        self.llama = os.environ.get("COMPARE_LLAMA", "http://127.0.0.1:8080").rstrip("/")

    def run(self, task: Task, workspace: Path, instruction: str, timeout_s: int) -> AdapterResult:
        ok, missing = self.can_run(task)
        if not ok:
            return AdapterResult(
                status="skip",
                detail=f"jewell lacks capabilities: {missing}",
                capabilities_missing=missing,
            )

        if "host_eval" in task.capabilities or task.track == "agent_os" and task.id.startswith(
            "host_"
        ):
            return self._run_host_eval(task, timeout_s)

        if task.track == "closed_form" and os.environ.get("JEWELL_CLOSED_FORM", "chat") == "direct":
            return self._run_llama_direct(instruction, timeout_s)

        return self._run_chat(task, workspace, instruction, timeout_s)

    def _run_host_eval(self, task: Task, timeout_s: int) -> AdapterResult:
        """Structural gates without requiring a live chat model."""
        mode = (task.path / "workspace" / "host_mode.txt").read_text(encoding="utf-8").strip() if (
            task.path / "workspace" / "host_mode.txt"
        ).is_file() else "tool_plan"

        t0 = time.perf_counter()
        if mode == "tool_plan":
            cmd = ["cargo", "run", "-q", "--bin", "eval_introspection"]
            try:
                proc = subprocess.run(
                    cmd,
                    cwd=str(RUST_AGENT),
                    capture_output=True,
                    text=True,
                    timeout=timeout_s,
                )
            except (subprocess.TimeoutExpired, OSError) as e:
                return AdapterResult(status="error", detail=f"host eval: {e}")
            t_ms = int((time.perf_counter() - t0) * 1000)
            # Write a marker file the grader can check; also return stdout for debug.
            return AdapterResult(
                status="ok" if proc.returncode == 0 else "error",
                stdout=proc.stdout or "",
                stderr=proc.stderr or "",
                t_ms=t_ms,
                detail="eval_introspection",
                extra={"exit_code": proc.returncode, "host_mode": mode},
            )

        if mode == "team":
            # Re-use unit test as host gate
            cmd = [
                "cargo",
                "test",
                "-q",
                "team_collaboration_green",
                "--",
                "--nocapture",
            ]
            try:
                proc = subprocess.run(
                    cmd,
                    cwd=str(RUST_AGENT),
                    capture_output=True,
                    text=True,
                    timeout=timeout_s,
                )
            except (subprocess.TimeoutExpired, OSError) as e:
                return AdapterResult(status="error", detail=f"team host eval: {e}")
            t_ms = int((time.perf_counter() - t0) * 1000)
            return AdapterResult(
                status="ok" if proc.returncode == 0 else "error",
                stdout=proc.stdout or "",
                stderr=proc.stderr or "",
                t_ms=t_ms,
                detail="team_collaboration_green",
                extra={"exit_code": proc.returncode, "host_mode": mode},
            )

        return AdapterResult(status="error", detail=f"unknown host_mode {mode}")

    def _run_chat(
        self, task: Task, workspace: Path, instruction: str, timeout_s: int
    ) -> AdapterResult:
        prompt = (
            f"Evaluation workspace directory: {workspace}\n"
            f"Task track: {task.track}\n"
            f"Use your available tools. Prefer concise final answers.\n"
            f"If the task asks for files, explain what you would write "
            f"(you may lack arbitrary FS write tools).\n\n"
            f"{instruction.strip()}\n"
        )
        body = json.dumps(
            {
                "agent_stem": self.agent,
                "message": prompt,
                "history": [],
            }
        ).encode("utf-8")
        req = urllib.request.Request(
            f"{self.base}/api/chat/stream",
            data=body,
            headers={"Content-Type": "application/json", "Accept": "text/event-stream"},
            method="POST",
        )
        t0 = time.perf_counter()
        chunks: list[str] = []
        stream_err = ""
        try:
            with urllib.request.urlopen(req, timeout=timeout_s) as resp:
                # SSE: data: {...}\n\n  (token deltas use {"delta": "..."})
                buf = ""
                done = False
                while not done:
                    piece = resp.read(4096)
                    if not piece:
                        break
                    buf += piece.decode("utf-8", errors="replace")
                    while "\n" in buf:
                        line, buf = buf.split("\n", 1)
                        line = line.strip()
                        if not line.startswith("data:"):
                            continue
                        data = line[5:].strip()
                        if data == "[DONE]":
                            done = True
                            break
                        try:
                            obj = json.loads(data)
                        except json.JSONDecodeError:
                            chunks.append(data)
                            continue
                        if "error" in obj:
                            # Keep partial transcript — context overflow mid-run is common
                            # on small llama-server ctx; graders may still pass.
                            stream_err = str(obj["error"])
                            done = True
                            break
                        for k in ("text", "content", "delta", "message", "token"):
                            if k in obj and isinstance(obj[k], str):
                                chunks.append(obj[k])
                        if obj.get("type") == "assistant" and isinstance(obj.get("content"), str):
                            chunks.append(obj["content"])
        except urllib.error.HTTPError as e:
            t_ms = int((time.perf_counter() - t0) * 1000)
            err = e.read().decode("utf-8", errors="replace") if e.fp else str(e)
            return AdapterResult(
                status="error",
                stderr=err,
                t_ms=t_ms,
                detail=f"HTTP {e.code} from {self.base} — is rust-agent running?",
            )
        except urllib.error.URLError as e:
            return AdapterResult(
                status="error",
                detail=f"cannot reach jewell at {self.base}: {e.reason}",
            )
        except TimeoutError:
            return AdapterResult(status="error", detail="chat timeout")

        t_ms = int((time.perf_counter() - t0) * 1000)
        text = "".join(chunks)
        if not text.strip():
            return AdapterResult(
                status="error",
                stderr=stream_err,
                t_ms=t_ms,
                detail="chat error (empty transcript)",
            )

        if task.track == "closed_form":
            (workspace / "agent_answer.txt").write_text(text, encoding="utf-8")
        else:
            (workspace / "agent_transcript.txt").write_text(text, encoding="utf-8")

        detail = "jewell chat stream completed"
        if stream_err:
            detail = f"partial stream (server error after tokens): {stream_err[:120]}"
        return AdapterResult(
            status="ok",
            stdout=text,
            stderr=stream_err,
            t_ms=t_ms,
            detail=detail,
        )

    def _run_llama_direct(self, instruction: str, timeout_s: int) -> AdapterResult:
        """Optional closed_form path: raw llama-server (no agent tools)."""
        body = json.dumps(
            {
                "model": "local",
                "temperature": 0,
                "messages": [{"role": "user", "content": instruction}],
            }
        ).encode("utf-8")
        req = urllib.request.Request(
            f"{self.llama}/v1/chat/completions",
            data=body,
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        t0 = time.perf_counter()
        try:
            with urllib.request.urlopen(req, timeout=timeout_s) as resp:
                payload = json.loads(resp.read().decode("utf-8"))
        except Exception as e:
            return AdapterResult(status="error", detail=f"llama direct: {e}")
        t_ms = int((time.perf_counter() - t0) * 1000)
        try:
            text = payload["choices"][0]["message"]["content"]
        except (KeyError, IndexError, TypeError):
            return AdapterResult(
                status="error",
                stdout=json.dumps(payload)[:2000],
                t_ms=t_ms,
                detail="bad completion shape",
            )
        return AdapterResult(status="ok", stdout=text, t_ms=t_ms, detail="llama direct")
