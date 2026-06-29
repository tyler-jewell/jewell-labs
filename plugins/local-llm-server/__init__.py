"""local-llm-server — Hermes plugin entry point.

This is a pure *dashboard* plugin: its UI tab and API are discovered from
``dashboard/manifest.json`` (frontend ``dist/index.js`` + backend
``plugin_api.py``). It registers no agent tools or hooks, so ``register`` is a
no-op — present only so Hermes recognizes this directory as a first-class
plugin (clean ``hermes plugins list`` entry, no "missing __init__.py" warning).
"""


def register(ctx):  # noqa: D401 - no tools/hooks; dashboard-only plugin
    return None
