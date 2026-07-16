#!/usr/bin/env bash
# Read-only profile of the Hostinger VPS. Safe to run anytime — it changes nothing.
#   .claude/skills/paperclip-admin/recon.sh            # default host
#   VPS=root@2.25.132.76 .claude/skills/paperclip-admin/recon.sh
set -euo pipefail
VPS="${VPS:-root@2.25.132.76}"
ssh -o BatchMode=yes -o ConnectTimeout=10 "$VPS" 'bash -s' <<'RECON'
echo "== os =="; grep PRETTY_NAME /etc/os-release; uname -r; uptime
echo "== cpu/mem/disk =="; nproc; free -h | awk 'NR<=2'; df -h / | tail -1
echo "== listening ports =="; ss -tlnp 2>/dev/null | awk 'NR==1||/LISTEN/'
echo "== docker containers =="; docker ps --format '{{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}' 2>/dev/null || echo "docker: n/a"
echo "== compose projects =="; ls -d /docker/*/ 2>/dev/null
echo "== running services =="; systemctl list-units --type=service --state=running --no-legend --no-pager 2>/dev/null | awk '{print $1}'
echo "== disk hogs (top /docker) =="; du -sh /docker/* 2>/dev/null | sort -rh | head
echo "== recent auth (last 5 logins) =="; last -n5 2>/dev/null
RECON
