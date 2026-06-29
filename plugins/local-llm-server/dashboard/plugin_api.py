"""Local LLM Server — dashboard plugin backend API.

Shows high-level GPU metrics for one or more **Ollama servers** (local and/or
reached over SSH) plus a clean, incremental stream of each server's Ollama API
calls + token-rate. Hosts are declared in ``config.yaml`` (next to this
plugin); the shipped default is localhost-only, so the plugin is fully
host/codebase-agnostic — no machine names, IPs, or secrets are baked in.

GPU backend is auto-detected per host:
  * macOS         -> Apple Metal via ``ioreg`` (no sudo)
  * NVIDIA Jetson -> ``tegrastats`` (GR3D GPU%, temps, power rails)
  * other NVIDIA  -> ``nvidia-smi``

Mounted by the Hermes dashboard plugin system at
``/api/plugins/local-llm-server/``. No Hermes core files are touched.
"""

import os
import re
import time
import json
import subprocess
from pathlib import Path

from fastapi import APIRouter

try:
    import yaml  # provided by the Hermes runtime (used for plugin.yaml too)
except Exception:  # pragma: no cover
    yaml = None

router = APIRouter()

PLUGIN_ROOT = Path(__file__).resolve().parent.parent
CONFIG_PATH = PLUGIN_ROOT / "config.yaml"

DEFAULT_CONFIG = {
    "hosts": [
        {
            "name": "local",
            "kind": "local",
            "ollama_url": "http://localhost:11434",
            "log": {"type": "auto"},
        }
    ]
}

SSH_OPTS = [
    "-o", "BatchMode=yes",
    "-o", "ConnectTimeout=6",
    "-o", "StrictHostKeyChecking=accept-new",
]

_CONFIG_CACHE = {"data": None, "mtime": 0.0}


# --------------------------------------------------------------------------- #
# config + command execution
# --------------------------------------------------------------------------- #
def _load_config() -> dict:
    """Read config.yaml (cached on mtime); fall back to localhost-only."""
    try:
        st = CONFIG_PATH.stat()
    except OSError:
        return DEFAULT_CONFIG
    if _CONFIG_CACHE["data"] is not None and _CONFIG_CACHE["mtime"] == st.st_mtime:
        return _CONFIG_CACHE["data"]
    data = DEFAULT_CONFIG
    if yaml is not None:
        try:
            parsed = yaml.safe_load(CONFIG_PATH.read_text(encoding="utf-8")) or {}
            if isinstance(parsed, dict) and parsed.get("hosts"):
                data = parsed
        except Exception:
            data = DEFAULT_CONFIG
    _CONFIG_CACHE["data"] = data
    _CONFIG_CACHE["mtime"] = st.st_mtime
    return data


def _hosts() -> list:
    return _load_config().get("hosts", [])


def _find_host(name: str):
    for h in _hosts():
        if h.get("name") == name:
            return h
    return None


def _run(host: dict, cmd: str, timeout: int = 12):
    """Run a shell command for *host*; return (ok, stdout, err).

    kind=local -> run on this machine; kind=ssh -> run via the host's
    ~/.ssh/config alias.
    """
    if host.get("kind") == "ssh":
        alias = host.get("ssh") or host.get("name")
        argv = ["ssh"] + SSH_OPTS + [alias, cmd]
    else:
        argv = ["bash", "-lc", cmd]
    try:
        p = subprocess.run(argv, capture_output=True, text=True, timeout=timeout)
        if p.returncode != 0:
            return False, p.stdout or "", (p.stderr or f"exit {p.returncode}").strip()
        return True, p.stdout, ""
    except subprocess.TimeoutExpired:
        return False, "", "timeout"
    except Exception as e:  # pragma: no cover
        return False, "", str(e)


