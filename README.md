# jewell-labs

Installable [Hermes](https://github.com/) assets — dashboard **plugins** and
**skills** — that you can pull directly with the `hermes` CLI. Each asset lives
in a typed top-level directory and installs by subpath.

## Plugins

Hermes dashboard plugins (UI tab + backend API). Install any by subpath:

```sh
hermes plugins install tyler-jewell/jewell-labs/plugins/<name> --enable
hermes dashboard
```

| Plugin | Tab | What it does |
|--------|-----|--------------|
| [`local-llm-server`](plugins/local-llm-server) | Local LLM Server | GPU metrics for your local Ollama servers (Apple Metal / NVIDIA / Jetson) + live Ollama API-call & token-rate stream. Multi-host, config-driven, with host discovery. |
| [`benchmark-results`](plugins/benchmark-results) | Benchmarks | Runs lm-evaluation-harness against local Ollama models and charts pass@1 / scores. |

```sh
hermes plugins install tyler-jewell/jewell-labs/plugins/local-llm-server --enable
hermes plugins install tyler-jewell/jewell-labs/plugins/benchmark-results --enable
hermes plugins list
```

Plugins are **host- and codebase-agnostic** — no machine names, IPs, or secrets
are committed. Host-specific settings live in a per-plugin `config.yaml`
(created from `config.yaml.example` on install) which is gitignored.

## Skills

Hermes skills (see [`skills/`](skills)):

```sh
hermes skills tap add tyler-jewell/jewell-labs          # browse/search all skills here
hermes skills install tyler-jewell/jewell-labs/skills/<name>
```

## Layout

```
jewell-labs/
  plugins/<name>/   # hermes plugins install tyler-jewell/jewell-labs/plugins/<name>
  skills/<name>/    # hermes skills install  tyler-jewell/jewell-labs/skills/<name>
```

## License

MIT
