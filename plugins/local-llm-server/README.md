# Local LLM Server

A [Hermes](https://github.com/) dashboard plugin that shows **high-level GPU
metrics for your local Ollama servers** side by side, plus a **live stream of
Ollama API calls + token rate** for any host.

It is **multi-host** and **config-driven** — point it at one or more Ollama
servers (the machine running the dashboard, and/or others reached over SSH).
The GPU backend is auto-detected per host:

| Host | GPU metrics via | Shows |
|------|-----------------|-------|
| macOS (Apple Silicon) | `ioreg` (no sudo) | GPU utilization %, unified memory |
| NVIDIA Jetson | `tegrastats` | GR3D GPU %, memory, temps, power rails |
| other NVIDIA | `nvidia-smi` | GPU %, VRAM used / total |

Loaded models + VRAM come from Ollama's `/api/ps`; the streaming log is
filtered to **API request lines** (method, path, status, latency) and
**token-rate lines** (tokens/s, decoded counts).

## Install

```sh
hermes plugins install tyler-jewell/jewell-labs/plugins/local-llm-server --enable
hermes dashboard            # open the "Local LLM Server" tab (/local-llm)
```

Installing copies `config.yaml.example` → `config.yaml` (localhost only by
default). No secrets or machine names are committed.

## Configure hosts

Edit `config.yaml` in the plugin directory. Each host describes one Ollama
server:

```yaml
hosts:
  - name: local
    kind: local
    ollama_url: http://localhost:11434
    log: { type: auto }          # auto = detect brew/file log or journalctl

  - name: jetson                 # a second server over SSH
    kind: ssh
    ssh: jetson                  # a Host alias in ~/.ssh/config (never an IP/secret)
    ollama_url: http://localhost:11434
    log: { type: journalctl, unit: ollama }
```

SSH hosts use passwordless key auth via a `~/.ssh/config` alias — the repo
never contains hostnames, IPs, or keys.

## Discover your machines

Don't know what you have? The plugin can probe for you:

- In the UI: **Scan my machines** (shown when no hosts resolve).
- API: `GET /api/plugins/local-llm-server/discover` — probes localhost and
  every `~/.ssh/config` alias for a reachable Ollama server + GPU kind, and
  returns ready-to-paste `config.yaml` entries.

## Requirements

- [Ollama](https://ollama.com) running on each host you configure.
- For SSH hosts: passwordless SSH (`ssh <alias>` works non-interactively).
- macOS GPU metrics need no sudo; Jetson needs `tegrastats` (part of L4T).

## Endpoints

`GET /servers` · `GET /stats?host=<name>` · `GET /logs?host=<name>&cursor=<c>`
· `GET /discover`
