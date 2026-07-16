/**
 * Host-facing orchestration without SDK side effects (no runWorker).
 * Safe to import from tests and package index.
 */

import { assertAllowlisted } from "./allowlist.js";
import {
  allowlistPolicyFromConfig,
  credentialsFromConfig,
  validateConfig,
  type InboundMode,
  type SendBluePluginConfig,
} from "./config.js";
import {
  formatAgentError,
  formatApprovalRequested,
  formatIssueCreated,
  formatIssueDone,
} from "./notify-format.js";
import {
  executePrepared,
  parseLinesResponse,
  parseMessagesResponse,
  parseSendMessageResponse,
  prepareSendMessage,
  type Credentials,
} from "./sendblue-api.js";
import { buildToolRequest } from "./tools.js";
import { asString } from "./util.js";
import {
  handleWebhookRequest,
  inboundSenderE164,
  type NormalizedWebhook,
} from "./webhooks.js";

export type ResolvedSecrets = {
  apiKey: string;
  apiSecret: string;
  fromNumber?: string;
  webhookSecret?: string;
  allowlist: string[];
  emptyMeansDeny: boolean;
  notifyOnIssueDone: boolean;
  notifyOnIssueCreated: boolean;
  notifyOnApprovalCreated: boolean;
  notifyOnAgentError: boolean;
  notifyNumber?: string;
  inboundMode: InboundMode;
  defaultCompanyId?: string;
  defaultProjectId?: string;
  defaultAssigneeAgentId?: string;
};

export function resolvedFromValidated(
  config: SendBluePluginConfig,
): ResolvedSecrets {
  return {
    apiKey: config.apiKey ?? "",
    apiSecret: config.apiSecret ?? "",
    fromNumber: config.fromNumber,
    webhookSecret: config.webhookSecret,
    allowlist: config.allowlist,
    emptyMeansDeny: config.emptyMeansDeny,
    notifyOnIssueDone: config.notifyOnIssueDone,
    notifyOnIssueCreated: config.notifyOnIssueCreated,
    notifyOnApprovalCreated: config.notifyOnApprovalCreated,
    notifyOnAgentError: config.notifyOnAgentError,
    notifyNumber: config.notifyNumber,
    inboundMode: config.inboundMode,
    defaultCompanyId: config.defaultCompanyId,
    defaultProjectId: config.defaultProjectId,
    defaultAssigneeAgentId: config.defaultAssigneeAgentId,
  };
}

export function resolveRuntimeConfig(
  raw: Record<string, unknown>,
): ReturnType<typeof validateConfig> {
  return validateConfig(raw, { requireCredentials: true });
}

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
    const deny = denyInboundIfNotAllowlisted(
      handled.normalized,
      {
        numbers: input.secrets.allowlist,
        emptyMeansDeny: input.secrets.emptyMeansDeny,
      },
    );
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
 * Inbound allowlist gate.
 * Missing or non-E.164 from_number is always denied (cannot safely allowlist "unknown").
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

/** Pure decision for what the host should do with an allowed inbound message. */
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

export function planInboundAction(
  normalized: NormalizedWebhook,
  secrets: ResolvedSecrets,
): InboundAction {
  if (normalized.kind !== "inbound_message") {
    return { action: "ignore", reason: `not inbound (${normalized.kind})` };
  }
  if (secrets.inboundMode === "ignore") {
    return { action: "ignore", reason: "inboundMode=ignore" };
  }

  // Defense in depth: never create issues for unallowlisted / unknown senders.
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

  if (!secrets.defaultCompanyId) {
    return {
      action: "log_only",
      summary: `${summary} (no defaultCompanyId — not creating issue)`,
    };
  }

  const title =
    content.length > 72
      ? `iMessage from ${sender}: ${content.slice(0, 69)}…`
      : `iMessage from ${sender}: ${content}`;

  const body = [
    `Inbound SendBlue message`,
    ``,
    content,
    media,
    handle,
    service,
    `from: ${sender}`,
    normalized.to_number ? `to: ${normalized.to_number}` : "",
  ]
    .filter(Boolean)
    .join("\n");

  return {
    action: "create_issue",
    companyId: secrets.defaultCompanyId,
    projectId: secrets.defaultProjectId,
    assigneeAgentId: secrets.defaultAssigneeAgentId,
    title,
    body,
    requestWakeup: Boolean(secrets.defaultAssigneeAgentId),
  };
}

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
