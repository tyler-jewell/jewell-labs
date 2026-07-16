/** Execute a tool name against live SendBlue via pure builders. */

import {
  allowlistPolicyFromConfig,
  credentialsFromConfig,
  validateConfig,
} from "../config.js";
import {
  executePrepared,
  parseLinesResponse,
  parseMessagesResponse,
  parseSendMessageResponse,
} from "../sendblue-api.js";
import { buildToolRequest } from "../tools.js";
import type { ResolvedSecrets } from "./resolved.js";

export async function executeToolForHost(
  name: string,
  params: Record<string, unknown>,
  secrets: ResolvedSecrets,
  fetchImpl: typeof fetch = fetch,
): Promise<{ ok: boolean; result?: unknown; error?: string }> {
  const v = validateConfig(
    {
      apiKey: secrets.apiKey,
      apiSecret: secrets.apiSecret,
      fromNumber: secrets.fromNumber,
      allowlist: secrets.allowlist,
      emptyMeansDeny: secrets.emptyMeansDeny,
    },
    { requireCredentials: true },
  );
  if (!v.ok) {
    return { ok: false, error: v.errors.join("; ") };
  }
  const creds = credentialsFromConfig(v.config);
  const built = buildToolRequest(name, params, {
    creds,
    fromNumber: v.config.fromNumber,
    allowlist: allowlistPolicyFromConfig(v.config),
  });
  if (!built.ok || !built.request) {
    return { ok: false, error: built.error ?? "build failed" };
  }
  try {
    const json = await executePrepared(built.request, fetchImpl);
    if (name === "list_lines") {
      return { ok: true, result: parseLinesResponse(json) };
    }
    if (name === "list_messages") {
      return { ok: true, result: parseMessagesResponse(json) };
    }
    if (
      name === "send_message" ||
      name === "send_group_message" ||
      name === "create_group"
    ) {
      return { ok: true, result: parseSendMessageResponse(json) };
    }
    return { ok: true, result: json };
  } catch (e) {
    return {
      ok: false,
      error: e instanceof Error ? e.message : String(e),
    };
  }
}
