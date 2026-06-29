"""benchmark-results — Hermes plugin entry point.

Pure *dashboard* plugin: the "Benchmarks" tab and its API are discovered from
``dashboard/manifest.json``. No agent tools or hooks, so ``register`` is a
no-op — present so Hermes treats this directory as a first-class plugin.
"""


def register(ctx):  # noqa: D401 - no tools/hooks; dashboard-only plugin
    return None
