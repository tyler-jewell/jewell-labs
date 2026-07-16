/** UI bridge: settings-overview (health) + test_api action. */

import type { PluginContext } from "@paperclipai/plugin-sdk";
import { TOOL_NAMES, type ToolName } from "../tools.js";
import { executeToolForHost } from "../worker-logic.js";
import { fetchFromCtx, loadResolvedSecrets } from "./config-load.js";
import { probePluginHealth } from "./health.js";

export function registerUiBridge(ctx: PluginContext): void {
  ctx.data.register("settings-overview", async () => {
    return probePluginHealth(ctx);
  });

  ctx.actions.register("test_api", async (params) => {
    const apiRaw = params.api;
    if (typeof apiRaw !== "string" || !apiRaw.trim()) {
      throw new Error("test_api requires params.api (tool name)");
    }
    const api = apiRaw.trim() as ToolName;
    if (!(TOOL_NAMES as readonly string[]).includes(api)) {
      throw new Error(
        `unknown API "${api}"; expected one of: ${TOOL_NAMES.join(", ")}`,
      );
    }
    const toolParams =
      typeof params.params === "object" &&
      params.params !== null &&
      !Array.isArray(params.params)
        ? (params.params as Record<string, unknown>)
        : {};

    const resolved = await loadResolvedSecrets(ctx);
    if (!resolved.ok) {
      throw new Error(
        `config error: ${resolved.errors.join("; ")}${
          resolved.warnings.length
            ? ` (warnings: ${resolved.warnings.join("; ")})`
            : ""
        }`,
      );
    }

    const out = await executeToolForHost(
      api,
      toolParams,
      resolved.secrets,
      fetchFromCtx(ctx),
    );
    if (!out.ok) {
      throw new Error(out.error ?? `${api} failed`);
    }
    return {
      ok: true,
      api,
      result: out.result ?? null,
    };
  });
}
