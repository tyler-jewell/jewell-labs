/**
 * Plugin system health: credentials + soft config + board routing readiness.
 * Company/agent UUIDs are never plugin config — resolved live from the board.
 */

import type { PluginContext } from "@paperclipai/plugin-sdk";
import { validateConfig } from "../config.js";
import { PLUGIN_ID, PLUGIN_VERSION, SECRET_KEYS } from "../constants.js";
import { loadResolvedSecrets, readRawConfig } from "./config-load.js";
import { resolveCompanyAndAgent } from "./conversation-io.js";

export type PluginHealthReport = {
  status: "ok" | "degraded" | "error";
  pluginId: string;
  version: string;
  /** SendBlue API credentials resolvable (plain / env / ops files / vault). */
  credentialsConfigured: boolean;
  secrets: {
    apiKey: boolean;
    apiSecret: boolean;
    webhookSecret: boolean;
    /** Canonical vault key names (optional when env/files provision credentials). */
    expectedVaultKeys: typeof SECRET_KEYS;
  };
  hasFromNumber: boolean;
  allowlistCount: number;
  emptyMeansDeny: boolean;
  inboundMode: string;
  /** Live board resolve — not stored in plugin config. */
  boardRouting: {
    ok: boolean;
    companyId?: string;
    projectId?: string;
    assigneeAgentId?: string;
  };
  warnings: string[];
  errors: string[];
};

export async function probePluginHealth(
  ctx: PluginContext,
): Promise<PluginHealthReport> {
  const raw = await readRawConfig(ctx);
  const soft = validateConfig(raw, { requireCredentials: false });
  const full = await loadResolvedSecrets(ctx);
  const warnings: string[] = [];
  const errors: string[] = [];

  const credentialsConfigured = full.ok;
  if (!soft.ok) {
    errors.push(...soft.errors);
  } else {
    // Soft config cannot see vault/file secrets — drop credential noise when full resolve ok.
    for (const w of soft.warnings) {
      if (credentialsConfigured && /no SendBlue credentials/i.test(w)) continue;
      warnings.push(w);
    }
  }
  if (!full.ok) {
    errors.push(...full.errors);
    warnings.push(...full.warnings);
  } else {
    warnings.push(...full.warnings);
  }

  let boardRouting: PluginHealthReport["boardRouting"] = { ok: false };
  try {
    const r = await resolveCompanyAndAgent(ctx);
    boardRouting = {
      ok: Boolean(r.companyId),
      companyId: r.companyId,
      projectId: r.projectId,
      assigneeAgentId: r.assigneeAgentId,
    };
    if (!r.companyId) {
      warnings.push(
        "board routing: no company resolved (board REST empty or board API key missing)",
      );
    }
  } catch (err) {
    warnings.push(`board routing probe failed: ${String(err)}`);
  }

  const inboundMode = soft.ok ? soft.config.inboundMode : "create_issue";
  if (inboundMode === "create_issue" && !boardRouting.ok) {
    warnings.push(
      "inboundMode=create_issue but board cannot resolve a company yet",
    );
  }

  let status: PluginHealthReport["status"] = "ok";
  if (errors.length) status = "error";
  else if (!credentialsConfigured || warnings.length) status = "degraded";
  if (credentialsConfigured && boardRouting.ok && errors.length === 0) {
    status = warnings.length ? "degraded" : "ok";
  }

  return {
    status,
    pluginId: PLUGIN_ID,
    version: PLUGIN_VERSION,
    credentialsConfigured,
    secrets: {
      apiKey: full.ok && Boolean(full.secrets.apiKey),
      apiSecret: full.ok && Boolean(full.secrets.apiSecret),
      webhookSecret: full.ok
        ? Boolean(full.secrets.webhookSecret)
        : false,
      expectedVaultKeys: SECRET_KEYS,
    },
    hasFromNumber: soft.ok
      ? Boolean(soft.config.fromNumber ?? (full.ok && full.secrets.fromNumber))
      : full.ok && Boolean(full.secrets.fromNumber),
    allowlistCount: soft.ok ? soft.config.allowlist.length : 0,
    emptyMeansDeny: soft.ok ? soft.config.emptyMeansDeny : true,
    inboundMode,
    boardRouting,
    warnings: [...new Set(warnings)],
    errors: [...new Set(errors)],
  };
}
