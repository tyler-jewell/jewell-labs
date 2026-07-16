/** Inbound webhook allowlist + board action planning. */

import { assertAllowlisted } from "../allowlist.js";
import {
  handleWebhookRequest,
  inboundSenderE164,
  type NormalizedWebhook,
} from "../webhooks.js";
import type { BoardRouting, ResolvedSecrets } from "./resolved.js";

export function processInboundWebhook(input: {
  secrets: Pick<
    ResolvedSecrets,
    "webhookSecret" | "allowlist" | "emptyMeansDeny"
  >;
  headers?: Record<string, string | string[] | undefined>;
  rawBody?: string;
  parsedBody: unknown;
}) {
  const handled = handleWebhookRequest({
    secret: input.secrets.webhookSecret,
    headers: input.headers,
    rawBody: input.rawBody,
    parsedBody: input.parsedBody,
  });
  if (!handled.body.ok) return handled;

  if (handled.normalized.kind === "inbound_message") {
    const deny = denyInboundIfNotAllowlisted(handled.normalized, {
      numbers: input.secrets.allowlist,
      emptyMeansDeny: input.secrets.emptyMeansDeny,
    });
    if (deny) {
      return {
        status: 403,
        body: { ok: false, error: deny },
        normalized: handled.normalized,
      };
    }
  }
  return handled;
}

/**
 * Missing or non-E.164 from_number is always denied.
 * Valid senders must pass assertAllowlisted (emptyMeansDeny applies).
 */
export function denyInboundIfNotAllowlisted(
  normalized: NormalizedWebhook,
  policy: { numbers: string[]; emptyMeansDeny?: boolean },
): string | null {
  if (normalized.kind !== "inbound_message") return null;
  const sender = inboundSenderE164(normalized);
  if (!sender) {
    const raw = normalized.from_number ?? "(missing)";
    return `SendBlue allowlist denied: inbound from_number missing or not E.164: ${raw}`;
  }
  try {
    assertAllowlisted(sender, policy);
    return null;
  } catch (e) {
    return e instanceof Error ? e.message : String(e);
  }
}

export type InboundAction =
  | { action: "ignore"; reason: string }
  | { action: "log_only"; summary: string }
  | {
      action: "create_issue";
      companyId: string;
      projectId?: string;
      assigneeAgentId?: string;
      title: string;
      body: string;
      requestWakeup: boolean;
    };

/**
 * Plan inbound board action.
 * `routing` comes from live board resolve (companies/agents) — never plugin config.
 */
export function planInboundAction(
  normalized: NormalizedWebhook,
  secrets: ResolvedSecrets,
  routing: BoardRouting = {},
): InboundAction {
  if (normalized.kind !== "inbound_message") {
    return { action: "ignore", reason: `not inbound (${normalized.kind})` };
  }
  if (secrets.inboundMode === "ignore") {
    return { action: "ignore", reason: "inboundMode=ignore" };
  }

  const deny = denyInboundIfNotAllowlisted(normalized, {
    numbers: secrets.allowlist,
    emptyMeansDeny: secrets.emptyMeansDeny,
  });
  if (deny) {
    return { action: "ignore", reason: deny };
  }

  const sender = inboundSenderE164(normalized);
  if (!sender) {
    return {
      action: "ignore",
      reason:
        "SendBlue allowlist denied: inbound from_number missing or not E.164",
    };
  }
  const content = (normalized.content ?? "").trim() || "(no text)";
  const media = normalized.media_url ? `\nmedia: ${normalized.media_url}` : "";
  const handle = normalized.message_handle
    ? `\nhandle: ${normalized.message_handle}`
    : "";
  const service = normalized.service ? `\nservice: ${normalized.service}` : "";
  const summary = `SMS/iMessage from ${sender}: ${content.slice(0, 80)}`;

  if (secrets.inboundMode === "log_only") {
    return { action: "log_only", summary };
  }

  if (!routing.companyId) {
    return {
      action: "log_only",
      summary: `${summary} (board routing has no company — not creating issue)`,
    };
  }

  const title =
    content.length > 72
      ? `iMessage from ${sender}: ${content.slice(0, 69)}…`
      : `iMessage from ${sender}: ${content}`;

  const body = [
    `Inbound SendBlue / iMessage`,
    ``,
    content,
    media,
    handle,
    service,
    `from: ${sender}`,
    normalized.to_number ? `to: ${normalized.to_number}` : "",
    ``,
    `---`,
    `Reply instructions: work this request, then post a final issue comment`,
    `with a concise answer for the original sender (plain text, under ~500 chars).`,
    `That comment is SMS/iMessaged back automatically. Avoid markdown tables.`,
  ]
    .filter(Boolean)
    .join("\n");

  return {
    action: "create_issue",
    companyId: routing.companyId,
    projectId: routing.projectId,
    assigneeAgentId: routing.assigneeAgentId,
    title,
    body,
    requestWakeup: Boolean(routing.assigneeAgentId),
  };
}
