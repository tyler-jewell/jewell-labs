#!/usr/bin/env bash
# One-time: install this Mac's SSH public key on the VPS using the VPS password, so the
# reverse tunnel can authenticate with the key (no password ever again). The password is
# read with no echo into a 0600 temp file, fed to `llm-provider expect` (a pure-Rust PTY
# driver — no expect(1)), and shredded. After this succeeds, everything is key-based.
#
# Run it in a real terminal (it prompts):   deploy/install-tunnel-key.sh
set -euo pipefail

VPS="${VPS:-root@2.25.132.76}"
PUB="$HOME/.ssh/id_ed25519.pub"
BIN="${LLM_PROVIDER_BIN:-$HOME/Apps/jewell-labs/llm-provider/target/release/llm-provider}"
[ -f "$PUB" ] || { echo "no $PUB — run: ssh-keygen -t ed25519 -N '' -f ~/.ssh/id_ed25519"; exit 1; }
[ -x "$BIN" ] || { echo "no $BIN — run: cargo build --release"; exit 1; }

pwfile="$(mktemp)"; chmod 600 "$pwfile"
trap 'rm -f "$pwfile"' EXIT
printf 'VPS password for %s (from your Passwords app; input hidden): ' "$VPS"
read -rs pw; echo
[ -n "$pw" ] || { echo "empty password, aborting"; exit 1; }
printf '%s' "$pw" > "$pwfile"; unset pw

# Force password auth for the install connection; the PTY driver answers the prompt.
printf 'expect [Pp]assword:\nsecret %s\nexpect Number of key|already exist|denied\n' "$pwfile" \
  | "$BIN" expect --timeout 40 -- \
      ssh-copy-id -o StrictHostKeyChecking=accept-new -o PubkeyAuthentication=no -i "$PUB" "$VPS"

echo "verifying key auth (no password should be asked)..."
if ssh -o BatchMode=yes -o ConnectTimeout=10 "$VPS" 'echo KEY_OK; uname -sm'; then
    echo "✔ key-based SSH works. Next: launchctl load ~/Library/LaunchAgents/com.jewell-labs.llm-tunnel.plist"
else
    echo "✘ key auth still failing — check the password and VPS sshd (PasswordAuthentication yes for the install)."
    exit 1
fi
