#!/usr/bin/env bun
/** Minimal agent runner (Bun): agents/{name}.md → llama-server chat. */

import { readFileSync, existsSync, openSync } from "fs";
import { join, dirname } from "path";
import { spawn } from "bun";
import { fileURLToPath } from "url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const AGENTS = join(ROOT, "agents");
const REGISTRY = join(ROOT, "models", "registry.yaml");

type Dict = Record<string, any>;

function parseFrontmatter(text: string): [Dict, string] {
  if (!text.startsWith("---")) return [{}, text];
  const parts = text.split("---");
  if (parts.length < 3) return [{}, text];
  const rawFm = parts[1];
  const body = parts.slice(2).join("---").replace(/^\n/, "");
  const data: Dict = {};
  const stack: [number, Dict][] = [[0, data]];
  for (const line of rawFm.split("\n")) {
    if (!line.trim() || line.trim().startsWith("#")) continue;
    const indent = line.length - line.trimStart().length;
    const m = line.match(/^(\s*)([A-Za-z0-9_]+):\s*(.*)$/);
    if (!m) continue;
    const key = m[2];
    let val: any = m[3].trim();
    while (stack.length > 1 && indent <= stack[stack.length - 1][0]) stack.pop();
    const cur = stack[stack.length - 1][1];
    if (val === "") {
      cur[key] = {};
      stack.push([indent, cur[key]]);
    } else {
      if (
        (val.startsWith('"') && val.endsWith('"')) ||
        (val.startsWith("'") && val.endsWith("'"))
      )
        val = val.slice(1, -1);
      else if (val === "true" || val === "false") val = val === "true";
      else if (/^-?\d+$/.test(val)) val = parseInt(val, 10);
      else if (/^-?\d+\.\d+$/.test(val)) val = parseFloat(val);
      else if (val === "null") val = null;
      cur[key] = val;
    }
  }
  return [data, body];
}

function loadRegistry(): Dict {
  const text = readFileSync(REGISTRY, "utf8");
  const models: Dict = {};
  let cur: string | null = null;
  let inDefaults = false;
  for (const line of text.split("\n")) {
    let m = line.match(/^  ([A-Za-z0-9_.-]+):\s*$/);
    if (m) {
      cur = m[1];
      models[cur] = {};
      inDefaults = false;
      continue;
    }
    if (!cur) continue;
    if (/^    defaults:\s*$/.test(line)) {
      models[cur].defaults = {};
      inDefaults = true;
      continue;
    }
    m = line.match(/^    ([A-Za-z0-9_]+):\s*(.+)$/);
    if (m && !inDefaults) {
      models[cur][m[1]] = m[2].trim().replace(/^["']|["']$/g, "");
      continue;
    }
    m = line.match(/^      ([A-Za-z0-9_]+):\s*(.+)$/);
    if (m && inDefaults) {
      let v: any = m[2].trim().replace(/^["']|["']$/g, "");
      if (/^-?\d+$/.test(v)) v = parseInt(v, 10);
      models[cur].defaults[m[1]] = v;
    }
  }
  return models;
}

function expandHome(p: string): string {
  if (p.startsWith("~/")) return join(process.env.HOME || "", p.slice(2));
  return p;
}

function resolveModel(key: string, registry: Dict): Dict {
  if (registry[key]) {
    const spec = { ...registry[key], key };
    spec.path = expandHome(spec.path || "");
    return spec;
  }
  const path = expandHome(key);
  if (path.endsWith(".gguf") && existsSync(path)) {
    return { key, path, alias: path.split("/").pop()?.replace(/\.gguf$/, ""), defaults: {} };
  }
  throw new Error(`Unknown model '${key}'`);
}

async function serverUp(base: string): Promise<boolean> {
  try {
    const r = await fetch(`${base}/v1/models`, { signal: AbortSignal.timeout(1000) });
    return r.ok;
  } catch {
    return false;
  }
}

async function ensureServer(
  path: string,
  host: string,
  port: number,
  ctx: number,
  reasoning: string
): Promise<string> {
  const base = `http://${host}:${port}`;
  if (await serverUp(base)) return base;
  const log = openSync("/tmp/run-agent-llama.log", "w");
  spawn({
    cmd: [
      "llama-server",
      "-m",
      path,
      "--host",
      host,
      "--port",
      String(port),
      "-c",
      String(ctx),
      "-np",
      "1",
      "--ctx-checkpoints",
      "0",
      "--reasoning",
      reasoning,
    ],
    stdout: log,
    stderr: log,
    stdin: "ignore",
  });
  for (let i = 0; i < 60; i++) {
    if (await serverUp(base)) return base;
    await Bun.sleep(500);
  }
  throw new Error("llama-server failed to become ready");
}

async function chat(
  base: string,
  model: string,
  system: string,
  user: string,
  sampling: Dict
): Promise<string> {
  const body: Dict = {
    model,
    messages: [
      { role: "system", content: system },
      { role: "user", content: user },
    ],
    temperature: sampling.temperature ?? 0,
    max_tokens: sampling.max_tokens ?? 256,
  };
  if (sampling.seed != null) body.seed = sampling.seed;
  const r = await fetch(`${base}/v1/chat/completions`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!r.ok) throw new Error(`chat failed: ${r.status} ${await r.text()}`);
  const data = await r.json();
  const msg = data.choices[0].message;
  return msg.content || msg.reasoning_content || "";
}

async function main() {
  const argv = process.argv.slice(2);
  let dryParse = false;
  let serveOnly = false;
  let modelOverride: string | null = null;
  const positional: string[] = [];
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--dry-parse") dryParse = true;
    else if (a === "--serve-only") serveOnly = true;
    else if (a === "--model") {
      modelOverride = argv[++i] ?? null;
    } else if (a.startsWith("-")) {
      console.error(`unknown flag: ${a}`);
      process.exit(1);
    } else positional.push(a);
  }
  const agent = positional[0];
  const prompt =
    positional.length > 1
      ? positional.slice(1).join(" ")
      : "What is 2+2? Put answer in \\boxed{}.";
  if (!agent) {
    console.error("usage: run_agent.ts <agent> [prompt] [--dry-parse] [--serve-only] [--model KEY]");
    process.exit(1);
  }
  const agentPath = join(AGENTS, `${agent}.md`);
  if (!existsSync(agentPath)) throw new Error(`missing ${agentPath}`);
  const [fm, body] = parseFrontmatter(readFileSync(agentPath, "utf8"));
  const modelKey = modelOverride || fm.model || fm.default_model;
  if (!modelKey) throw new Error("missing default_model");
  const registry = loadRegistry();
  const spec = resolveModel(String(modelKey), registry);
  const server = fm.server || {};
  const sampling = fm.sampling || {};
  const host = server.host || "127.0.0.1";
  const port = Number(server.port || 8080);
  const ctx = Number(server.ctx ?? spec.defaults?.ctx ?? 2048);
  const reasoning = String(server.reasoning ?? spec.defaults?.reasoning ?? "off");

  if (dryParse) {
    console.log(
      JSON.stringify({
        agent,
        model_key: modelKey,
        path: spec.path,
        system_chars: body.length,
        port,
      })
    );
    return;
  }

  const base = await ensureServer(spec.path, host, port, ctx, reasoning);
  if (serveOnly) {
    console.log(base);
    return;
  }
  const out = await chat(base, spec.alias || "local", body, prompt, sampling);
  console.log(out);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
