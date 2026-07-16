/**
 * Paperclip worker entry — definePlugin + runWorker.
 * Pure logic lives in worker-logic.ts so tests never start a worker process.
 */

import {
  definePlugin,
  runWorker,
  type PluginContext,
  type PluginEvent,
  type PluginWebhookInput,
} from "@paperclipai/plugin-sdk";
import { validateConfig } from "./config.js";
import { PLUGIN_ID, PLUGIN_VERSION, WEBHOOK_ENDPOINT_KEYS } from "./constants.js";
import { TOOL_NAMES, toolParameterSchemas } from "./tools.js";
import {
  executeToolForHost,
  maybeNotifyFromEvent,
  planInboundAction,
  processInboundWebhook,
  resolvedFromValidated,
  type ResolvedSecrets,
} from "./worker-logic.js";

/** Set in setup; onWebhook runs outside setup and needs host services. */
let runtimeCtx: PluginContext | null = null;

async function readRawConfig(ctx: PluginContext): Promise<Record<string, unknown>> {
  try {
    return await ctx.config.get();
  } catch {
    return {};
  }
}

async function resolveSecret(
  ctx: PluginContext,
  ref: string | undefined,
  plain: string | undefined,
): Promise<string | undefined> {
  if (plain?.trim()) return plain.trim();
  if (!ref?.trim()) return undefined;
  try {
    return await ctx.secrets.resolve(ref.trim());
  } catch (err) {
    ctx.logger.warn("Failed to resolve secret ref", {
      ref,
      error: String(err),
    });
    return undefined;
  }
}

/**
 * Merge vault secrets into validated config for runtime use.
 */
async function loadResolvedSecrets(
  ctx: PluginContext,
): Promise<
  | { ok: true; secrets: ResolvedSecrets; warnings: string[] }
  | { ok: false; errors: string[]; warnings: string[] }
> {
  const raw = await readRawConfig(ctx);
  const soft = validateConfig(raw, { requireCredentials: false });
  if (!soft.ok) {
    return { ok: false, errors: soft.errors, warnings: soft.warnings };
  }

  const apiKey = await resolveSecret(
    ctx,
    soft.config.apiKeyRef,
    soft.config.apiKey,
  );
  const apiSecret = await resolveSecret(
    ctx,
    soft.config.apiSecretRef,
    soft.config.apiSecret,
  );
  const webhookSecret = await resolveSecret(
    ctx,
    soft.config.webhookSecretRef,
    soft.config.webhookSecret,
  );

  const merged: Record<string, unknown> = {
    ...raw,
    apiKey,
    apiSecret,
    webhookSecret,
    fromNumber: soft.config.fromNumber,
    allowlist: soft.config.allowlist,
    emptyMeansDeny: soft.config.emptyMeansDeny,
    notifyNumber: soft.config.notifyNumber,
    notifyOnIssueDone: soft.config.notifyOnIssueDone,
    notifyOnIssueCreated: soft.config.notifyOnIssueCreated,
    notifyOnApprovalCreated: soft.config.notifyOnApprovalCreated,
    notifyOnAgentError: soft.config.notifyOnAgentError,
    inboundMode: soft.config.inboundMode,
    defaultCompanyId: soft.config.defaultCompanyId,
    defaultProjectId: soft.config.defaultProjectId,
    defaultAssigneeAgentId: soft.config.defaultAssigneeAgentId,
  };

  const hard = validateConfig(merged, { requireCredentials: true });
  if (!hard.ok) {
    return { ok: false, errors: hard.errors, warnings: hard.warnings };
  }
  return {
    ok: true,
    secrets: resolvedFromValidated(hard.config),
    warnings: hard.warnings,
  };
}

function fetchFromCtx(ctx: PluginContext): typeof fetch {
  // Prefer host http client (SSRF policy, metrics).
  return ((input: string | URL, init?: RequestInit) =>
    ctx.http.fetch(String(input), init)) as typeof fetch;
}

async function applyInboundSideEffects(
  ctx: PluginContext,
  secrets: ResolvedSecrets,
  handled: ReturnType<typeof processInboundWebhook>,
): Promise<void> {
  if (!handled.body.ok) return;
  const plan = planInboundAction(handled.normalized, secrets);
  if (plan.action === "ignore") {
    ctx.logger.info("Inbound webhook ignored", { reason: plan.reason });
    return;
  }
  if (plan.action === "log_only") {
    ctx.logger.info(plan.summary);
    try {
      await ctx.activity.log({
        companyId: secrets.defaultCompanyId ?? "instance",
        message: plan.summary,
        entityType: "plugin",
      });
    } catch {
      /* activity optional */
    }
    return;
  }

  // create_issue — SDK: create({ companyId, title, ... })
  try {
    const issue = await ctx.issues.create({
      companyId: plan.companyId,
      title: plan.title,
      description: plan.body,
      projectId: plan.projectId,
      assigneeAgentId: plan.assigneeAgentId,
      originKind: `plugin:${PLUGIN_ID}`,
      originId: PLUGIN_ID,
    });
    ctx.logger.info("Created issue from inbound SendBlue message", {
      issueId: issue.id,
      companyId: plan.companyId,
    });
    if (plan.requestWakeup) {
      try {
        await ctx.issues.requestWakeup(issue.id, plan.companyId, {
          reason: "sendblue inbound message",
          contextSource: PLUGIN_ID,
        });
      } catch (err) {
        ctx.logger.warn("requestWakeup failed", { error: String(err) });
      }
    }
    try {
      await ctx.activity.log({
        companyId: plan.companyId,
        message: `SendBlue inbound → issue ${issue.identifier ?? issue.id}`,
        entityType: "issue",
        entityId: issue.id,
      });
    } catch {
      /* optional */
    }
  } catch (err) {
    ctx.logger.error("Failed to create issue from inbound message", {
      error: String(err),
    });
  }
}

