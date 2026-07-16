# Operator Handbook

This repo runs a **Paperclip agent-company** on the in-house **llm-provider** model gateway.
The custom harness (`rust-agent/`) and the local eval framework (`evals/`) have been removed;
we no longer maintain our own agent/eval harness. See `AGENTS.md` for the current contract.

## Stack (orchestration + models)

| Layer | What |
| --- | --- |
| **Control plane** | Paperclip (self-hosted on Hostinger VPS) — admin via the `paperclip-admin` skill |
| **Model gateway** | In-house **llm-provider** on this Mac (`:4141`), OAuth-gated to the allowlist; Paperclip reaches it via reverse tunnel. Local Qwen3.6-35B-A3B is the `default_model` (first point of contact); Claude/Grok are explicit escalation only. |
| **Coding harness** | Being standardized on a single external runtime — **opencode vs Hermes** comparison in progress. No custom harness. |

## Messaging (SendBlue) for Paperclip agents

Human SMS/iMessage is **agent-native**, not a long-running monorepo daemon:

- Attach company skill **`sendblue`** + **`paperclip`** via the Paperclip setup scripts.
- Host needs `@sendblue/cli` + env/credentials; agents decide when to text (LLM + skill), with issues as SSoT.
- Do **not** reintroduce a deterministic poller/router (retired `paperclip-messenger`).
