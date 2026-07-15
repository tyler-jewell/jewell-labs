// Paperclip external adapter: "gateway_openai".
//
// Talks DIRECTLY to the local llm-provider's OpenAI-compatible endpoint
// (/v1/chat/completions) — NO hermes, no agent CLI in the mix. Loadable by a
// running Paperclip server via the adapter-plugin-store (see ../register-adapter.mjs).
//
// llm-provider trusts loopback (127.0.0.1), so on this Mac NO api key is needed. A key is
// only required if you point baseUrl at the gateway remotely; set apiKey/apiKeyFile then.

import { readFileSync } from "node:fs";

export const type = "gateway_openai";
export const label = "LLM Provider (OpenAI-compatible, direct)";

export const models = [
  { id: "grok-4.20-0309-non-reasoning", label: "Grok 4.20 (xAI)" },
  { id: "qwen2.5:7b", label: "Qwen2.5 7B (local)" },
  { id: "llama3.2:1b", label: "Llama 3.2 1B (local)" },
];

export const agentConfigurationDoc = `# gateway_openai agent configuration

Adapter: gateway_openai — call the local llm-provider directly (no intermediary CLI).

Fields:
- baseUrl (string, optional): OpenAI-compatible base URL. Default http://localhost:4141/v1
- model (string, optional): model id from the gateway's /v1/models
- apiKey / apiKeyFile (string, optional): bearer key — only needed for REMOTE (non-loopback) access
- systemPrompt (string, optional): system message prepended to the turn
- maxTokens (number, optional): output cap (default 4096)
- timeoutSec (number, optional): request timeout (default 180)
`;

function cfgStr(v, d = "") { return typeof v === "string" && v ? v : d; }
function cfgNum(v, d) { return typeof v === "number" && Number.isFinite(v) ? v : d; }

function resolveApiKey(config) {
  if (cfgStr(config.apiKey)) return config.apiKey;
  const file = cfgStr(config.apiKeyFile);
  if (file) { try { return readFileSync(file, "utf8").trim(); } catch { return ""; } }
  return ""; // loopback needs none
}

export function buildMessages(agent = {}, context = {}, config = {}) {
  const system = cfgStr(config.systemPrompt) ||
    `You are "${cfgStr(agent.name, "an agent")}", an AI agent in a Paperclip-managed company. ` +
    `Respond directly and concisely to the task.`;
  const parts = [];
  const title = cfgStr(context.issueTitle) || cfgStr(context.title);
  const body = cfgStr(context.issueBody) || cfgStr(context.prompt) || cfgStr(context.text);
  if (title) parts.push(`Task: ${title}`);
  if (body) parts.push(body);
  const user = parts.join("\n\n") || "Introduce yourself and state you are ready to work.";
  return [
    { role: "system", content: system },
    { role: "user", content: user },
  ];
}