# --------------------------------------------------------------------------- #
# GPU + ollama snapshot
# --------------------------------------------------------------------------- #
def _stats_snippet(ollama_url: str) -> str:
    """One shell snippet that emits labelled, line-based metrics for a host."""
    u = ollama_url.rstrip("/")
    return (
        'echo "__OS__$(uname -s)"; '
        'if [ -e /etc/nv_tegra_release ] || grep -qiE "orin|tegra|jetson" /proc/device-tree/model 2>/dev/null; then echo "__TEGRA__1"; fi; '
        'if command -v nvidia-smi >/dev/null 2>&1; then echo "__NVSMI__$(nvidia-smi --query-gpu=utilization.gpu,memory.used,memory.total --format=csv,noheader,nounits 2>/dev/null | head -1)"; fi; '
        'if [ "$(uname -s)" = "Darwin" ]; then ioreg -r -c IOAccelerator -d 1 2>/dev/null | tr "," "\\n" | grep -E "Device Utilization %|In use system memory|Alloc system memory"; echo "__MEMSIZE__$(sysctl -n hw.memsize 2>/dev/null)"; fi; '
        'if [ -e /etc/nv_tegra_release ]; then echo "__TEGRASTATS__$(timeout 2 tegrastats --interval 1000 2>/dev/null | head -1)"; fi; '
        'echo "__OLLAMAVER__$(curl -s --max-time 3 ' + u + '/api/version)"; '
        'echo "__OLLAMAPS__$(curl -s --max-time 3 ' + u + '/api/ps)"'
    )


def _parse_tegrastats(line: str) -> dict:
    out = {}
    m = re.search(r"RAM (\d+)/(\d+)MB", line)
    if m:
        out["ram"] = {"used_mb": int(m.group(1)), "total_mb": int(m.group(2))}
    m = re.search(r"GR3D_FREQ (\d+)%", line)
    if m:
        out["gpu_pct"] = int(m.group(1))
    out["temps"] = {n: float(v) for n, v in re.findall(r"(\w+)@([\d.]+)C", line)}
    power = {}
    total = 0
    for n, inst, _avg in re.findall(r"(\S+)\s+(\d+)mW/(\d+)mW", line):
        power[n] = int(inst)
        total += int(inst)
    if power:
        out["power"] = power
        out["power_total_mw"] = total
    return out


