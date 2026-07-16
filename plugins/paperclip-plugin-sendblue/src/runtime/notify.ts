/** Board event → optional SMS notify. */

import { assertAllowlisted } from "../allowlist.js";
import {
  allowlistPolicyFromConfig,
  credentialsFromConfig,
  validateConfig,
} from "../config.js";
import {
  formatAgentError,
  formatApprovalRequested,
  formatIssueCreated,
  formatIssueDone,
} from "../notify-format.js";
import {
  executePrepared,
  prepareSendMessage,
  type Credentials,
} from "../sendblue-api.js";
import { asString } from "../util.js";
import type { ResolvedSecrets } from "./resolved.js";

export async function maybeNotifyFromEvent(
  event: { type: string; payload?: Record<string, unknown> },
  secrets: ResolvedSecrets,
  fetchImpl: typeof fetch = fetch,
): Promise<{ sent: boolean; reason?: string; requestUrl?: string }> {
  const v = validateConfig(
    {
      apiKey: secrets.apiKey,
      apiSecret: secrets.apiSecret,
      fromNumber: secrets.fromNumber,
      allowlist: secrets.allowlist,
      emptyMeansDeny: secrets.emptyMeansDeny,
      notifyNumber: secrets.notifyNumber,
      notifyOnIssueDone: secrets.notifyOnIssueDone,
      notifyOnIssueCreated: secrets.notifyOnIssueCreated,
      notifyOnApprovalCreated: secrets.notifyOnApprovalCreated,
      notifyOnAgentError: secrets.notifyOnAgentError,
    },
    { requireCredentials: true },
  );
  if (!v.ok) return { sent: false, reason: v.errors.join("; ") };
  if (!v.config.notifyNumber || !v.config.fromNumber) {
    return { sent: false, reason: "notifyNumber/fromNumber not configured" };
  }

  let content: string | null = null;
  const p = event.payload ?? {};
  if (event.type === "issue.updated" && secrets.notifyOnIssueDone) {
    const status = asString(p.status ?? p.newStatus);
    if (status === "done" || status === "completed") {
      content = formatIssueDone({
        identifier: asString(p.identifier ?? p.issueIdentifier),
        title: asString(p.title),
      });
    }
  } else if (event.type === "issue.created" && secrets.notifyOnIssueCreated) {
    content = formatIssueCreated({
      identifier: asString(p.identifier),
      title: asString(p.title),
    });
  } else if (
    (event.type === "approval.created" ||
      event.type === "approval.requested") &&
    secrets.notifyOnApprovalCreated
  ) {
    content = formatApprovalRequested({
      summary: asString(p.summary ?? p.title),
      id: asString(p.id),
    });
  } else if (
    (event.type === "agent.run.failed" || event.type === "agent.error") &&
    secrets.notifyOnAgentError
  ) {
    content = formatAgentError({
      agentName: asString(p.agentName ?? p.agentId, "agent"),
      message: asString(p.error ?? p.message, "error"),
    });
  }

  if (!content) return { sent: false, reason: "event not configured for notify" };

  const creds: Credentials = credentialsFromConfig(v.config);
  const policy = allowlistPolicyFromConfig(v.config);
  try {
    const to = assertAllowlisted(v.config.notifyNumber, policy);
    const prep = prepareSendMessage(creds, {
      number: to,
      from_number: v.config.fromNumber,
      content,
    });
    await executePrepared(prep, fetchImpl);
    return { sent: true, requestUrl: prep.url };
  } catch (e) {
    return {
      sent: false,
      reason: e instanceof Error ? e.message : String(e),
    };
  }
}
