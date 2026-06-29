/**
 * Benchmarks — Hermes Dashboard Plugin (frontend)
 *
 * Plain IIFE, no build step. Uses window.__HERMES_PLUGIN_SDK__ for React +
 * shadcn primitives, and fetches normalized benchmark runs from this plugin's
 * backend at /api/plugins/benchmark-results/.
 *
 * Shows local LLM coding/SWE benchmark results produced by
 * lm-evaluation-harness run against Ollama models.
 */
(function () {
  "use strict";

  const SDK = window.__HERMES_PLUGIN_SDK__;
  if (!SDK) return;

  const { React } = SDK;
  const h = React.createElement;
  const { useState, useEffect, useCallback } = SDK.hooks;
  const C = SDK.components || {};
  const utils = SDK.utils || {};
  const timeAgo = utils.timeAgo || ((d) => new Date((d || 0) * 1000).toLocaleString());

  const API = "/api/plugins/benchmark-results";

  // --- small fallback-friendly UI helpers -------------------------------
  const Card = C.Card || ((p) => h("div", { className: "rounded-lg border p-4 " + (p.className || "") }, p.children));
  const CardContent = C.CardContent || ((p) => h("div", { className: p.className || "" }, p.children));
  const Badge = C.Badge || ((p) => h("span", { className: "inline-block rounded px-2 py-0.5 text-xs font-medium border " + (p.className || "") }, p.children));
  const Button = C.Button || ((p) => h("button", Object.assign({ className: "rounded border px-3 py-1 text-sm" }, p), p.children));

  function pct(score) {
    if (score == null || isNaN(score)) return "—";
    return (score * 100).toFixed(1) + "%";
  }

  function scoreTone(score) {
    if (score == null) return "";
    if (score >= 0.7) return "bg-green-500/15 text-green-400 border-green-500/30";
    if (score >= 0.4) return "bg-yellow-500/15 text-yellow-400 border-yellow-500/30";
    return "bg-red-500/15 text-red-400 border-red-500/30";
  }

  function StatCard(props) {
    return h(Card, { className: "flex-1 min-w-[160px]" },
      h(CardContent, { className: "p-0" },
        h("div", { className: "text-xs uppercase tracking-wide opacity-60" }, props.label),
        h("div", { className: "mt-1 text-2xl font-semibold" }, props.value),
        props.sub ? h("div", { className: "mt-0.5 text-xs opacity-50" }, props.sub) : null
      )
    );
  }

  function BenchmarksPage() {
    const [runs, setRuns] = useState(null);
    const [error, setError] = useState(null);
    const [loading, setLoading] = useState(false);

    const load = useCallback(() => {
      setLoading(true);
      SDK.fetchJSON(API + "/runs")
        .then((d) => { setRuns((d && d.runs) || []); setError(null); })
        .catch((e) => setError(String(e && e.message ? e.message : e)))
        .finally(() => setLoading(false));
    }, []);

    useEffect(() => { load(); }, [load]);

    const header = h("div", { className: "flex items-center justify-between mb-4" },
      h("div", null,
        h("h1", { className: "text-xl font-semibold" }, "Benchmarks"),
        h("p", { className: "text-sm opacity-60" }, "Local LLM coding/SWE benchmarks · lm-evaluation-harness via Ollama")
      ),
      h(Button, { onClick: load, disabled: loading }, loading ? "Refreshing…" : "Refresh")
    );

    if (error) {
      return h("div", { className: "p-6" }, header,
        h(Card, { className: "border-red-500/40" },
          h(CardContent, { className: "p-0 text-sm text-red-400" }, "Failed to load runs: " + error)));
    }
    if (runs == null) {
      return h("div", { className: "p-6" }, header, h("div", { className: "opacity-60 text-sm" }, "Loading…"));
    }
    if (runs.length === 0) {
      return h("div", { className: "p-6" }, header,
        h(Card, null, h(CardContent, { className: "p-0 text-sm opacity-70" },
          "No benchmark runs yet. Run one with:",
          h("pre", { className: "mt-2 whitespace-pre-wrap rounded bg-black/30 p-2 text-xs" },
            "bun ~/.hermes/plugins/benchmark-results/runner/run.ts --task humaneval_instruct --model qwen3:14b --limit 20"))));
    }

    const latest = runs[0];
    const stats = h("div", { className: "flex flex-wrap gap-3 mb-5" },
      h(StatCard, { label: "Latest score", value: pct(latest.score), sub: (latest.metric || "pass@1") + " · " + (latest.task || "") }),
      h(StatCard, { label: "Model", value: latest.model || "—", sub: latest.endpoint || "" }),
      h(StatCard, { label: "Problems", value: String(latest.n_problems != null ? latest.n_problems : "—"), sub: latest.n_total ? ("of " + latest.n_total) : "" }),
      h(StatCard, { label: "Runs", value: String(runs.length), sub: latest.date ? timeAgo(latest.date) : "" })
    );

    const rows = runs.map((r, i) =>
      h("tr", { key: r.id || i, className: "border-t border-white/5" },
        h("td", { className: "py-2 pr-4" }, h(Badge, { className: scoreTone(r.score) }, pct(r.score))),
        h("td", { className: "py-2 pr-4 font-mono text-xs" }, r.model || "—"),
        h("td", { className: "py-2 pr-4" }, r.task || "—"),
        h("td", { className: "py-2 pr-4 text-xs opacity-70" }, r.metric || "pass@1"),
        h("td", { className: "py-2 pr-4 text-right" }, r.n_problems != null ? r.n_problems : "—"),
        h("td", { className: "py-2 pr-4 text-right text-xs opacity-70" }, r.eval_time_s != null ? (Math.round(r.eval_time_s) + "s") : "—"),
        h("td", { className: "py-2 text-xs opacity-60" }, r.date ? timeAgo(r.date) : "—")
      )
    );

    const table = h(Card, null, h(CardContent, { className: "p-0 overflow-x-auto" },
      h("table", { className: "w-full text-sm" },
        h("thead", null, h("tr", { className: "text-left text-xs uppercase tracking-wide opacity-50" },
          ["Score", "Model", "Task", "Metric", "N", "Time", "When"].map((c, i) =>
            h("th", { key: i, className: "pb-2 pr-4" + (i >= 4 ? " text-right" : "") }, c)))),
        h("tbody", null, rows))));

    return h("div", { className: "p-6" }, header, stats, table);
  }

  if (window.__HERMES_PLUGINS__ && typeof window.__HERMES_PLUGINS__.register === "function") {
    window.__HERMES_PLUGINS__.register("benchmark-results", BenchmarksPage);
  }
})();