def _parse_stats(raw: str) -> dict:
    lines = raw.splitlines()
    os_name = ""
    tegra = False
    nvsmi = None
    ioreg = {}
    memsize = None
    tegrastats_line = None
    ollama_ver = ""
    ollama_ps = ""
    for ln in lines:
        if ln.startswith("__OS__"):
            os_name = ln[6:].strip()
        elif ln.startswith("__TEGRA__"):
            tegra = True
        elif ln.startswith("__NVSMI__"):
            nvsmi = ln[len("__NVSMI__"):].strip()
        elif ln.startswith("__MEMSIZE__"):
            try:
                memsize = int(ln[len("__MEMSIZE__"):].strip())
            except ValueError:
                pass
        elif ln.startswith("__TEGRASTATS__"):
            tegrastats_line = ln[len("__TEGRASTATS__"):].strip()
        elif ln.startswith("__OLLAMAVER__"):
            ollama_ver = ln[len("__OLLAMAVER__"):].strip()
        elif ln.startswith("__OLLAMAPS__"):
            ollama_ps = ln[len("__OLLAMAPS__"):].strip()
        elif '"Device Utilization %"=' in ln:
            m = re.search(r'"Device Utilization %"=(\d+)', ln)
            if m:
                ioreg["util"] = int(m.group(1))
        elif '"In use system memory"=' in ln:
            m = re.search(r'"In use system memory"=(\d+)', ln)
            if m:
                ioreg["used"] = int(m.group(1))

    gpu = {}
    kind = "cpu"
    if os_name == "Darwin":
        kind = "apple-metal"
        gpu["util_pct"] = ioreg.get("util")
        if ioreg.get("used") is not None:
            gpu["mem_used_gb"] = round(ioreg["used"] / 1e9, 2)
        if memsize:
            gpu["mem_total_gb"] = round(memsize / 1e9, 1)
        gpu["backend"] = "Apple Metal"
    elif tegra and tegrastats_line:
        kind = "jetson-cuda"
        t = _parse_tegrastats(tegrastats_line)
        gpu["util_pct"] = t.get("gpu_pct")
        ram = t.get("ram") or {}
        if ram:
            gpu["mem_used_gb"] = round(ram["used_mb"] / 1024, 2)
            gpu["mem_total_gb"] = round(ram["total_mb"] / 1024, 1)
        if t.get("temps"):
            gpu["temps"] = t["temps"]
        if t.get("power_total_mw") is not None:
            gpu["power_w"] = round(t["power_total_mw"] / 1000, 1)
        gpu["backend"] = "Jetson CUDA"
    elif nvsmi:
        kind = "nvidia-cuda"
        parts = [p.strip() for p in nvsmi.split(",")]
        try:
            gpu["util_pct"] = int(float(parts[0]))
            gpu["mem_used_gb"] = round(float(parts[1]) / 1024, 2)
            gpu["mem_total_gb"] = round(float(parts[2]) / 1024, 1)
        except (ValueError, IndexError):
            pass
        gpu["backend"] = "NVIDIA CUDA"

    ollama = {"running": False, "models": []}
    if ollama_ver.startswith("{"):
        try:
            ollama["version"] = json.loads(ollama_ver).get("version")
            ollama["running"] = True
        except ValueError:
            pass
    if ollama_ps.startswith("{"):
        try:
            for m in json.loads(ollama_ps).get("models", []):
                det = m.get("details") or {}
                ollama["models"].append({
                    "name": m.get("name"),
                    "size_vram_gb": round((m.get("size_vram") or m.get("size") or 0) / 1e9, 2),
                    "context_length": m.get("context_length"),
                    "quant": det.get("quantization_level"),
                    "param_size": det.get("parameter_size"),
                })
        except ValueError:
            pass

    return {"os": os_name, "kind": kind, "gpu": gpu, "ollama": ollama}


@router.get("/servers")
async def servers():
    """Configured hosts with reachability + ollama-running flags."""
    out = []
    for h in _hosts():
        ok, raw, err = _run(h, _stats_snippet(h.get("ollama_url", "http://localhost:11434")), timeout=12)
        entry = {"name": h.get("name"), "kind_cfg": h.get("kind", "local"), "reachable": ok}
        if ok:
            parsed = _parse_stats(raw)
            entry.update({
                "kind": parsed["kind"],
                "ollama_running": parsed["ollama"]["running"],
                "backend": parsed["gpu"].get("backend"),
            })
        else:
            entry["error"] = err
        out.append(entry)
    return {"hosts": out, "ts": int(time.time())}


@router.get("/stats")
async def stats(host: str = "local"):
    """GPU + Ollama snapshot for one host."""
    h = _find_host(host)
    if h is None:
        return {"online": False, "error": f"unknown host '{host}'", "host": host}
    ok, raw, err = _run(h, _stats_snippet(h.get("ollama_url", "http://localhost:11434")), timeout=12)
    if not ok:
        return {"online": False, "error": err, "host": host, "ts": int(time.time())}
    data = _parse_stats(raw)
    data.update({"online": True, "host": host, "ts": int(time.time())})
    return data


# --------------------------------------------------------------------------- #
# streaming log — API calls + token rate
# --------------------------------------------------------------------------- #
_GIN_RE = re.compile(
    r'\[GIN\][^|]*\|\s*(\d{3})\s*\|\s*([0-9.]+[^\s|]*)\s*\|[^|]*\|\s*(\w+)\s+"([^"]+)"'
)
_PATH_RE = re.compile(r'"(/api/[^"]*)"')
_TG_RE = re.compile(r'\btg\s*=\s*([\d.]+)\s*t/s')
_ND_RE = re.compile(r'\bn_decoded\s*=\s*(\d+)')


