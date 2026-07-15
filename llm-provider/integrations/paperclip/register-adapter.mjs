#!/usr/bin/env node
// Register (or --remove) the direct gateway_openai adapter with a Paperclip
// instance, using Paperclip's OWN adapter-plugin-store. After registering,
// start the server (npx paperclipai run) and "gateway_openai" is a selectable
// adapter type — an agent using it calls the llm-gateway directly, no hermes.
//
//   node register-adapter.mjs            # register
//   node register-adapter.mjs --remove   # unregister
//   PAPERCLIP_HOME=~/some-instance node register-adapter.mjs   # isolated instance
//
// Writes to $PAPERCLIP_HOME/adapter-plugins.json (default ~/.paperclip).

import { execSync } from "node:child_process";

const HOME = process.env.HOME;
const PKG_DIR = `${HOME}/Apps/jewell-labs/llm-provider/integrations/paperclip/gateway-adapter`;
const serverDist = execSync(
  "ls -d ~/.npm/_npx/*/node_modules/@paperclipai/server/dist 2>/dev/null | head -1",
  { shell: "/bin/bash", encoding: "utf8" }
).trim();
if (!serverDist) { console.error("paperclip not found in npx cache; run `npx paperclipai` once first"); process.exit(1); }

const store = await import(`${serverDist}/services/adapter-plugin-store.js`);
const remove = process.argv.includes("--remove");

if (remove) {
  const ok = store.removeAdapterPlugin("gateway_openai");
  console.log(ok ? "removed gateway_openai" : "gateway_openai was not registered");
} else {
  store.addAdapterPlugin({
    type: "gateway_openai",
    packageName: "paperclip-gateway-openai-adapter",
    localPath: PKG_DIR,
    label: "LLM Gateway (OpenAI-compatible, direct)",
  });
  const home = process.env.PAPERCLIP_HOME || `${HOME}/.paperclip`;
  console.log(`registered gateway_openai -> ${PKG_DIR}`);
  console.log(`store: ${home}/adapter-plugins.json`);
}
