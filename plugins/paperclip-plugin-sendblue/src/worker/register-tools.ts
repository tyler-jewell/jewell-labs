/**
 * Register agent tools + board events:
 * - optional notifyNumber alerts (approvals / errors; issue-done off by default)
 * - conversation loop: typing while agent runs, reply SMS on agent comments
 */

import type { PluginContext, PluginEvent } from "@paperclipai/plugin-sdk";
import {
  commentBodyFromPayload,
  commentIdFromPayload,
  isAgentAuthoredComment,
  issueIdFromEvent,
  sendReplyTo,
  sendTypingTo,
} from "../runtime/conversation.js";
import { TOOL_META } from "../tools/meta.js";
import { TOOL_NAMES, toolParameterSchemas } from "../tools.js";
import { executeToolForHost, maybeNotifyFromEvent } from "../worker-logic.js";
import { fetchFromCtx, loadResolvedSecrets } from "./config-load.js";
import { loadConversation } from "./conversation-io.js";

export function registerToolsAndEvents(ctx: PluginContext): void {
  for (const name of TOOL_NAMES) {
    const schema = toolParameterSchemas[name];
    const meta = TOOL_META[name];
    ctx.tools.register(
      name,
      {
        displayName: meta.displayName,
        description: meta.description,
        parametersSchema: schema,
      },
      async (params: unknown) => {
        const resolved = await loadResolvedSecrets(ctx);
        if (!resolved.ok) {
          return {
            error: `SendBlue not configured: ${resolved.errors.join("; ")}`,
          };
        }
        const out = await executeToolForHost(
          name,
          (params ?? {}) as Record<string, unknown>,
          resolved.secrets,
          fetchFromCtx(ctx),
        );
        if (!out.ok) {
          return { error: out.error ?? `${name} failed` };
        }
        // Compact JSON for agent context windows; full object in data.
        return {
          content: JSON.stringify(out.result ?? { ok: true }, null, 2),
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

  const tryTypingForIssue = async (
    issueId: string,
    source: string,
  ): Promise<void> => {
    const conv = await loadConversation(ctx, issueId);
    if (!conv) return;
    const resolved = await loadResolvedSecrets(ctx);
    if (!resolved.ok) {
      ctx.logger.warn("typing skipped — credentials not ready", {
        issueId,
        source,
      });
      return;
    }
    // Refresh long pulse on each agent-work signal (run start / checkout).
    const r = await sendTypingTo(
      resolved.secrets,
      conv.fromNumber,
      globalThis.fetch.bind(globalThis),
      {
        fromLine: conv.fromLine,
        maxDurationMs: 180_000,
        markReadFirst: true,
        state: "start",
      },
    );
    if (r.ok) {
      ctx.logger.info("SendBlue typing sent", {
        issueId,
        source,
        to: conv.fromNumber,
        fromLine: conv.fromLine,
        status: r.status,
      });
    } else {
      ctx.logger.warn("SendBlue typing failed", {
        issueId,
        source,
        to: conv.fromNumber,
        fromLine: conv.fromLine,
        error: r.error,
      });
    }
  };

  // Typing while agent works on a conversation-linked issue.
  ctx.events.on("agent.run.started", async (event: PluginEvent) => {
    const payload =
      typeof event.payload === "object" && event.payload !== null
        ? (event.payload as Record<string, unknown>)
        : {};
    const issueId = issueIdFromEvent({
      entityId: event.entityId,
      entityType: event.entityType,
      payload,
    });
    if (issueId) {
      await tryTypingForIssue(issueId, "agent.run.started");
      return;
    }
    // Run entity events often omit issueId; primary typing is on inbound create.
  });

  // Checkout is issue-scoped and a reliable "agent is working" signal.
  ctx.events.on("issue.checked_out", async (event: PluginEvent) => {
    const issueId = issueIdFromEvent({
      entityId: event.entityId,
      entityType: event.entityType ?? "issue",
      payload:
        typeof event.payload === "object" && event.payload !== null
          ? (event.payload as Record<string, unknown>)
          : {},
    });
    if (!issueId) return;
    await tryTypingForIssue(issueId, "issue.checked_out");
  });

  // Reply to original SMS sender when the agent posts a comment.
  // Host maps activity `issue.comment_added` → plugin event `issue.comment.created`
  // with payload { commentId, bodySnippet, agentId, … } (often no full body).
  ctx.events.on("issue.comment.created", async (event: PluginEvent) => {
    const payload =
      typeof event.payload === "object" && event.payload !== null
        ? (event.payload as Record<string, unknown>)
        : {};
    if (!isAgentAuthoredComment(payload, event.actorType)) {
      ctx.logger.info("comment event ignored — not agent-authored", {
        actorType: event.actorType,
      });
      return;
    }

    const issueId =
      issueIdFromEvent({
        entityId: event.entityId,
        entityType: event.entityType,
        payload,
      }) ??
      (typeof payload.issueId === "string" ? payload.issueId : undefined);
    if (!issueId) {
      ctx.logger.warn("agent comment reply skipped — no issueId", {
        entityId: event.entityId,
        entityType: event.entityType,
      });
      return;
    }

    const conv = await loadConversation(ctx, issueId);
    if (!conv) {
      ctx.logger.info("comment event ignored — no inbound conversation", {
        issueId,
      });
      return;
    }

    // Prefer full comment body when host only sent bodySnippet.
    let body = commentBodyFromPayload(payload);
    const commentId = commentIdFromPayload(payload);
    if (commentId && event.companyId) {
      try {
        const comments = await ctx.issues.listComments(
          issueId,
          event.companyId,
        );
        const full = comments.find((c) => c.id === commentId);
        const fullBody = full && typeof full.body === "string" ? full.body.trim() : "";
        if (fullBody) body = fullBody;
      } catch (err) {
        ctx.logger.warn("listComments for reply body failed", {
          issueId,
          error: String(err),
        });
      }
    }
    if (!body) {
      ctx.logger.warn("agent comment reply skipped — empty body", { issueId });
      return;
    }

    const resolved = await loadResolvedSecrets(ctx);
    if (!resolved.ok) {
      ctx.logger.warn("agent reply skipped — credentials not ready", {
        issueId,
      });
      return;
    }

    const r = await sendReplyTo(
      resolved.secrets,
      conv.fromNumber,
      body,
      globalThis.fetch.bind(globalThis),
      { fromLine: conv.fromLine },
    );
    if (r.ok) {
      ctx.logger.info("SendBlue agent reply sent", {
        issueId,
        to: conv.fromNumber,
      });
    } else {
      ctx.logger.error("SendBlue agent reply failed", {
        issueId,
        error: r.error,
      });
    }
  });

  // Short failure SMS to the original sender when a run fails.
  ctx.events.on("agent.run.failed", async (event: PluginEvent) => {
    const payload =
      typeof event.payload === "object" && event.payload !== null
        ? (event.payload as Record<string, unknown>)
        : {};
    const issueId = issueIdFromEvent({
      entityId: event.entityId,
      entityType: event.entityType,
      payload,
    });
    if (!issueId) return;
    const conv = await loadConversation(ctx, issueId);
    if (!conv) return;
    const resolved = await loadResolvedSecrets(ctx);
    if (!resolved.ok) return;
    const errMsg =
      (typeof payload.error === "string" && payload.error) ||
      (typeof payload.message === "string" && payload.message) ||
      "agent run failed";
    const r = await sendReplyTo(
      resolved.secrets,
      conv.fromNumber,
      `Agent error: ${errMsg}`.slice(0, 500),
      globalThis.fetch.bind(globalThis),
      { fromLine: conv.fromLine },
    );
    if (r.ok) {
      ctx.logger.info("SendBlue agent-error SMS sent", { issueId });
    }
  });
}
