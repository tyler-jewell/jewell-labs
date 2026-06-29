---
name: extending-hermes-dashboard
description: Build, test & install a Hermes dashboard plugin.
version: 1.0.0
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
project-agnostic** so the same plugin runs on someone else's completely
different setup.

## 0. Canonical examples (start here)

All worked examples live in one public repo — **clone or browse it first**:

> **https://github.com/tyler-jewell/jewell-labs**  → `plugins/`

```sh
git clone https://github.com/tyler-jewell/jewell-labs ~/Apps/jewell-labs
```

| Example plugin | Demonstrates |
|----------------|--------------|
| `plugins/local-llm-server`  | Multi-host metrics over SSH, **config-driven hosts** (`config.yaml.example`), host **discovery**, incremental **streaming log** endpoint — the gold standard for host-agnostic design. |
| `plugins/benchmark-results` | Serving data files from `data/`, a companion runner script. |

Read those two before writing your own — copy their idioms rather than
inventing new ones.

## 1. Anatomy of a dashboard plugin

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
- `hermes plugins install/list` treats the dir as a first-class plugin when it
  has `plugin.yaml` + `__init__.py` (absent = a harmless warning).
Ship both so the plugin is installable *and* shows its tab.

## 2. Scaffold

Use the helper bundled with this skill (writes a complete skeleton; no network,
works anywhere):

```sh
bash "${HERMES_SKILL_DIR}/scripts/scaffold.sh" my-plugin       # -> ~/.hermes/plugins/my-plugin/
bash "${HERMES_SKILL_DIR}/scripts/scaffold.sh" my-plugin ./.hermes/plugins   # -> project-local plugin (see step 8)
```

Or copy an example: `cp -r ~/Apps/jewell-labs/plugins/benchmark-results ~/.hermes/plugins/my-plugin` and rename.

## 3. `dashboard/manifest.json`

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
`icon` is any [lucide](https://lucide.dev) icon name.

## 4. Backend — `dashboard/plugin_api.py`

Expose a FastAPI `router`; it is auto-mounted at `/api/plugins/<name>/`.

```python
from fastapi import APIRouter
router = APIRouter()

@router.get("/hello")
async def hello():
    return {"msg": "hi"}   # -> GET /api/plugins/my-plugin/hello
```

Rules that keep it portable:
- **Never hardcode paths/hosts/secrets.** Use `pathlib.Path.home()` for files,
  and read any host/machine settings from a `config.yaml` next to the plugin
  (see step 6). Default to `localhost` when no config exists.
- Keep handlers fast; the UI polls them. Shell out with `subprocess` + a
  timeout if you must.

## 5. Frontend — `dashboard/dist/index.js`

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
`SDK.fetchJSON` injects the session token for you (see gotcha in step 7).
Reuse `SDK.components` (Card/Badge/…) and Tailwind classes for a native look.

## 6. Make it machine-agnostic (config without secrets)

Anything machine-specific (hostnames, SSH aliases, ports, paths) must NOT be
committed. The pattern:

1. Ship **`config.yaml.example`** with safe localhost defaults + commented
   examples. On `hermes plugins install`, Hermes auto-copies every `*.example`
   to its real name (`config.yaml.example` → `config.yaml`).
2. `plugin_api.py` reads `config.yaml` at runtime; falls back to localhost
   defaults if missing.
3. For remote hosts, reference a **`~/.ssh/config` alias** in `config.yaml` —
   never a raw IP, key, or password.
4. Add a discovery endpoint (see `local-llm-server`'s `/discover`) that probes
   the local machine + `~/.ssh/config` aliases and returns ready-to-paste
   config — so a new user on any machine can find their own setup.
5. `.gitignore`: `config.yaml`, generated data, `__pycache__/`, `*.pyc`.

For secrets that belong in env, declare `requires_env:` in `plugin.yaml` and
Hermes prompts for them on install (saved to `~/.hermes/.env`).

## 7. Test locally

```sh
pkill -f "hermes dashboard"
nohup hermes dashboard --port 9119 --host 127.0.0.1 --no-open --skip-build \
  >~/.hermes/dash-start.log 2>&1 & disown
# wait for: HERMES_DASHBOARD_READY port=9119
```

Gotchas:
- **API routes mount at startup** — restart the dashboard after adding/editing
  `plugin_api.py` (a tab rescan alone won't remount routes).
- `/api/plugins/*` is **auth-gated even on loopback**. The browser SPA passes
  the token automatically, but curl needs it:
  ```sh
  TOK=$(curl -s http://127.0.0.1:9119/ | grep -oE 'window.__HERMES_SESSION_TOKEN__ *= *"[^"]+"' | grep -oE '"[^"]+"$' | tr -d '"')
  curl -s -H "X-Hermes-Session-Token: $TOK" "http://127.0.0.1:9119/api/plugins/my-plugin/hello"
  ```
- Confirm discovery: `curl -s http://127.0.0.1:9119/api/dashboard/plugins` lists your `name`.
- Validate before restarting: `node --check dashboard/dist/index.js` and
  `python3 -c "import ast,sys; ast.parse(open('dashboard/plugin_api.py').read())"`.

## 8. Publish + install to the current project

**Publish** to any Git repo using a typed layout (`plugins/<name>/`), then it is
installable by subpath:

```sh
hermes plugins install <owner>/<repo>/plugins/<name> --enable        # GitHub shorthand
hermes plugins install https://github.com/<owner>/<repo>/tree/main/plugins/<name> --enable
hermes plugins install <full-git-url> --enable                       # plugin at repo root
```
`--enable` / `--no-enable` make install **non-interactive (one-shot)**. `--force`
reinstalls (it `rmtree`s the target first — back up a live `config.yaml`/`data/`
and restore after). Verify with `hermes plugins list`, then restart the dashboard.

**Install scope:**
- **User-level (default):** lands in `~/.hermes/plugins/<name>/` — available in
  every project.
- **Current project only:** put the plugin at `./.hermes/plugins/<name>/` in the
  project root and run the dashboard with project plugins enabled:
  ```sh
  HERMES_ENABLE_PROJECT_PLUGINS=1 hermes dashboard ...
  ```
  Project plugins override user/bundled plugins of the same name. Use this when
  the tab is specific to one repo/project.

## 9. Agnosticism checklist (before you publish)

- [ ] No absolute paths — use `Path.home()` / relative-to-plugin paths.
- [ ] No hostnames/IPs/keys in committed files — only in gitignored `config.yaml` / `~/.ssh/config`.
- [ ] `config.yaml.example` defaults to localhost and works out of the box.
- [ ] OS-specific logic is detected at runtime (`uname`, capability probes), not assumed.
- [ ] `node --check` + `python3 -c "ast.parse(...)"` pass.
- [ ] Fresh-clone test: install on a different machine (or a clean `~/.hermes`) and the tab loads with zero manual edits.
- [ ] Never edits Hermes core — everything under the plugin dir.