const plugin = definePlugin({
  async setup(ctx: PluginContext) {
    runtimeCtx = ctx;
    ctx.logger.info(`${PLUGIN_ID} v${PLUGIN_VERSION} setup`);

    for (const name of TOOL_NAMES) {
      const schema = toolParameterSchemas[name];
      ctx.tools.register(
        name,
        {
          displayName: name
            .split("_")
            .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
            .join(" "),
          description: `SendBlue ${name.replace(/_/g, " ")}`,
          parametersSchema: schema,
        },
        async (params: unknown) => {
          const resolved = await loadResolvedSecrets(ctx);
          if (!resolved.ok) {
            return { error: `config error: ${resolved.errors.join("; ")}` };
          }
          const out = await executeToolForHost(
            name,
            (params ?? {}) as Record<string, unknown>,
            resolved.secrets,
            fetchFromCtx(ctx),
          );
          if (!out.ok) {
            return { error: out.error ?? "tool failed" };
          }
          return {
            content: JSON.stringify(out.result ?? {}, null, 2),
            data: out.result,
          };
        },
      );
    }

    const onNotify = async (event: PluginEvent) => {
      const resolved = await loadResolvedSecrets(ctx);
      if (!resolved.ok) return;
      const payload =
        typeof event.payload === "object" && event.payload !== null
          ? (event.payload as Record<string, unknown>)
          : {};
      const eventType = event.eventType;
      try {
        const r = await maybeNotifyFromEvent(
          { type: eventType, payload },
          resolved.secrets,
          fetchFromCtx(ctx),
        );
        if (r.sent) {
          ctx.logger.info("SendBlue notify sent", { eventType });
        }
      } catch (err) {
        ctx.logger.error("notify failed", {
          eventType,
          error: String(err),
        });
      }
    };
    ctx.events.on("issue.created", onNotify);
    ctx.events.on("issue.updated", onNotify);
    ctx.events.on("approval.created", onNotify);
    ctx.events.on("agent.run.failed", onNotify);

    ctx.logger.info("SendBlue plugin tools + event handlers registered");
  },

  async onHealth() {
    return {
      status: "ok" as const,
      message: `${PLUGIN_ID}@${PLUGIN_VERSION}`,
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
    const ctx = runtimeCtx;
    if (!ctx) {
      throw new Error("SendBlue plugin not ready (setup incomplete)");
    }

    // Soft-load: webhook secret may be the only required field for verify.
    const raw = await readRawConfig(ctx);
    const soft = validateConfig(raw, { requireCredentials: false });
    const webhookSecret = soft.ok
      ? await resolveSecret(
          ctx,
          soft.config.webhookSecretRef,
          soft.config.webhookSecret,
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
      // Fail delivery so operators notice misconfigured secrets.
      throw new Error(handled.body.error ?? "webhook auth failed");
    }
    if (handled.status === 403) {
      // Ack but do not process unallowlisted senders (no retries storm).
      ctx.logger.warn("Inbound sender not on allowlist", {
        error: handled.body.error,
        endpointKey: input.endpointKey,
      });
      return;
    }

    // Prefer full resolved secrets for side effects when credentials exist.
    let secrets: ResolvedSecrets | null = null;
    const full = await loadResolvedSecrets(ctx);
    if (full.ok) {
      secrets = full.secrets;
    } else if (soft.ok) {
      secrets = resolvedFromValidated({
        ...soft.config,
        apiKey: soft.config.apiKey ?? "",
        apiSecret: soft.config.apiSecret ?? "",
        webhookSecret,
      });
    }

    if (
      secrets &&
      (input.endpointKey === WEBHOOK_ENDPOINT_KEYS.inbound ||
        !input.endpointKey)
    ) {
      await applyInboundSideEffects(ctx, secrets, handled);
    } else {
      ctx.logger.info("Webhook received", {
        kind: handled.normalized.kind,
        endpointKey: input.endpointKey,
        requestId: input.requestId,
      });
    }
  },
});

function tryParse(raw: string | undefined): unknown {
  if (!raw) return undefined;
  try {
    return JSON.parse(raw);
  } catch {
    return raw;
  }
}

export default plugin;
runWorker(plugin, import.meta.url);
