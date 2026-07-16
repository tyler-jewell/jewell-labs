---
name: add-provider
description: >-
  Add or update a model provider in llm-provider (the OpenAI-compatible gateway at
  ~/Apps/jewell-labs/llm-provider). Use when wiring a new upstream (another OpenAI-compatible
  API, or one needing translation like Anthropic), adding a local backend, or changing how an
  existing provider authenticates/routes. Explains the Provider trait, the 3-edit registration,
  and when to reuse PassthroughProvider vs. write a translator.
---

# Add or update a provider

`llm-provider` routes model ids to providers behind one trait. Adding a provider is
mechanical: **one module + one config variant + one registry arm.** Most new upstreams need
*no new code at all* — just a config entry — because `PassthroughProvider` already handles
any OpenAI-compatible API.

## Decide first: passthrough or translator?

- **OpenAI-compatible upstream** (speaks `/v1/chat/completions`) → reuse
  `PassthroughProvider`. This covers local llama.cpp/ollama and grok. Often just a config
  entry; no Rust.
- **Different API shape** (like Anthropic Messages) → write a new provider module that
  translates, modeled on `src/providers/claude.rs`.

## Case A — another OpenAI-compatible upstream (no code)

Add a block to `config.toml`. Local backend (no auth, discovered-set routing):

```toml
[[providers]]
kind = "local"
name = "vllm"                       # owned_by tag
base_url = "http://127.0.0.1:8000/v1"
```

A remote OpenAI-compatible service that needs a bearer works today only if it uses one of
the existing token sources. If it needs a **static API key** or a **new auth scheme**, add a
`TokenSource` variant (see Case B, step 2) and construct it in `PassthroughProvider`.

## Case B — a provider that needs translation or new auth

Three edits. Follow `claude.rs` as the template.

1. **New module** `src/providers/<name>.rs`: a struct implementing `registry::Provider`:
   ```rust
   #[async_trait]
   impl Provider for MyProvider {
       fn name(&self) -> &str { "myprov" }                 // also the model owned_by tag
       fn matches(&self, model: &str) -> bool { model.starts_with(&self.prefix) }
       async fn models(&self, http: &reqwest::Client) -> anyhow::Result<Vec<Value>> { /* OpenAI model objects; Ok(vec![]) if unreachable */ }
       async fn chat(&self, http: &reqwest::Client, body: Value) -> anyhow::Result<Response> { /* translate → call → OpenAI response, SSE or JSON */ }
   }
   ```
   Export it from `src/providers/mod.rs` (`mod <name>; pub use <name>::MyProvider;`).

2. **New auth (only if needed)**: add a `TokenSource` variant in `src/token/mod.rs` and its
   refresh logic in a new `src/token/<name>.rs` (model on `keychain.rs`/`oidc.rs`). Each
   source owns a cache + mutex so concurrent chats don't double-refresh.

3. **New config variant** in `src/config.rs`: add `MyProv(MyProvConfig)` to the
   `ProviderConfig` enum (with `#[serde(rename_all = "lowercase")]` the TOML `kind` is
   `"myprov"`), and a `MyProvConfig` struct for its slice.

4. **One registry arm** in `src/registry.rs` `from_config`:
   ```rust
   ProviderConfig::MyProv(c) => Box::new(MyProvider::new(c)) as Box<dyn Provider>,
   ```
   Order matters: put prefix-routed providers **before** local (discovered-set) ones so a
   prefixed id isn't shadowed by a discovered local model.

## Rules to preserve

- `matches()` is sync. Prefix providers compare a prefix; local providers consult a set
  warmed by `models()` (the router warms it on a miss — see the ponytail note in
  `registry.rs`). Don't do async work in `matches()`.
- `models()` must return `Ok(vec![])` (not `Err`) when the backend is down, so one dead
  backend never sinks the whole `/v1/models` aggregate.
- Streaming: emit OpenAI `chat.completion.chunk` frames ending with `data: [DONE]`. If you
  translate an upstream SSE, buffer **bytes** and split on `\n\n`, decoding only complete
  frames (see `claude_sse` — a multibyte codepoint split across TCP chunks corrupts
  otherwise).
- Add a unit test next to the module for any translation logic.

## Verify

```sh
cargo test                          # unit + translation tests
cargo run &                         # or ./launch + the launchd service
curl -s localhost:4141/v1/models | grep <your owned_by tag>
curl -s localhost:4141/v1/chat/completions -d '{"model":"<id>","messages":[{"role":"user","content":"hi"}]}'
```
