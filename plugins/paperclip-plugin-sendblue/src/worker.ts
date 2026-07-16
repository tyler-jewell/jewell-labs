/**
 * Paperclip worker entry — definePlugin + runWorker.
 * Pure logic lives in worker-logic / runtime so tests never start a worker.
 */

import {
  definePlugin,
  runWorker,
  type PluginContext,
  type PluginWebhookInput,
} from "@paperclipai/plugin-sdk";
import { validateConfig } from "./config.js";
import { PLUGIN_ID, PLUGIN_VERSION } from "./constants.js";
import { probePluginHealth } from "./worker/health.js";
import { handlePluginWebhook } from "./worker/on-webhook.js";
import { registerUiBridge } from "./worker/register-bridge.js";
import { registerToolsAndEvents } from "./worker/register-tools.js";

/** Set in setup; onWebhook runs outside setup and needs host services. */
let runtimeCtx: PluginContext | null = null;

const plugin = definePlugin({
  async setup(ctx: PluginContext) {
    runtimeCtx = ctx;
    ctx.logger.info(`${PLUGIN_ID} v${PLUGIN_VERSION} setup`);
    registerToolsAndEvents(ctx);
    registerUiBridge(ctx);
    ctx.logger.info("SendBlue plugin tools + event handlers registered");
  },

  async onHealth() {
    if (!runtimeCtx) {
      return {
        status: "error" as const,
        message: `${PLUGIN_ID}@${PLUGIN_VERSION} not ready`,
      };
    }
    const h = await probePluginHealth(runtimeCtx);
    const status =
      h.status === "ok"
        ? ("ok" as const)
        : h.status === "degraded"
          ? ("degraded" as const)
          : ("error" as const);
    return {
      status,
      message: `${PLUGIN_ID}@${PLUGIN_VERSION} creds=${h.credentialsConfigured} routing=${h.boardRouting.ok}`,
      details: h,
    };
  },

  async onValidateConfig(config: Record<string, unknown>) {
    const v = validateConfig(config, { requireCredentials: false });
    if (!v.ok) {
      return { ok: false, errors: v.errors, warnings: v.warnings };
    }
    return { ok: true, warnings: v.warnings };
  },

  async onConfigChanged(_newConfig: Record<string, unknown>) {
    // Config is re-read on each tool/webhook/event; no restart needed.
  },

  async onWebhook(input: PluginWebhookInput) {
    await handlePluginWebhook(runtimeCtx, input);
  },
});

export default plugin;
runWorker(plugin, import.meta.url);
