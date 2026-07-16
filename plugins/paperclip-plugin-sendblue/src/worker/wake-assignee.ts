/**
 * Wake the assignee after inbound issue create via the board REST client.
 */

import type { PluginContext } from "@paperclipai/plugin-sdk";
import { wakeAssigneeViaBoardRest } from "./board-rest.js";

export async function wakeAssigneeForInbound(
  ctx: PluginContext,
  input: {
    issueId: string;
    companyId: string;
    assigneeAgentId?: string;
    title: string;
    body: string;
  },
): Promise<void> {
  if (!input.assigneeAgentId) {
    ctx.logger.warn("inbound issue without assignee — not waking agent", {
      issueId: input.issueId,
    });
    return;
  }

  const result = await wakeAssigneeViaBoardRest({
    issueId: input.issueId,
    companyId: input.companyId,
    assigneeAgentId: input.assigneeAgentId,
  });

  if (result.ok) {
    ctx.logger.info("board agent wakeup ok", {
      issueId: input.issueId,
      assigneeAgentId: input.assigneeAgentId,
    });
    return;
  }

  ctx.logger.error("board agent wakeup failed", {
    issueId: input.issueId,
    assigneeAgentId: input.assigneeAgentId,
    error: result.error,
  });
}
