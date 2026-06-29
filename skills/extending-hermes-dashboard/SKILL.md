---
name: extending-hermes-dashboard
description: Build, test & install a Hermes dashboard plugin.
version: 1.0.1
license: MIT
author: tyler-jewell
platforms: [linux, macos, windows]
metadata:
  hermes:
    tags: [hermes, dashboard, plugin, extend, ui, fastapi, tab, install]
    category: software-development
    related_skills: []
---

# Extending the Hermes Dashboard

This skill teaches you to add a **new tab + API to the Hermes dashboard** the
*supported* way — a self-contained plugin under `~/.hermes/plugins/` that
**never edits Hermes core**, so `hermes update` stays clean. It takes you from
zero to an installed, working tab, and keeps everything **machine-, repo-, and
project-agnostic** so the same plugin runs on someone else's different setup.

## When to use

Use this when you want to surface live data or controls in the Hermes dashboard
(a metrics tab, a results viewer, a control panel) without forking Hermes.

## Worked examples (start here)

Browse the example plugins in the companion repository
**`tyler-jewell/jewell-labs`** under its `plugins/` directory:

| Example plugin | Demonstrates |
|----------------|--------------|
| `plugins/local-llm-server`  | Multi-host metrics, **config-driven hosts** (`config.yaml.example`), host **discovery**, an incremental **streaming log** endpoint — the gold standard for machine-agnostic design. |
| `plugins/benchmark-results` | Serving data files from a `data/` directory, plus a companion runner script. |

Read those two before writing your own — copy their idioms rather than
inventing new ones.

## Anatomy of a dashboard plugin

```
<plugin-name>/
  plugin.yaml              # first-class plugin manifest (makes it `hermes plugins`-installable)
  __init__.py              # no-op register(ctx) for pure dashboard plugins
  README.md
  config.yaml.example      # OPTIONAL: per-machine settings; auto-copied to config.yaml on install
  dashboard/
    manifest.json          # tab metadata (label, icon, path, entry, api)
    plugin_api.py          # FastAPI router  -> mounted at /api/plugins/<name>/
    dist/
      index.js             # plain IIFE frontend (NO build step)
```

Two independent discovery paths:
- The **dashboard** discovers the tab from `dashboard/manifest.json` (no
  `plugin.yaml` needed for the tab to appear).
- `hermes plugins install/list` treats the directory as a first-class plugin
  when it has `plugin.yaml` + `__init__.py` (absent = a harmless warning).
Ship both so the plugin is installable *and* shows its tab.

## Step 1 — Scaffold

Run the bundled helper (writes a complete skeleton; no network, works anywhere):

```sh
bash "${HERMES_SKILL_DIR}/scripts/scaffold.sh" my-plugin
# project-local instead of user-level: pass a project plugins dir as arg 2
bash "${HERMES_SKILL_DIR}/scripts/scaffold.sh" my-plugin ./.hermes/plugins
```

Or copy an example plugin directory from the companion repo and rename it.

## Step 2 — `dashboard/manifest.json`

```json
{
  "name": "my-plugin",
  "label": "My Plugin",
  "description": "What this tab shows.",
  "icon": "Server",
  "version": "0.1.0",
  "tab": { "path": "/my-plugin", "position": "after:benchmarks" },
  "entry": "dist/index.js",
  "api": "plugin_api.py"
}
```
`name` must match `plugin.yaml` `name` and the frontend `register("<name>", …)`.
`icon` is any lucide icon name. `tab.position` is `end`, `after:<tab>`, or `before:<tab>`.

## Step 3 — Backend (`dashboard/plugin_api.py`)

Expose a FastAPI `router`; it is auto-mounted at `/api/plugins/<name>/`.

```python
from fastapi import APIRouter
router = APIRouter()

@router.get("/hello")
async def hello():
    return {"msg": "hi"}   # -> GET /api/plugins/my-plugin/hello
```

Rules that keep it portable:
- **Never hardcode paths or machine-specific values.** Use `pathlib.Path.home()`
  for files, and read any per-machine settings from a `config.yaml` next to the
  plugin (see step 6). Default to localhost when no config exists.
- Keep handlers fast; the UI polls them. Shell out with `subprocess` + a timeout
  if needed, and detect OS capabilities at runtime instead of assuming them.

## Step 4 — Frontend (`dashboard/dist/index.js`)

A plain IIFE (no bundler). The dashboard injects an SDK and a registry:

```js
(function () {
  const SDK = window.__HERMES_PLUGIN_SDK__; if (!SDK) return;
  const { React } = SDK; const h = React.createElement;
  const { useState, useEffect } = SDK.hooks;
  const API = "/api/plugins/my-plugin";

  function Page() {
    const [data, setData] = useState(null);
    useEffect(() => { SDK.fetchJSON(API + "/hello").then(setData); }, []);
    return h("div", { className: "p-6" }, data ? data.msg : "Loading…");
  }
  window.__HERMES_PLUGINS__.register("my-plugin", Page);
})();
```
`SDK.fetchJSON` handles auth for you (see the gotcha in step 7). Reuse
`SDK.components` (Card/Badge/…) and Tailwind classes for a native look.

## Step 5 — First-class + installable

- `plugin.yaml`: `manifest_version: 1`, plus `name`, `version`, `author`,
  `kind: standalone`, and empty `provides_tools` / `provides_hooks` for a pure
  dashboard plugin.
- `__init__.py` with a no-op `def register(ctx): return None`.

## Step 6 — Make it machine-agnostic (config without sensitive values)

Anything machine-specific (host names, ports, paths) must NOT be committed:

1. Ship **`config.yaml.example`** with safe localhost defaults. On install,
   Hermes auto-copies every `*.example` to its real name
   (`config.yaml.example` → `config.yaml`).
2. `plugin_api.py` reads `config.yaml` at runtime; falls back to localhost
   defaults if absent.
3. For remote hosts, reference an alias from the user's SSH config rather than a
   literal address.
4. Add a discovery endpoint (see `local-llm-server`'s `discover` route) that
   probes the local machine and the user's configured remote aliases and returns
   ready-to-paste config — so a new user on any machine can find their own setup.
5. Add a `.gitignore`: `config.yaml`, generated data, `__pycache__/`, `*.pyc`.

For values that belong in the environment, declare `requires_env:` in
`plugin.yaml` and Hermes prompts for them on install.

## Step 7 — Test locally

Restart the dashboard so new API routes mount (routes mount at startup):

```sh
pkill -f "hermes dashboard"
nohup hermes dashboard --port 9119 --host 127.0.0.1 --no-open --skip-build \
  >~/.hermes/dash-start.log 2>&1 & disown
# wait for the log line: HERMES_DASHBOARD_READY port=9119
```

Gotchas:
- **API routes mount at startup** — restart after adding/editing `plugin_api.py`
  (a tab rescan alone won't remount routes).
- `/api/plugins/*` is **auth-gated even on loopback**. The browser SPA passes the
  token automatically; in the browser it just works. For command-line checks,
  read the token from the served page's `window.__HERMES_SESSION_TOKEN__` and
  send it as the `X-Hermes-Session-Token` header (the example plugins'
  verification notes show the exact one-liner).
- Confirm discovery: the dashboard plugins endpoint lists your `name`.
- Validate before restart: `node --check dashboard/dist/index.js` and
  `python3 -c "import ast; ast.parse(open('dashboard/plugin_api.py').read())"`.

## Step 8 — Publish + install to the current project

Publish to any Git repo using a typed layout (`plugins/<name>/`), then install
by subpath:

```sh
hermes plugins install <owner>/<repo>/plugins/<name> --enable
```
`--enable` / `--no-enable` make install **non-interactive (one-shot)**. `--force`
reinstalls (it removes the target first — back up a live `config.yaml`/`data/`
and restore after). Verify with `hermes plugins list`, then restart the dashboard.

**Install scope:**
- **User-level (default):** lands in `~/.hermes/plugins/<name>/` — available in
  every project.
- **Current project only:** place the plugin at `./.hermes/plugins/<name>/` in the
  project root and run the dashboard with `HERMES_ENABLE_PROJECT_PLUGINS=1`.
  Project plugins override user/bundled plugins of the same name.

### Keeping it current (avoiding staleness)

Native `hermes plugins update <name>` only works when the installed directory is
a git checkout (i.e. the plugin lived at a repo root). A plugin installed from a
**subdirectory** of a monorepo has no `.git`, so refresh it by re-running the
install with `--force` (back up and restore `config.yaml` + `data/` around it),
or publish each plugin as its own repo so `hermes plugins update` can `git pull`.
Skills, by contrast, update by identifier via `hermes skills check` /
`hermes skills update` and do not need a `.git` checkout.

## Agnosticism checklist (before you publish)

- [ ] No absolute paths — use `Path.home()` / paths relative to the plugin.
- [ ] No machine-specific values in committed files — only in a gitignored `config.yaml`.
- [ ] `config.yaml.example` defaults to localhost and works out of the box.
- [ ] OS-specific logic is detected at runtime, not assumed.
- [ ] `node --check` + `python3 -c "ast.parse(...)"` pass.
- [ ] Fresh-clone test: install on a different machine (or a clean `~/.hermes`) and the tab loads with zero manual edits.
- [ ] Never edits Hermes core — everything lives under the plugin directory.
