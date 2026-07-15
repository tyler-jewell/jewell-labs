---
name: drive-interactive-cli
description: >-
  Drive an interactive command from an agent when the one-shot shell tool can't answer its
  prompts — a password (ssh, sudo, ssh-copy-id), a sudo challenge, a y/n question. Uses the
  pure-Rust `llm-provider expect` subcommand (a PTY driver, no tmux, no expect(1)): pipe a
  small step-script on stdin, put the command after `--`. Includes the worked example of
  installing this Mac's SSH key on the VPS by answering ssh-copy-id's password prompt. Use
  when a command needs typed input the agent's Bash tool cannot supply.
---

# Drive interactive CLIs via `llm-provider expect`

The agent's Bash tool is **one-shot and non-interactive**: each call runs to completion with
no stdin, so it can't answer a `password:` prompt or a sudo challenge. The `expect`
subcommand of the `llm-provider` binary runs the command inside a **PTY** and scripts the
prompt/answer exchange — entirely in one Bash call, pure Rust, no tmux and no `expect(1)`.

```sh
BIN=~/Apps/jewell-labs/llm-provider/target/release/llm-provider
printf '<step>\n<step>\n...' | "$BIN" expect [--timeout N] -- CMD ARGS...
```

Steps (one per line on stdin; `#` comments and blank lines ignored):

| step | effect |
|------|--------|
| `expect <regex>` | block until the PTY output matches `<regex>` (bounded by `--timeout`, default 30s). Only output produced *after* the previous match is searched, so a prompt word can't stale-match. |
| `send <text>` | type `<text>` + Enter |
| `secret <path>` | type the trimmed contents of a `chmod 600` file + Enter — the value never appears in argv, the step-script, or shell history |

Exit code: the spawned command's own exit code on success; **1** on an `expect` timeout (the
last PTY output is dumped to stderr so you can see what actually happened) or if the command
is still running after `--timeout`; **2** on a usage/regex error.

## The loop the agent runs (one Bash call)

Script the whole exchange up front — you can't see the screen between steps, so each `expect`
gates the next `send`/`secret`. Because it's a single process, there's no session to persist
or tear down.

```sh
printf 'expect password:\nsecret ~/.vps_pw\nexpect Number of key|already exist|denied\n' \
  | "$BIN" expect --timeout 30 -- ssh user@host ...
echo "exit=$?"
```

If it times out, the stderr dump shows the real prompt text — adjust the regex (wrong
wording, an extra question) rather than re-running blind.

## Secrets

- Prefer `secret <path>`: have the user drop the secret into a `chmod 600` file (e.g.
  `~/.vps_pw`) and pass the **path**. The value never touches chat, argv, or history. Delete
  the file after (`rm -P ~/.vps_pw`).
- Trade-off to state: the value is written to the PTY master fd and lives briefly in this
  process's memory — fine on a single-user Mac. Never ask the user to paste a password into
  chat, and never echo one back in output you print.

## Robustness checklist

- Always pass `--timeout`; on timeout, read the stderr dump and fix the regex — don't loop.
- Anchor `expect` regexes to the actual prompt (`password:`, `\$ $`, `\[y/N\]`); the cursor
  already prevents matching earlier scrollback, so keep patterns specific to the current step.
- Put the whole exchange in the script — an unscripted prompt makes the command block until
  `--timeout`, then it's killed and you get exit 1.

## When this is the wrong tool

`expect` handles a **scripted exchange that completes in one call**. It does **not** hold a
session open *across* the agent's separate tool calls (run a command now, come back later) —
that needs a resident process, which is what tmux is for. We don't need that here; if it ever
comes up, reach for tmux rather than adding a daemon to this binary.

## Worked example — install the SSH key on the VPS

Let the reverse tunnel authenticate by key. `ssh-copy-id` needs the VPS password once; after
that it's key-only. The user places the VPS password in `~/.vps_pw` (`chmod 600`) beforehand:

```sh
BIN=~/Apps/jewell-labs/llm-provider/target/release/llm-provider
printf 'expect password:\nsecret ~/.vps_pw\nexpect Number of key|already exist|denied\n' \
  | "$BIN" expect --timeout 30 -- \
      ssh-copy-id -o StrictHostKeyChecking=accept-new -o PubkeyAuthentication=no \
                  -i ~/.ssh/id_ed25519.pub root@2.25.132.76
rm -P ~/.vps_pw

# verify key auth (non-interactive → plain Bash is fine):
ssh -o BatchMode=yes -o ConnectTimeout=10 root@2.25.132.76 'echo KEY_OK; uname -sm'
```

`deploy/install-tunnel-key.sh` wraps exactly this (prompting for the password with no echo).
Then bring up the tunnel: `launchctl load
~/Library/LaunchAgents/com.jewell-labs.llm-tunnel.plist` (see the `paperclip-admin`
skill, Scenario 3).
