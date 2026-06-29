/**
 * Local LLM Server — Hermes Dashboard Plugin (frontend)
 *
 * Plain IIFE, no build step. Renders one card per configured Ollama host
 * (GPU util / memory / loaded models, auto-detected Apple-Metal / NVIDIA /
 * Jetson backend) and a live, incremental stream of Ollama API calls +
 * token-rate for the selected host. Talks to this plugin's backend at
 * /api/plugins/local-llm-server/.
 */
(function () {
  "use strict";

  const SDK = window.__HERMES_PLUGIN_SDK__;
  if (!SDK) return;

  const { React } = SDK;
  const h = React.createElement;
  const { useState, useEffect, useRef, useCallback } = SDK.hooks;
  const C = SDK.components || {};

  const API = "/api/plugins/local-llm-server";
  const STATS_MS = 2500;
  const LOG_MS = 1500;

  const Card = C.Card || ((p) => h("div", { className: "rounded-lg border p-4 " + (p.className || "") }, p.children));
  const CardContent = C.CardContent || ((p) => h("div", { className: p.className || "" }, p.children));
  const Badge = C.Badge || ((p) => h("span", { className: "inline-block rounded px-2 py-0.5 text-xs font-medium border " + (p.className || "") }, p.children));

  function tone(frac) {
    if (frac == null) return "bg-white/10";
    if (frac >= 0.85) return "bg-red-500/70";
    if (frac >= 0.6) return "bg-yellow-500/70";
    return "bg-green-500/60";
  }
  function tempTone(c) {
    if (c == null) return "";
    if (c >= 80) return "text-red-400";
    if (c >= 65) return "text-yellow-400";
    return "text-green-400";
  }
  function backendTone(kind) {
    if (kind === "apple-metal") return "bg-sky-500/15 text-sky-300 border-sky-500/30";
    if (kind === "jetson-cuda") return "bg-green-500/15 text-green-300 border-green-500/30";
    if (kind === "nvidia-cuda") return "bg-emerald-500/15 text-emerald-300 border-emerald-500/30";
    return "bg-white/10 text-white/70 border-white/20";
  }

  function Bar(props) {
    const frac = props.frac;
    const w = frac == null ? 0 : Math.max(0, Math.min(1, frac)) * 100;
    return h("div", { className: "mb-2" },
      h("div", { className: "flex justify-between text-xs mb-0.5" },
        h("span", { className: "opacity-70" }, props.label),
        h("span", { className: "font-mono opacity-90" }, props.value)),
      h("div", { className: "h-2 w-full rounded bg-white/10 overflow-hidden" },
        h("div", { className: "h-full rounded " + tone(frac), style: { width: w + "%" } }))
    );
  }

  // ---- one card per host -------------------------------------------------
  function ServerCard(props) {
    const name = props.name;
    const [s, setS] = useState(null);
    const [err, setErr] = useState(null);

    const poll = useCallback(() => {
      SDK.fetchJSON(API + "/stats?host=" + encodeURIComponent(name))
        .then((d) => { setS(d); setErr(d && d.online === false ? (d.error || "offline") : null); })
        .catch((e) => setErr(String(e && e.message ? e.message : e)));
    }, [name]);

    useEffect(() => {
      poll();
      const t = setInterval(poll, STATS_MS);
      return () => clearInterval(t);
    }, [poll]);

    const online = s && s.online;
    const gpu = (s && s.gpu) || {};
    const ollama = (s && s.ollama) || {};
    const util = gpu.util_pct;
    const memFrac = (gpu.mem_used_gb != null && gpu.mem_total_gb) ? gpu.mem_used_gb / gpu.mem_total_gb : null;
    const temps = gpu.temps || {};
    const tj = temps.tj != null ? temps.tj : null;

    const head = h("div", { className: "flex items-center justify-between mb-3" },
      h("div", { className: "flex items-center gap-2" },
        h("span", { className: "text-base font-semibold" }, name),
        h(Badge, { className: online ? "bg-green-500/15 text-green-400 border-green-500/30" : "bg-red-500/15 text-red-400 border-red-500/30" },
          online ? "online" : "offline")),
      gpu.backend ? h(Badge, { className: backendTone(s && s.kind) }, gpu.backend) : null
    );

    if (!s) return h(Card, null, h(CardContent, { className: "p-0" }, head, h("div", { className: "text-sm opacity-60" }, "Connecting…")));
    if (!online) return h(Card, { className: "border-red-500/30" }, h(CardContent, { className: "p-0" }, head, h("div", { className: "text-sm text-red-400" }, "Unreachable: " + (err || "offline"))));

    const models = ollama.models || [];
    return h(Card, null, h(CardContent, { className: "p-0" },
      head,
      h("div", { className: "flex gap-4 mb-3" },
        h("div", { className: "flex-1" },
          h("div", { className: "text-xs uppercase tracking-wide opacity-60" }, "GPU util"),
          h("div", { className: "text-2xl font-semibold" }, util != null ? util + "%" : "—")),
        h("div", { className: "flex-1" },
          h("div", { className: "text-xs uppercase tracking-wide opacity-60" }, "VRAM / mem"),
          h("div", { className: "text-2xl font-semibold" }, gpu.mem_total_gb != null ? ((gpu.mem_used_gb != null ? gpu.mem_used_gb : "?") + " / " + gpu.mem_total_gb + " GB") : "—")),
        (gpu.power_w != null || tj != null) ? h("div", { className: "flex-1" },
          h("div", { className: "text-xs uppercase tracking-wide opacity-60" }, "Power / temp"),
          h("div", { className: "text-lg font-semibold" },
            (gpu.power_w != null ? gpu.power_w + " W" : "—") + (tj != null ? " · " : ""),
            tj != null ? h("span", { className: tempTone(tj) }, tj.toFixed(0) + "°C") : null)) : null
      ),
      h(Bar, { label: "GPU", frac: util != null ? util / 100 : null, value: util != null ? util + "%" : "—" }),
      h(Bar, { label: "Memory", frac: memFrac, value: gpu.mem_total_gb != null ? ((gpu.mem_used_gb != null ? gpu.mem_used_gb : "?") + " / " + gpu.mem_total_gb + " GB") : "—" }),
      h("div", { className: "mt-3" },
        h("div", { className: "flex items-center gap-2 mb-1" },
          h("span", { className: "text-xs uppercase tracking-wide opacity-60" }, "Ollama"),
          h(Badge, { className: ollama.running ? "bg-green-500/15 text-green-400 border-green-500/30" : "bg-white/10 text-white/60 border-white/20" },
            ollama.running ? ("running" + (ollama.version ? " · v" + ollama.version : "")) : "not running")),
        models.length
          ? h("div", { className: "space-y-1" }, models.map((m, i) =>
              h("div", { key: i, className: "flex justify-between text-sm font-mono" },
                h("span", { className: "opacity-90" }, m.name),
                h("span", { className: "opacity-60" },
                  (m.size_vram_gb != null ? m.size_vram_gb + " GB" : "") +
                  (m.quant ? " · " + m.quant : "") +
                  (m.context_length ? " · ctx " + m.context_length : "")))))
          : h("div", { className: "text-sm opacity-50" }, ollama.running ? "no models loaded" : "—"))
    ));
  }

  // ---- streaming log pane ------------------------------------------------
  function statusTone(st) {
    if (st == null) return "text-sky-300";
    if (st >= 500) return "text-red-400";
    if (st >= 400) return "text-yellow-400";
    return "text-green-400";
  }

  function LogStream(props) {
    const host = props.host;
    const [lines, setLines] = useState([]);
    const [live, setLive] = useState(true);
    const [err, setErr] = useState(null);
    const cursor = useRef("");
    const boxRef = useRef(null);
    const stick = useRef(true);

    // reset when host changes
    useEffect(() => { cursor.current = ""; setLines([]); setErr(null); }, [host]);

    const poll = useCallback(() => {
      SDK.fetchJSON(API + "/logs?host=" + encodeURIComponent(host) + "&cursor=" + encodeURIComponent(cursor.current))
        .then((d) => {
          if (d && d.error && (!d.lines || !d.lines.length)) { setErr(d.error); return; }
          setErr(null);
          if (d && d.cursor != null) cursor.current = String(d.cursor);
          if (d && d.lines && d.lines.length) {
            setLines((prev) => prev.concat(d.lines).slice(-500));
          }
        })
        .catch((e) => setErr(String(e && e.message ? e.message : e)));
    }, [host]);

    useEffect(() => {
      if (!live) return;
      poll();
      const t = setInterval(poll, LOG_MS);
      return () => clearInterval(t);
    }, [poll, live]);

    useEffect(() => {
      const el = boxRef.current;
      if (el && stick.current) el.scrollTop = el.scrollHeight;
    }, [lines]);

    const onScroll = () => {
      const el = boxRef.current;
      if (!el) return;
      stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
    };

    const renderLine = (ln, i) => {
      if (ln.type === "token") {
        return h("div", { key: i, className: "whitespace-pre-wrap text-cyan-300/90" },
          "  ↳ " + ln.tps + " tok/s" + (ln.n_decoded != null ? " (" + ln.n_decoded + " decoded)" : ""));
      }
      // api
      return h("div", { key: i, className: "whitespace-pre-wrap" },
        h("span", { className: "font-semibold " + statusTone(ln.status) }, (ln.status != null ? ln.status : "···")),
        h("span", { className: "opacity-50" }, ln.latency ? "  " + ln.latency : "  "),
        h("span", { className: "font-semibold opacity-90" }, "  " + (ln.method || "") + " "),
        h("span", { className: "text-violet-300" }, ln.path || ""));
    };

    return h(Card, { className: "mt-1" }, h(CardContent, { className: "p-0" },
      h("div", { className: "flex items-center justify-between mb-2" },
        h("div", { className: "text-xs uppercase tracking-wide opacity-60" }, "Ollama API stream — " + host),
        h("div", { className: "flex gap-2" },
          h("button", { className: "rounded border px-2 py-0.5 text-xs", onClick: () => setLive((v) => !v) }, live ? "Pause" : "Resume"),
          h("button", { className: "rounded border px-2 py-0.5 text-xs", onClick: () => { setLines([]); } }, "Clear"))),
      err ? h("div", { className: "text-xs text-red-400 mb-1" }, err) : null,
      h("div", {
        ref: boxRef, onScroll: onScroll,
        className: "h-72 overflow-auto rounded bg-black/40 p-2 font-mono text-xs leading-5",
      }, lines.length ? lines.map(renderLine) : h("div", { className: "opacity-40" }, "waiting for API activity…"))
    ));
  }

  // ---- discovery hint (shown when nothing configured) --------------------
  function DiscoverHint() {
    const [data, setData] = useState(null);
    const [loading, setLoading] = useState(false);
    const run = () => { setLoading(true); SDK.fetchJSON(API + "/discover").then((d) => { setData(d); setLoading(false); }).catch(() => setLoading(false)); };
    return h(Card, { className: "mb-4" }, h(CardContent, { className: "p-0" },
      h("div", { className: "flex items-center justify-between mb-2" },
        h("div", { className: "text-sm font-semibold" }, "Discover hosts"),
        h("button", { className: "rounded border px-3 py-1 text-sm", onClick: run }, loading ? "Scanning…" : "Scan my machines")),
      h("p", { className: "text-xs opacity-60 mb-2" }, "Probes localhost + every ~/.ssh/config alias for an Ollama server. Paste the suggested entries into the plugin's config.yaml."),
      data ? h("pre", { className: "rounded bg-black/40 p-2 text-xs overflow-auto" }, JSON.stringify(data.hosts, null, 2)) : null
    ));
  }

  function LocalLLMPage() {
    const [servers, setServers] = useState(null);
    const [sel, setSel] = useState(null);

    useEffect(() => {
      SDK.fetchJSON(API + "/servers")
        .then((d) => {
          const hosts = (d && d.hosts) || [];
          setServers(hosts);
          if (hosts.length && !sel) setSel(hosts[0].name);
        })
        .catch(() => setServers([]));
    }, []);

    const header = h("div", { className: "flex items-center justify-between mb-4" },
      h("div", null,
        h("h1", { className: "text-xl font-semibold" }, "Local LLM Server"),
        h("p", { className: "text-sm opacity-60" }, "GPU metrics + live Ollama API stream across your local inference servers")));

    if (!servers) return h("div", { className: "p-6" }, header, h("div", { className: "opacity-60 text-sm" }, "Loading…"));
    if (!servers.length) return h("div", { className: "p-6" }, header, DiscoverHint());

    const grid = h("div", { className: "grid grid-cols-1 lg:grid-cols-2 gap-4 mb-4" },
      servers.map((s) => h(ServerCard, { key: s.name, name: s.name })));

    const selector = servers.length > 1 ? h("div", { className: "flex items-center gap-2 mb-2" },
      h("span", { className: "text-xs uppercase tracking-wide opacity-60" }, "Log host:"),
      servers.map((s) => h("button", {
        key: s.name,
        className: "rounded border px-3 py-1 text-sm " + (sel === s.name ? "bg-white/15 border-white/40" : "opacity-70"),
        onClick: () => setSel(s.name),
      }, s.name))) : null;

    return h("div", { className: "p-6" }, header, grid, selector, sel ? h(LogStream, { host: sel, key: sel }) : null);
  }

  if (window.__HERMES_PLUGINS__ && typeof window.__HERMES_PLUGINS__.register === "function") {
    window.__HERMES_PLUGINS__.register("local-llm-server", LocalLLMPage);
  }
})();
