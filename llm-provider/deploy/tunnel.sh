#!/usr/bin/env bash
# Reverse SSH tunnel: exposes the Mac gateway (localhost:4141) on the VPS's LOOPBACK
# (127.0.0.1:4141), so Paperclip running ON the VPS can reach it — without opening any public
# port. autossh keeps it alive across drops. The gateway runs trust_loopback=false, so a
# minted key is still required over the tunnel (defense in depth).
#
# Prereqs (one-time):
#   brew install autossh
#   ssh-copy-id root@2.25.132.76        # key-based, passphrase-less (or an agent-loaded key)
#   VPS sshd: AllowTcpForwarding yes (default)
#
# Run:  VPS=root@2.25.132.76 deploy/tunnel.sh    (or via the launchd agent)
set -euo pipefail

VPS="${VPS:-root@2.25.132.76}"
LOCAL_PORT="${LOCAL_PORT:-4141}"
REMOTE_PORT="${REMOTE_PORT:-4141}"

exec autossh -M 0 -N \
  -o ServerAliveInterval=30 -o ServerAliveCountMax=3 \
  -o ExitOnForwardFailure=yes -o BatchMode=yes -o StrictHostKeyChecking=accept-new \
  -R "127.0.0.1:${REMOTE_PORT}:localhost:${LOCAL_PORT}" "$VPS"
