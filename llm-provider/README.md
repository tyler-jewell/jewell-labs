# llm-provider

One OpenAI-compatible API on this Mac unifying **local llama.cpp/ollama + Grok (xAI) +
Claude (Anthropic)** behind a single endpoint on `:4141`. Local tools call it over loopback
with no auth; remote callers authenticate with an OAuth-minted key gated to an email
allowlist. Rust (axum + reqwest), single static binary.

## Why it exists

Claude and Grok are reached through their **OAuth subscription tokens** (Claude Code's
Keychain token; the grok CLI's OIDC token) — not metered API keys — so this box serves those
models at subscription cost. Local models run on llama.cpp/ollama. Everything speaks the
OpenAI wire format, so any OpenAI-compatible client (Paperclip, SDKs) just points at
`http://localhost:4141/v1`.

## Architecture

```
client ──▶ :4141 /v1/*  ──▶ auth middleware (loopback OK | else minted key)
                              └─▶ Registry.route(model) ──▶ Provider
                                     claude* ─▶ ClaudeProvider   (OpenAI⇄Anthropic translate)
                                     grok*   ─▶ PassthroughProvider (xAI, OIDC bearer)
                                     else    ─▶ PassthroughProvider (llama.cpp/ollama)
```

- `src/registry.rs` — the `Provider` trait + registry. **Adding a provider = one module + one
  arm** (see the `add-provider` skill).
- `src/providers/` — `passthrough.rs` (local + grok share it), `claude.rs` (the only translator).
- `src/token/` — `TokenSource`: Keychain (Claude) / OIDC (grok) refresh, each with its own cache.
- `src/server/` — router + the single loopback-vs-OAuth `auth_mw`.
- `src/identity/` — keys.json, allowlist, Google `/auth/google` + browser `/login`, `login-xai`.
- `config.toml` — the one config: providers, allowlist, ports, and tuned local runtime profiles.

## Run

```sh
cargo build --release
./target/release/llm-provider            # serves 0.0.0.0:4141 (loopback trusted)
```

Deploy as a launchd service: see `deploy/com.jewell-labs.llm-provider.plist`.

### Get a key (remote clients)

```sh
# allowlisted user, headless (1h Google identity token):
curl -s -X POST localhost:4141/auth/google -H content-type:application/json \
  -d "{\"id_token\": \"$(gcloud auth print-identity-token)\"}"
# or browser: open http://localhost:4141/login   (needs auth.google_client_id/secret in config.toml)
# or xAI device-code:  ./target/release/llm-provider login-xai
# or admin mint:       ./target/release/llm-provider mint <allowed-email>
```

Loopback callers need **no key** — *unless* `auth.trust_loopback = false`, which is the
correct setting when the gateway is exposed (a reverse SSH tunnel makes remote traffic look
like loopback), in which case everyone needs a key. See the `paperclip-admin` skill for
the local / LAN / remote-VPS setups and the tunnel.

## Config

Copy `config.example.toml` → `config.toml`. All fields have defaults (the binary runs with no
file). Providers are a `[[providers]]` list; the tuned llama.cpp launch flags live under
`[[local_runtime.profiles]]`. Secrets (`keys.json`, `config.toml`) are `.gitignore`d.

## Local runtime tuning

`llama-server` flags decide throughput and concurrency. Start a tuned server from config:

```sh
./launch-local.sh qwen3-4b               # reads model/port/flags from config.toml
```

Find the best flags with the DoE harness (wraps `llama-batched-bench`):

```sh
tuning/doe.sh ~/.llama/models/<model>.gguf 32768 512 128   # → tuning/results/*.csv
```

The **tune-local-llm** skill (`.claude/skills/tune-local-llm/`) explains every lever, the
memory math for the 32 GB M1 Max, and how to read the sweep and pick a winner.

## Tests

```sh
cargo test                                   # unit (translation, SSE UTF-8 framing, auth decision)
cargo test --test oauth_integration -- --nocapture   # boots the server, mints real gcloud
                                                     # allowed+denied tokens, runs grok inference
```

The OAuth test proves the allow path (200 + key), the deny path (403 `email_not_allowed`),
loopback bypass, and live inference. The remote-without-key → 401 path is covered by the
pure `identity::is_authorized` unit test.

## Clients

- **Paperclip (canonical):** `integrations/paperclip/` — custom `gateway_openai` external
  adapter that calls this gateway directly over the OpenAI wire. Register with
  `register-adapter.mjs`, then `node integrations/paperclip/test-gateway-direct.mjs`.
  Production: Paperclip on Hostinger VPS + reverse tunnel + OAuth/minted key. **Do not use
  Hermes** (`hermes_local` / `hermes_gateway`) as a client path.