def _parse_log_line(line: str):
    """Keep only API-call + token-rate lines; return a structured dict or None."""
    if "[GIN]" in line or '"/api/' in line:
        m = _GIN_RE.search(line)
        if m:
            return {
                "type": "api",
                "status": int(m.group(1)),
                "latency": m.group(2),
                "method": m.group(3),
                "path": m.group(4),
                "raw": line.rstrip(),
            }
        pm = _PATH_RE.search(line)
        if pm:
            return {"type": "api", "path": pm.group(1), "raw": line.rstrip()}
        return None
    if "t/s" in line and "tg" in line:
        tg = _TG_RE.search(line)
        nd = _ND_RE.search(line)
        if tg:
            d = {"type": "token", "tps": float(tg.group(1)), "raw": line.rstrip()}
            if nd:
                d["n_decoded"] = int(nd.group(1))
            return d
    return None


def _resolve_log(host: dict):
    """Return ('file', path) or ('journalctl', unit) for a host's log."""
    log = host.get("log") or {}
    ltype = log.get("type", "auto")
    if ltype == "file" and log.get("path"):
        return ("file", log["path"])
    if ltype == "journalctl" and log.get("unit"):
        return ("journalctl", log["unit"])
    # auto-detect
    ok, raw, _ = _run(host, (
        'for f in /opt/homebrew/var/log/ollama.log "$HOME/.ollama/logs/server.log" '
        '/var/log/ollama.log; do [ -f "$f" ] && echo "FILE=$f" && break; done; '
        'systemctl is-active ollama >/dev/null 2>&1 && echo "UNIT=ollama"; true'
    ), timeout=8)
    path = unit = None
    if ok:
        for ln in raw.splitlines():
            if ln.startswith("FILE="):
                path = ln[5:].strip()
            elif ln.startswith("UNIT="):
                unit = ln[5:].strip()
    if path:
        return ("file", path)
    if unit:
        return ("journalctl", unit)
    return (None, None)


@router.get("/logs")
async def logs(host: str = "local", cursor: str = ""):
    """Incremental Ollama log tail (API calls + token rate) since *cursor*."""
    h = _find_host(host)
    if h is None:
        return {"error": f"unknown host '{host}'", "host": host, "lines": [], "cursor": ""}
    mode, ref = _resolve_log(h)
    if mode is None:
        return {"error": "no ollama log found", "host": host, "lines": [], "cursor": ""}

    if mode == "file":
        f = ref.replace('"', '\\"')
        if cursor.isdigit():
            cmd = (
                f'f="{f}"; sz=$(wc -c < "$f" 2>/dev/null || echo 0); start={cursor}; '
                f'if [ "$start" -gt "$sz" ]; then start=0; fi; '
                f'tail -c +$((start+1)) "$f" 2>/dev/null; echo "__SIZE__$sz"'
            )
        else:
            # first call: backfill the tail end only
            cmd = (
                f'f="{f}"; sz=$(wc -c < "$f" 2>/dev/null || echo 0); '
                f'tail -c 20000 "$f" 2>/dev/null; echo "__SIZE__$sz"'
            )
        ok, raw, err = _run(h, cmd, timeout=10)
        if not ok:
            return {"error": err, "host": host, "lines": [], "cursor": cursor}
        new_cursor = cursor
        body = []
        for ln in raw.splitlines():
            if ln.startswith("__SIZE__"):
                new_cursor = ln[len("__SIZE__"):].strip()
            else:
                body.append(ln)
        parsed = [p for p in (_parse_log_line(x) for x in body) if p]
        return {"host": host, "source": ref, "mode": "file",
                "lines": parsed[-300:], "cursor": new_cursor}

    # journalctl
    if cursor:
        c = cursor.replace('"', '')
        cmd = f'journalctl -u {ref} -o cat --after-cursor "{c}" --show-cursor --no-pager 2>/dev/null'
    else:
        cmd = f'journalctl -u {ref} -o cat -n 120 --show-cursor --no-pager 2>/dev/null'
    ok, raw, err = _run(h, cmd, timeout=10)
    if not ok:
        return {"error": err, "host": host, "lines": [], "cursor": cursor}
    new_cursor = cursor
    body = []
    for ln in raw.splitlines():
        s = ln.strip()
        if s.startswith("-- cursor:"):
            new_cursor = s[len("-- cursor:"):].strip()
        elif s.startswith("--") and "cursor" in s:
            continue
        else:
            body.append(ln)
    parsed = [p for p in (_parse_log_line(x) for x in body) if p]
    return {"host": host, "source": f"journalctl:{ref}", "mode": "journalctl",
            "lines": parsed[-300:], "cursor": new_cursor}


