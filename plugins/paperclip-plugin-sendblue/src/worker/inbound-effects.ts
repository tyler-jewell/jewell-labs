/** Apply planned inbound actions (log / create issue / wakeup / conversation). */

import type { PluginContext } from "@paperclipai/plugin-sdk";
import { PLUGIN_ID } from "../constants.js";
import {
  conversationFromInbound,
  sendTypingTo,
} from "../runtime/conversation.js";
import {
  planInboundAction,
  type ResolvedSecrets,
} from "../worker-logic.js";
import type { WebhookHandlerResult } from "../webhooks.js";
import { inboundSenderE164 } from "../webhooks.js";
import { createIssueViaBoardRest } from "./board-rest.js";
import {
  resolveCompanyAndAgent,
  saveConversation,
} from "./conversation-io.js";
import { wakeAssigneeForInbound } from "./wake-assignee.js";

/** External SendBlue calls — use global fetch (stable under webhook workers). */
const sbFetch = globalThis.fetch.bind(globalThis);

function isCompanyUuid(id: string | undefined): id is string {
  return (
    typeof id === "string" &&
    /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(
      id,
    )
  );
}

async function pulseTyping(
  ctx: PluginContext,
  secrets: ResolvedSecrets,
  sender: string,
  fromLine: string,
  phase: string,
): Promise<void> {
  const typing = await sendTypingTo(secrets, sender, sbFetch, {
    fromLine,
    maxDurationMs: 180_000,
    markReadFirst: true,
    state: "start",
  });
  if (!typing.ok) {
    ctx.logger.warn("inbound typing indicator failed", {
      phase,
      error: typing.error,
      to: sender,
      fromLine,
    });
  } else {
    ctx.logger.info("SendBlue typing sent (inbound→agent)", {
      phase,
      to: sender,
      fromLine,
      status: typing.status,
      markRead: typing.markRead,
    });
  }
}

export async function applyInboundSideEffects(
  ctx: PluginContext,
  secrets: ResolvedSecrets,
  handled: WebhookHandlerResult,
): Promise<void> {
  if (!handled.body.ok) return;

  const routing = await resolveCompanyAndAgent(ctx);
  if (!routing.companyId) {
    ctx.logger.warn("board routing found no company");
  }
  const plan = planInboundAction(handled.normalized, secrets, routing);

  if (plan.action === "ignore") {
    ctx.logger.info("Inbound webhook ignored", { reason: plan.reason });
    return;
  }
  if (plan.action === "log_only") {
    ctx.logger.info(plan.summary);
    if (isCompanyUuid(routing.companyId)) {
      try {
        await ctx.activity.log({
          companyId: routing.companyId,
          message: plan.summary,
          entityType: "plugin",
        });
      } catch {
        /* optional */
      }
    }
    return;
  }

  const sender =
    handled.normalized.kind === "inbound_message"
      ? inboundSenderE164(handled.normalized)
      : undefined;
  const fromLine =
    secrets.fromNumber ??
    (handled.normalized.kind === "inbound_message"
      ? handled.normalized.to_number
      : undefined);

  // Typing ASAP — do not wait for issue create.
  if (sender && fromLine) {
    await pulseTyping(ctx, secrets, sender, fromLine, "pre-create");
  }

  const rest = await createIssueViaBoardRest({
    companyId: plan.companyId,
    title: plan.title,
    description: plan.body,
    projectId: plan.projectId,
    assigneeAgentId: plan.assigneeAgentId,
    status: "todo",
  });
  if (!rest.ok) {
    ctx.logger.error("Failed to create issue from inbound message", {
      error: rest.error,
      companyId: plan.companyId,
    });
    return;
  }

  const issueId = rest.issue.id;
  const issueIdentifier = rest.issue.identifier;
  ctx.logger.info("Created issue from inbound SendBlue message", {
    issueId,
    companyId: plan.companyId,
    assigneeAgentId: plan.assigneeAgentId,
    via: "board-rest",
    originKind: `plugin:${PLUGIN_ID}`,
  });

  if (sender && fromLine) {
    try {
      await saveConversation(
        ctx,
        issueId,
        conversationFromInbound({
          fromNumber: sender,
          companyId: plan.companyId,
          fromLine,
          messageHandle:
            handled.normalized.kind === "inbound_message"
              ? handled.normalized.message_handle
              : undefined,
        }),
      );
    } catch (err) {
      ctx.logger.warn("Failed to store inbound conversation mapping", {
        error: String(err),
      });
    }
    await pulseTyping(ctx, secrets, sender, fromLine, "post-create");
  }

  if (plan.requestWakeup || plan.assigneeAgentId) {
    await wakeAssigneeForInbound(ctx, {
      issueId,
      companyId: plan.companyId,
      assigneeAgentId: plan.assigneeAgentId,
      title: plan.title,
      body: plan.body,
    });
  }

  try {
    await ctx.activity.log({
      companyId: plan.companyId,
      message: `SendBlue inbound → issue ${issueIdentifier ?? issueId}`,
      entityType: "issue",
      entityId: issueId,
    });
  } catch {
    /* optional */
  }
}
