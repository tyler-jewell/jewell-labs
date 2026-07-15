#!/usr/bin/env node
// Repeatable integration test: Paperclip DIRECT adapter (gateway_openai) -> llm-provider.
// NO hermes, NO agent CLI spawn — our adapter's execute(ctx) POSTs straight to the
// gateway's /v1/chat/completions, streaming deltas back through ctx.onLog, exactly the
// way a real Paperclip run drives an adapter. It also loads the adapter through
// Paperclip's OWN plugin loader to prove the package is server-registerable.
//
// The gateway runs trust_loopback=false (exposed via tunnel), so a key IS required. The key is
// resolved the same way the adapter resolves it: from the creds bundle's access_token
// (credsFile), falling back to a static apiKeyFile. (Remote 401 is covered by the Rust unit
// test identity::is_authorized and the oauth_integration deny path.)
//
// Run:  node test-gateway-direct.mjs      Exit: 0 all pass, 1 any failure.

import { execSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { execute, buildMessages, type, createServerAdapter } from "./gateway-adapter/index.mjs";

const DIR = dirname(fileURLToPath(import.meta.url));
const GATEWAY = process.env.GATEWAY_URL || "http://localhost:4141";
const PKG_DIR = join(DIR, "gateway-adapter");
const CFG = JSON.parse(readFileSync(join(DIR, "adapter-config.json"), "utf8"));
// GATEWAY_URL lets a run target an alternate port. Resolve the bearer the same way the adapter
// does — prefer the creds bundle's access_token, fall back to a static apiKeyFile — and never
// crash if neither exists on disk (KEY stays "" and the key checks report the failure).
const AC = CFG.adapterConfig;
const baseConfig = { ...AC, baseUrl: `${GATEWAY}/v1` };
const KEY = (() => {
  try {
    if (AC.credsFile) return JSON.parse(readFileSync(AC.credsFile, "utf8")).access_token || "";
    if (AC.apiKeyFile) return readFileSync(AC.apiKeyFile, "utf8").trim();
  } catch { /* no creds on disk */ }
  return "";
})();

const serverDist = execSync(
  "ls -d ~/.npm/_npx/*/node_modules/@paperclipai/server/dist 2>/dev/null | head -1",
  { shell: "/bin/bash", encoding: "utf8" }
).trim();

let pass = 0, fail = 0;
const ok = (m) => { console.log(`  \x1b[32mPASS\x1b[0m ${m}`); pass++; };
const bad = (m) => { console.log(`  \x1b[31mFAIL\x1b[0m ${m}`); fail++; };
async function section(name, fn) {
  console.log(`\n▸ ${name}`);
  try { await fn(); } catch (e) { bad(`${name}: ${e.message}`); }
}

function makeCtx(model, context) {
  const out = { stdout: "", stderr: "", meta: null };
  return {
    ctx: {
      runId: "test-run",
      agent: { name: "Direct Tester" },
      config: { ...baseConfig, model },
      context,
      onLog: (ch, txt) => { out[ch] = (out[ch] || "") + txt; },
      onMeta: (m) => { out.meta = m; },
    },
    out,
  };
}

// 0. The adapter is the direct (no-hermes) one, and it prompts from ctx.context.
await section("adapter is direct gateway_openai (no hermes)", async () => {
  type === "gateway_openai" ? ok(`adapter type is ${type}`) : bad(`unexpected type ${type}`);
  CFG.adapterType === "gateway_openai"
    ? ok("adapter-config.json wired to gateway_openai")
    : bad(`config adapterType is ${CFG.adapterType}, expected gateway_openai`);
  const msgs = buildMessages({ name: "Ada" }, { issueTitle: "Ship it", issueBody: "Do the thing" }, {});
  msgs[0].role === "system" && /Ship it/.test(msgs[1].content) && /Do the thing/.test(msgs[1].content)
    ? ok("buildMessages turns ctx.context into a chat turn")
    : bad(`buildMessages wrong: ${JSON.stringify(msgs)}`);
});

// 1. Paperclip's OWN plugin loader loads our package and yields a valid adapter.
await section("paperclip's plugin loader accepts the package", async () => {
  if (!serverDist) return bad("paperclip server dist not found in npx cache");
  const loader = await import(`${serverDist}/adapters/plugin-loader.js`);
  const mod = await loader.loadExternalAdapterPackage("paperclip-gateway-openai-adapter", PKG_DIR);
  mod.type === "gateway_openai" ? ok(`loadExternalAdapterPackage -> type ${mod.type}`) : bad(`loaded type ${mod.type}`);
  typeof mod.execute === "function" ? ok("loaded module exposes execute()") : bad("no execute()");
  typeof mod.testEnvironment === "function" ? ok("loaded module exposes testEnvironment()") : bad("no testEnvironment()");
  const built = createServerAdapter();
  built.type === "gateway_openai" && typeof built.execute === "function"
    ? ok("createServerAdapter() returns a ServerAdapterModule") : bad("createServerAdapter invalid");
});

// 2. Gateway is a live multi-model provider; requires a key (trust_loopback=false).
await section("gateway is a live multi-model provider (key required)", async () => {
  const h = await fetch(`${GATEWAY}/healthz`).then(r => r.json());
  h.ok ? ok("healthz ok") : bad("healthz not ok");
  const nokey = await fetch(`${GATEWAY}/v1/models`).then(r => r.status);
  nokey === 401 ? ok("keyless request rejected (401)") : bad(`expected 401, got ${nokey}`);
  const res = await fetch(`${GATEWAY}/v1/models`, { headers: { Authorization: `Bearer ${KEY}` } });
  res.status === 200 ? ok("valid key accepted (200)") : bad(`expected 200, got ${res.status}`);
  const models = await res.json();
  const owners = new Set(models.data.map(m => m.owned_by));
  owners.has("xai") ? ok("grok (xai) models present") : bad("no xai models");
  [...owners].some(o => o === "ollama" || o === "llama-cpp")
    ? ok(`local models present (${[...owners].filter(o => o==="ollama"||o==="llama-cpp").join(",")})`)
    : bad("no local models");
});

// 3. testEnvironment() reports healthy against the gateway.
await section("adapter testEnvironment passes", async () => {
  const env = await createServerAdapter().testEnvironment({ adapterType: "gateway_openai", config: baseConfig });
  env.status === "pass" ? ok(`testEnvironment -> ${env.status}`) : bad(`testEnvironment -> ${env.status}: ${JSON.stringify(env.checks)}`);
});

// 4. Drive each model through the REAL adapter execute(ctx) — the exact Paperclip path.
async function runThrough(model, context) {
  const { ctx, out } = makeCtx(model, context);
  const res = await execute(ctx);
  return { res, out };
}
await section("models answer through the direct adapter execute(ctx)", async () => {
  const g = await runThrough("grok-4.20-0309-non-reasoning",
    { issueTitle: "Health check", issueBody: "Reply with exactly: grok direct ok" });
  g.res.exitCode === 0 && /grok direct ok/i.test(g.out.stdout)
    ? ok(`grok answered via onLog (meta model=${g.out.meta?.model})`)
    : bad(`grok unexpected: exit=${g.res.exitCode} out=${g.out.stdout.slice(-80)} err=${g.out.stderr.slice(-80)}`);

  const q = await runThrough("qwen2.5:7b",
    { issueTitle: "Math", issueBody: "What is 6+6? Reply with the number only." });
  q.res.exitCode === 0 && /\b12\b/.test(q.out.stdout)
    ? ok("qwen (local) answered 12 via onLog")
    : bad(`qwen unexpected: exit=${q.res.exitCode} out=${q.out.stdout.slice(-80)} err=${q.out.stderr.slice(-80)}`);

  const l = await runThrough("llama3.2:1b", { issueBody: "Say hello." });
  l.res.exitCode === 0 && l.out.stdout.length > 0
    ? ok("llama (local) reachable and streaming")
    : bad(`llama unexpected: exit=${l.res.exitCode} out=${l.out.stdout.slice(-80)} err=${l.out.stderr.slice(-80)}`);
});

console.log(`\n${fail === 0 ? "\x1b[32m✔" : "\x1b[31m✘"} ${pass} passed, ${fail} failed\x1b[0m`);
process.exit(fail === 0 ? 0 : 1);