# --------------------------------------------------------------------------- #
# discovery — "what computers do you have?"
# --------------------------------------------------------------------------- #
_DISCOVER_SNIPPET = (
    'echo "__OS__$(uname -s)"; '
    'command -v nvidia-smi >/dev/null 2>&1 && echo "__NV__1"; '
    '{ [ -e /etc/nv_tegra_release ] || grep -qiE "orin|tegra|jetson" /proc/device-tree/model 2>/dev/null; } && echo "__TEGRA__1"; '
    'echo "__OLV__$(curl -s --max-time 3 http://localhost:11434/api/version)"'
)


def _parse_ssh_aliases() -> list:
    cfg = Path.home() / ".ssh" / "config"
    aliases = []
    try:
        for ln in cfg.read_text(encoding="utf-8").splitlines():
            s = ln.strip()
            if s.lower().startswith("host ") and "*" not in s and "?" not in s:
                for tok in s.split()[1:]:
                    if tok and tok not in aliases:
                        aliases.append(tok)
    except OSError:
        pass
    return aliases


def _interpret_discover(raw: str) -> dict:
    os_name = ""
    nv = tegra = False
    ollama = False
    for ln in raw.splitlines():
        if ln.startswith("__OS__"):
            os_name = ln[6:].strip()
        elif ln.startswith("__NV__"):
            nv = True
        elif ln.startswith("__TEGRA__"):
            tegra = True
        elif ln.startswith("__OLV__"):
            ollama = ln[len("__OLV__"):].strip().startswith("{")
    if os_name == "Darwin":
        kind = "apple-metal"
    elif tegra:
        kind = "jetson-cuda"
    elif nv:
        kind = "nvidia-cuda"
    elif os_name:
        kind = "cpu"
    else:
        kind = "unknown"
    return {"os": os_name, "gpu_kind": kind, "ollama": ollama}


@router.get("/discover")
async def discover():
    """Probe localhost + every ~/.ssh/config alias for an Ollama server + GPU."""
    found = []

    # localhost
    ok, raw, _ = _run({"kind": "local"}, _DISCOVER_SNIPPET, timeout=8)
    if ok:
        info = _interpret_discover(raw)
        found.append({
            "name": "local", "kind": "local", "reachable": True,
            "ollama": info["ollama"], "gpu_kind": info["gpu_kind"], "os": info["os"],
            "suggested_config": {
                "name": "local", "kind": "local",
                "ollama_url": "http://localhost:11434", "log": {"type": "auto"},
            },
        })

    # ssh aliases
    for alias in _parse_ssh_aliases():
        ok, raw, err = _run({"kind": "ssh", "ssh": alias}, _DISCOVER_SNIPPET, timeout=7)
        if not ok:
            found.append({"name": alias, "kind": "ssh", "reachable": False, "error": err})
            continue
        info = _interpret_discover(raw)
        log = {"type": "journalctl", "unit": "ollama"} if info["os"] != "Darwin" else {"type": "auto"}
        found.append({
            "name": alias, "kind": "ssh", "reachable": True,
            "ollama": info["ollama"], "gpu_kind": info["gpu_kind"], "os": info["os"],
            "suggested_config": {
                "name": alias, "kind": "ssh", "ssh": alias,
                "ollama_url": "http://localhost:11434", "log": log,
            },
        })

    return {"hosts": found, "configured": [h.get("name") for h in _hosts()], "ts": int(time.time())}
