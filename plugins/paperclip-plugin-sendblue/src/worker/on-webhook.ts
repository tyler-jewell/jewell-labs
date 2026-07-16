/** onWebhook handler: verify, allowlist, side effects. */

import type { PluginContext, PluginWebhookInput } from "@paperclipai/plugin-sdk";
import { validateConfig } from "../config.js";
import { SECRET_KEYS, WEBHOOK_ENDPOINT_KEYS } from "../constants.js";
import { SENDBLUE_WEBHOOK_SECRET_PATHS } from "../ops-paths.js";
import {
  processInboundWebhook,
  resolvedFromValidated,
  type ResolvedSecrets,
} from "../worker-logic.js";
import {
  loadResolvedSecrets,
  readRawConfig,
  resolveSecret,
} from "./config-load.js";
import { applyInboundSideEffects } from "./inbound-effects.js";

function tryParse(raw: string | undefined): unknown {
  if (!raw) return undefined;
  try {
    return JSON.parse(raw);
  } catch {
    return raw;
  }
}

export async function handlePluginWebhook(
  runtimeCtx: PluginContext | null,
  input: PluginWebhookInput,
): Promise<void> {
  const ctx = runtimeCtx;
  if (!ctx) {
    throw new Error("SendBlue plugin not ready (setup incomplete)");
  }

  const raw = await readRawConfig(ctx);
  const soft = validateConfig(raw, { requireCredentials: false });
  const webhookSecret = soft.ok
    ? await resolveSecret(
        ctx,
        soft.config.webhookSecretRef,
        soft.config.webhookSecret,
        {
          canonicalKey: SECRET_KEYS.webhookSecret,
          envNames: ["SENDBLUE_WEBHOOK_SECRET"],
          filePaths: SENDBLUE_WEBHOOK_SECRET_PATHS,
        },
      )
    : undefined;

  const allowlist = soft.ok ? soft.config.allowlist : [];
  const emptyMeansDeny = soft.ok ? soft.config.emptyMeansDeny : true;

  const handled = processInboundWebhook({
    secrets: {
      webhookSecret,
      allowlist,
      emptyMeansDeny,
    },
    headers: input.headers,
    rawBody: input.rawBody,
    parsedBody: input.parsedBody ?? tryParse(input.rawBody),
  });

  if (handled.status === 401) {
    throw new Error(handled.body.error ?? "webhook auth failed");
  }
  if (handled.status === 403) {
    ctx.logger.warn("Inbound sender not on allowlist", {
      error: handled.body.error,
      endpointKey: input.endpointKey,
    });
    return;
  }

  // Full resolver merges vault/env/file credentials + routing IDs.
  let secrets: ResolvedSecrets | null = null;
  const full = await loadResolvedSecrets(ctx);
  if (full.ok) {
    secrets = full.secrets;
  } else if (soft.ok) {
    // Still apply inbound plan with soft config (typing/reply need full creds later).
    secrets = resolvedFromValidated({
      ...soft.config,
      apiKey: soft.config.apiKey ?? "",
      apiSecret: soft.config.apiSecret ?? "",
      webhookSecret,
    });
  }

  if (
    secrets &&
    (input.endpointKey === WEBHOOK_ENDPOINT_KEYS.inbound || !input.endpointKey)
  ) {
    await applyInboundSideEffects(ctx, secrets, handled);
  } else {
    ctx.logger.info("Webhook received", {
      kind: handled.normalized.kind,
      endpointKey: input.endpointKey,
      requestId: input.requestId,
    });
  }
}