export async function execute(ctx) {
  const { agent = {}, config = {}, context = {}, onLog = () => {}, onMeta } = ctx || {};
  const baseUrl = cfgStr(config.baseUrl, "http://localhost:4141/v1").replace(/\/$/, "");
  const model = cfgStr(config.model, "grok-4.20-0309-non-reasoning");
  const apiKey = resolveApiKey(config);
  const maxTokens = cfgNum(config.maxTokens, 4096);
  const timeoutSec = cfgNum(config.timeoutSec, 180);
  const messages = buildMessages(agent, context, config);

  if (onMeta) await onMeta({ adapterType: type, model, baseUrl });

  const headers = { "Content-Type": "application/json" };
  if (apiKey) headers.Authorization = `Bearer ${apiKey}`;

  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutSec * 1000);
  try {
    const resp = await fetch(`${baseUrl}/chat/completions`, {
      method: "POST",
      headers,
      body: JSON.stringify({ model, messages, max_tokens: maxTokens, stream: true }),
      signal: controller.signal,
    });
    if (!resp.ok) {
      const errText = await resp.text().catch(() => `HTTP ${resp.status}`);
      onLog("stderr", `gateway_openai: upstream ${resp.status}: ${errText}\n`);
      return { exitCode: 1, signal: null, timedOut: false, errorMessage: `gateway HTTP ${resp.status}`, errorCode: "upstream" };
    }
    const reader = resp.body.getReader();
    const decoder = new TextDecoder();
    let buf = "", produced = "";
    for (;;) {
      const { value, done } = await reader.read();
      if (done) break;
      buf += decoder.decode(value, { stream: true });
      let nl;
      while ((nl = buf.indexOf("\n")) >= 0) {
        const line = buf.slice(0, nl).trim();
        buf = buf.slice(nl + 1);
        if (!line.startsWith("data:")) continue;
        const data = line.slice(5).trim();
        if (data === "[DONE]") continue;
        let ev; try { ev = JSON.parse(data); } catch { continue; }
        const delta = ev?.choices?.[0]?.delta?.content;
        if (delta) { produced += delta; onLog("stdout", delta); }
      }
    }
    if (!produced.trim()) {
      onLog("stderr", "gateway_openai: empty response\n");
      return { exitCode: 1, signal: null, timedOut: false, errorMessage: "empty response", errorCode: "empty" };
    }
    return { exitCode: 0, signal: null, timedOut: false, summary: `gateway_openai ${model}`, output: produced };
  } catch (e) {
    const timedOut = e.name === "AbortError";
    onLog("stderr", `gateway_openai: ${timedOut ? "timeout" : e.message}\n`);
    return {
      exitCode: timedOut ? null : 1,
      signal: null,
      timedOut,
      errorMessage: timedOut ? `timed out after ${timeoutSec}s` : String(e.message || e),
      errorCode: timedOut ? "timeout" : "error",
    };
  } finally {
    clearTimeout(timer);
  }
}

export async function testEnvironment(ctx) {
  const config = ctx?.config || {};
  const checks = [];
  const baseUrl = cfgStr(config.baseUrl, "http://localhost:4141/v1").replace(/\/$/, "");
  const apiKey = resolveApiKey(config);
  checks.push({
    code: "gateway_auth_mode", level: "info",
    message: apiKey ? "Using a bearer key." : "Loopback (no key) — llm-provider trusts 127.0.0.1.",
  });
  try {
    const ctrl = new AbortController();
    const t = setTimeout(() => ctrl.abort(), 3000);
    const r = await fetch(`${baseUrl}/models`, {
      headers: apiKey ? { Authorization: `Bearer ${apiKey}` } : {},
      signal: ctrl.signal,
    });
    clearTimeout(t);
    if (r.ok) {
      const j = await r.json().catch(() => ({}));
      const n = Array.isArray(j?.data) ? j.data.length : 0;
      checks.push({ code: "gateway_reachable", level: "info", message: `Gateway reachable at ${baseUrl} (${n} models).` });
    } else if (r.status === 401) {
      checks.push({ code: "gateway_unauthorized", level: "error", message: "Gateway rejected the key (401)." });
    } else {
      checks.push({ code: "gateway_bad_status", level: "warn", message: `Gateway returned HTTP ${r.status}.` });
    }
  } catch (e) {
    checks.push({ code: "gateway_unreachable", level: "error",
      message: e.name === "AbortError" ? `Gateway probe timed out (${baseUrl}).` : String(e.message || e),
      hint: "Is the llm-provider service running on :4141?" });
  }
  const status = checks.some((c) => c.level === "error") ? "fail"
    : checks.some((c) => c.level === "warn") ? "warn" : "pass";
  return { adapterType: ctx?.adapterType || type, status, checks, testedAt: new Date().toISOString() };
}

export function createServerAdapter() {
  return { type, label, models, agentConfigurationDoc, execute, testEnvironment };
}

export default { type, label, models, agentConfigurationDoc, buildMessages, execute, testEnvironment, createServerAdapter };
