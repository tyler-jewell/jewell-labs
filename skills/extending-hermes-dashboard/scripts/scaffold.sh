#!/usr/bin/env bash
# Scaffold a Hermes dashboard plugin skeleton (tab + API).
# Usage: bash scaffold.sh <plugin-name> [target-base-dir]
#   target-base-dir defaults to ~/.hermes/plugins
#   use ./.hermes/plugins for a project-local plugin
# Machine/repo/project-agnostic: no network, no hardcoded hosts or paths.
set -euo pipefail

NAME="${1:-}"
BASE="${2:-$HOME/.hermes/plugins}"
if [ -z "$NAME" ]; then echo "usage: bash scaffold.sh <plugin-name> [target-base-dir]" >&2; exit 1; fi
if ! printf '%s' "$NAME" | grep -qE '^[a-z0-9][a-z0-9-]*$'; then
  echo "plugin name must be kebab-case (a-z, 0-9, -)" >&2; exit 1; fi

DIR="$BASE/$NAME"
if [ -e "$DIR" ]; then echo "refusing to overwrite existing $DIR" >&2; exit 1; fi
LABEL="$(printf '%s' "$NAME" | tr '-' ' ' | awk '{for(i=1;i<=NF;i++){$i=toupper(substr($i,1,1)) substr($i,2)}}1')"

mkdir -p "$DIR/dashboard/dist"

cat > "$DIR/dashboard/manifest.json" <<JSON
{
  "name": "$NAME",
  "label": "$LABEL",
  "description": "TODO: what this tab shows.",
  "icon": "Server",
  "version": "0.1.0",
  "tab": { "path": "/$NAME", "position": "after:benchmarks" },
  "entry": "dist/index.js",
  "api": "plugin_api.py"
}
JSON

cat > "$DIR/dashboard/plugin_api.py" <<'PY'
"""Backend for this dashboard plugin. Auto-mounted at /api/plugins/<name>/.

Keep it machine-agnostic: use Path.home() for files, read any host/machine
settings from ../config.yaml (default to localhost when absent), never hardcode
machine-specific values.
"""
from pathlib import Path
from fastapi import APIRouter

router = APIRouter()
PLUGIN_ROOT = Path(__file__).resolve().parent.parent


@router.get("/hello")
async def hello():
    return {"msg": "hello from this plugin"}
PY

cat > "$DIR/dashboard/dist/index.js" <<'JS'
/* Plain IIFE frontend — no build step. */
(function () {
  "use strict";
  const SDK = window.__HERMES_PLUGIN_SDK__; if (!SDK) return;
  const { React } = SDK; const h = React.createElement;
  const { useState, useEffect } = SDK.hooks;
  const C = SDK.components || {};
  const API = "/api/plugins/__NAME__";
  const Card = C.Card || ((p) => h("div", { className: "rounded-lg border p-4 " + (p.className || "") }, p.children));

  function Page() {
    const [data, setData] = useState(null);
    const [err, setErr] = useState(null);
    useEffect(() => {
      SDK.fetchJSON(API + "/hello").then(setData).catch((e) => setErr(String(e)));
    }, []);
    return h("div", { className: "p-6" },
      h("h1", { className: "text-xl font-semibold mb-4" }, "__LABEL__"),
      err ? h("div", { className: "text-red-400 text-sm" }, err)
          : h(Card, null, data ? data.msg : "Loading…"));
  }
  window.__HERMES_PLUGINS__.register("__NAME__", Page);
})();
JS
# substitute name/label into the JS template
sed -i.bak "s/__NAME__/$NAME/g; s/__LABEL__/$LABEL/g" "$DIR/dashboard/dist/index.js" && rm -f "$DIR/dashboard/dist/index.js.bak"

cat > "$DIR/plugin.yaml" <<YAML
manifest_version: 1
name: $NAME
version: 0.1.0
author: TODO
kind: standalone
description: >-
  TODO: one-line description. Pure dashboard plugin (tab + API); no tools/hooks.
provides_tools: []
provides_hooks: []
YAML

cat > "$DIR/__init__.py" <<'PY'
"""Dashboard-only plugin: tab/API discovered from dashboard/manifest.json.
register() is a no-op so Hermes recognizes this as a first-class plugin."""


def register(ctx):
    return None
PY

cat > "$DIR/config.yaml.example" <<'YAML'
# Per-machine settings (auto-copied to config.yaml on install; config.yaml is
# gitignored). Keep machine-specific values OUT of the repo —
# aliases here instead. Delete this file if your plugin needs no config.
hosts:
  - name: local
    kind: local
YAML

cat > "$DIR/README.md" <<MD
# $LABEL

Hermes dashboard plugin. Install:

\`\`\`sh
hermes plugins install <owner>/<repo>/plugins/$NAME --enable
hermes dashboard   # open the "$LABEL" tab (/$NAME)
\`\`\`
MD

cat > "$DIR/.gitignore" <<'GI'
config.yaml
**/data/run-*.json
**/data/raw/
__pycache__/
*.pyc
.DS_Store
GI

echo "Scaffolded plugin at: $DIR"
echo "Next: edit manifest.json + plugin_api.py + dist/index.js, then restart the dashboard."
node --check "$DIR/dashboard/dist/index.js" && echo "index.js: syntax OK"
python3 -c "import ast; ast.parse(open('$DIR/dashboard/plugin_api.py').read()); print('plugin_api.py: syntax OK')"
