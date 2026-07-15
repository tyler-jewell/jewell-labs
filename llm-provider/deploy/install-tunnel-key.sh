#!/usr/bin/env bash
# One-time: install this Mac's SSH public key on the VPS using the VPS password, so the
# reverse tunnel can authenticate with the key (no password ever again). The password is
# read with no echo, passed to `expect` via the environment, and NEVER written to disk or
# printed. After this succeeds, everything is key-based.
#
# Run it in a real terminal (it prompts):   deploy/install-tunnel-key.sh
set -euo pipefail

VPS="${VPS:-root@2.25.132.76}"
PUB="$HOME/.ssh/id_ed25519.pub"
[ -f "$PUB" ] || { echo "no $PUB — run: ssh-keygen -t ed25519 -N '' -f ~/.ssh/id_ed25519"; exit 1; }
command -v expect >/dev/null || { echo "expect not found"; exit 1; }

printf 'VPS password for %s (from your Passwords app; input hidden): ' "$VPS"
read -rs TUNNEL_PW; echo
[ -n "$TUNNEL_PW" ] || { echo "empty password, aborting"; exit 1; }

# Force password auth for the install connection; feed the password to ssh-copy-id via expect.
TUNNEL_PW="$TUNNEL_PW" VPS="$VPS" PUB="$PUB" expect <<'EXP'
set timeout 40
set pw  $env(TUNNEL_PW)
set vps $env(VPS)
set pub $env(PUB)
spawn ssh-copy-id -o StrictHostKeyChecking=accept-new -o PubkeyAuthentication=no -i $pub $vps
expect {
    -nocase -re {password:} { send "$pw\r"; exp_continue }
    -re {already exist|Number of key\(s\) added|skipped} { }
    timeout { puts "\nTIMED OUT talking to $vps"; exit 2 }
    eof
}
EXP
unset TUNNEL_PW

echo "verifying key auth (no password should be asked)..."
if ssh -o BatchMode=yes -o ConnectTimeout=10 "$VPS" 'echo KEY_OK; uname -sm'; then
    echo "✔ key-based SSH works. Next: launchctl load ~/Library/LaunchAgents/com.jewell-labs.llm-tunnel.plist"
else
    echo "✘ key auth still failing — check the password and VPS sshd (PasswordAuthentication yes for the install)."
    exit 1
fi
