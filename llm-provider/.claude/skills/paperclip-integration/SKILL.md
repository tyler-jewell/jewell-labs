---
name: paperclip-integration
description: >-
  Wire a Paperclip instance to the llm-provider gateway via the custom gateway_openai
  adapter — locally (same Mac), on the LAN, or remotely (Paperclip on a Hostinger VPS
  reaching the Mac gateway over a reverse SSH tunnel). Use when setting up Paperclip to use
  the multi-model gateway, minting/rotating the key it needs, or configuring the adapter's
  baseUrl/apiKey for each hosting scenario. Covers the auth model (loopback trust vs
  trust_loopback=false) and the tunnel setup.
---

# Paperclip ↔ llm-provider integration

Paperclip agents reach the gateway through a custom **`gateway_openai`** external adapter
(`integrations/paperclip/gateway-adapter/`) whose `execute(ctx)` POSTs to
`/v1/chat/completions` and streams the reply back — no hermes, no agent CLI.

## Where the gateway lives (and why)

The gateway runs **only on the Mac** (launchd `com.jewell-labs.llm-provider`, port 4141). It
is Mac-bound: Claude uses the macOS **Keychain**, local models use the **M1 Max GPU**, grok
uses `~/.grok`. It cannot be hosted on a Linux VPS without losing Claude + local models, so a
remote Paperclip must reach *into* the Mac (see Remote below).

## Auth model

- `auth.trust_loopback = true` (default): `127.0.0.1`/`::1` is trusted without a key.
- `auth.trust_loopback = false` (**current setting — the gateway is exposed via a tunnel**):
  every request needs a minted key, because the reverse tunnel makes VPS traffic appear as
  loopback. Loopback callers on the Mac therefore also need a key now.

### Mint / rotate the key

```sh
# admin mint (simplest):
LLM_PROVIDER_DIR=~/Apps/jewell-labs/llm-provider \
  ~/Apps/jewell-labs/llm-provider/target/release/llm-provider mint tyler.p.jewell@gmail.com
# or headless Google (1h gcloud token):
curl -s -X POST localhost:4141/auth/google -H content-type:application/json \
  -d "{\"id_token\": \"$(gcloud auth print-identity-token)\"}"
# or xAI device-code:  llm-provider login-xai
```

The local key lives at `~/Apps/jewell-labs/llm-provider/.gateway-key` (0600, gitignored).

## Register the adapter (every Paperclip instance)

```sh
node integrations/paperclip/register-adapter.mjs        # writes $PAPERCLIP_HOME/adapter-plugins.json
npx paperclipai run                                      # gateway_openai now selectable
```

Then create an agent with adapter type `gateway_openai` and the `adapterConfig` from
`integrations/paperclip/adapter-config.json` (edit `baseUrl`/`apiKeyFile` per scenario below).

## Scenario 1 — Local (Paperclip on this Mac)

```json
"adapterConfig": {
  "baseUrl": "http://localhost:4141/v1",
  "model": "grok-4.20-0309-non-reasoning",
  "apiKeyFile": "/Users/studio/Apps/jewell-labs/llm-provider/.gateway-key"
}
```

Verify: `node integrations/paperclip/test-gateway-direct.mjs` → 16 checks pass.

## Scenario 2 — LAN (Paperclip on another home machine)

Same as local but `baseUrl = http://10.0.0.166:4141/v1` (the Mac's LAN IP). A key is
required (LAN is non-loopback anyway). Copy the key file onto that machine and point
`apiKeyFile` at it. Verify with `GATEWAY_URL=http://10.0.0.166:4141 node ...test-gateway-direct.mjs`.

## Scenario 3 — Remote (Paperclip on the Hostinger VPS, 2.25.132.76)

The VPS is a **front door**, not the host. A reverse SSH tunnel exposes the Mac gateway on
the VPS's loopback; Paperclip on the VPS talks to `127.0.0.1:4141`. Nothing is public.

### One-time prerequisites
```sh
brew install autossh                       # on the Mac
ssh-copy-id root@2.25.132.76               # passphrase-LESS key (a background tunnel can't type one)
ssh root@2.25.132.76 true                  # must succeed non-interactively before proceeding
```

### Bring up the persistent tunnel (Mac side)
```sh
cp deploy/com.jewell-labs.llm-tunnel.plist ~/Library/LaunchAgents/
launchctl load ~/Library/LaunchAgents/com.jewell-labs.llm-tunnel.plist
# verify from the VPS:
ssh root@2.25.132.76 'curl -s localhost:4141/healthz'                    # {"ok":true}
ssh root@2.25.132.76 'curl -s -o /dev/null -w "%{http_code}\n" localhost:4141/v1/models'  # 401 (no key)
```

### Put the key on the VPS + configure Paperclip
```sh
scp ~/Apps/jewell-labs/llm-provider/.gateway-key root@2.25.132.76:/root/llm-provider.key
# copy the adapter package to the VPS:
scp -r integrations/paperclip/gateway-adapter root@2.25.132.76:/root/gateway-openai-adapter
```
On the VPS, register the adapter (point `register-adapter.mjs`'s `PKG_DIR` at
`/root/gateway-openai-adapter`) and use this adapter config:
```json
"adapterConfig": {
  "baseUrl": "http://127.0.0.1:4141/v1",
  "model": "grok-4.20-0309-non-reasoning",
  "apiKeyFile": "/root/llm-provider.key"
}
```
Verify on the VPS: `GATEWAY_URL=http://127.0.0.1:4141 node test-gateway-direct.mjs`.

### How the pieces sit
```
Mac: llm-provider :4141 (trust_loopback=false)
  ▲ reverse SSH tunnel (autossh + launchd; Mac dials the VPS)
  │ VPS 127.0.0.1:4141  ⇒  Mac gateway
VPS: Paperclip ──▶ http://127.0.0.1:4141/v1  (Bearer <key from /root/llm-provider.key>)
```

## Rotating the key later

Re-mint (above), overwrite `.gateway-key`, `scp` it to the VPS. The gateway reloads the key
set on file change (no restart); Paperclip re-reads `apiKeyFile` per run. Old keys remain
valid until removed from `keys.json`.

## Troubleshooting

- **401 everywhere**: `trust_loopback=false` and no/învalid key — check `apiKeyFile` resolves
  and the key exists in `keys.json`.
- **Remote can't connect**: tunnel down — `cat tunnel.log`, confirm `ssh root@2.25.132.76 true`
  works non-interactively, reload the tunnel agent.
- **Local model 404**: the local backend isn't up — start it with `./launch-local.sh` (see
  the `tune-local-llm` skill).
