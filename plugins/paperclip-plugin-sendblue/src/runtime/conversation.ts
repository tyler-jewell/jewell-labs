/**
 * Pure helpers for inbound SMS ↔ issue conversation mapping + outbound replies.
 */

import { assertAllowlisted } from "../allowlist.js";
import {
  allowlistPolicyFromConfig,
  credentialsFromConfig,
  type SendBluePluginConfig,
  validateConfig,
} from "../config.js";
import {
  executePrepared,
  prepareSendMessage,
  type Credentials,
} from "../sendblue-api.js";
import { asString } from "../util.js";
import type { ResolvedSecrets } from "./resolved.js";
import {
  assertSendBlueOutboundOk,
  sendTypingPulse,
  type TypingPulseOptions,
} from "./typing.js";

export const CONVERSATION_STATE_KEY = "inbound-conversation";

/** Stored on plugin state scope issue:{issueId}. */
export type ConversationRecord = {
  fromNumber: string;
  companyId: string;
  /** SendBlue line used for replies (E.164). */
  fromLine: string;
  messageHandle?: string;
  createdAt: string;
};

export function conversationFromInbound(input: {
  fromNumber: string;
  companyId: string;
  fromLine: string;
  messageHandle?: string;
}): ConversationRecord {
  return {
    fromNumber: input.fromNumber,
    companyId: input.companyId,
    fromLine: input.fromLine,
    messageHandle: input.messageHandle,
    createdAt: new Date().toISOString(),
  };
}

export function parseConversation(raw: unknown): ConversationRecord | null {
  if (!raw || typeof raw !== "object") return null;
  const o = raw as Record<string, unknown>;
  const fromNumber = asString(o.fromNumber);
  const companyId = asString(o.companyId);
  const fromLine = asString(o.fromLine);
  if (!fromNumber || !companyId || !fromLine) return null;
  const createdAt = asString(o.createdAt);
  return {
    fromNumber,
    companyId,
    fromLine,
    messageHandle: asString(o.messageHandle) || undefined,
    createdAt: createdAt || new Date().toISOString(),
  };
}

function firstNonEmpty(...vals: string[]): string | undefined {
  for (const v of vals) {
    if (v.trim()) return v.trim();
  }
  return undefined;
}

function nestedString(
  obj: Record<string, unknown>,
  path: string[],
): string | undefined {
  let cur: unknown = obj;
  for (const key of path) {
    if (!cur || typeof cur !== "object") return undefined;
    cur = (cur as Record<string, unknown>)[key];
  }
  return asString(cur);
}

/** Extract issue id from agent-run / comment event envelopes. */
export function issueIdFromEvent(event: {
  entityId?: string;
  entityType?: string;
  payload?: Record<string, unknown>;
}): string | undefined {
  const p = event.payload ?? {};
  const fromPayload = firstNonEmpty(
    asString(p.issueId),
    asString(p.issue_id),
    asString(p.taskId),
    asString(p.task_id),
    nestedString(p, ["issue", "id"]) ?? "",
    nestedString(p, ["issue", "issueId"]) ?? "",
    nestedString(p, ["context", "issueId"]) ?? "",
    nestedString(p, ["contextSnapshot", "issueId"]) ?? "",
    nestedString(p, ["wakeup", "issueId"]) ?? "",
  );
  if (fromPayload) return fromPayload;
  // Comments: entityId is often the comment UUID — only trust payload.
  if (event.entityType === "comment") {
    return undefined;
  }
  // issue.* events (and bare entityId) use entityId = issue UUID.
  if (
    event.entityId &&
    (event.entityType === "issue" || !event.entityType)
  ) {
    return event.entityId;
  }
  return undefined;
}

/** Prefer agent-authored comments only; skip system notices / human chatter. */
export function isAgentAuthoredComment(
  payload: Record<string, unknown>,
  actorType?: string,
): boolean {
  const presentation =
    payload.presentation && typeof payload.presentation === "object"
      ? (payload.presentation as Record<string, unknown>)
      : undefined;
  const presentationKind = asString(presentation?.kind);
  if (presentationKind === "system_notice") return false;

  const authorType = firstNonEmpty(
    asString(payload.authorType),
    asString(payload.author_type),
    asString(payload.actorType),
    asString(payload.actor_type),
    actorType ?? "",
  );
  if (authorType === "agent") return true;
  if (authorType === "user" || authorType === "human" || authorType === "system") {
    return false;
  }
  if (firstNonEmpty(asString(payload.agentId), asString(payload.agent_id))) {
    return true;
  }
  return false;
}

/**
 * Host activity→plugin events often only include `bodySnippet` (truncated),
 * not full `body`. Prefer full fields; fall back to snippet.
 */
export function commentBodyFromPayload(
  payload: Record<string, unknown>,
): string | undefined {
  return firstNonEmpty(
    asString(payload.body),
    asString(payload.content),
    asString(payload.text),
    asString(payload.message),
    asString(payload.bodySnippet),
    asString(payload.body_snippet),
  );
}

export function commentIdFromPayload(
  payload: Record<string, unknown>,
): string | undefined {
  return firstNonEmpty(
    asString(payload.commentId),
    asString(payload.comment_id),
    nestedString(payload, ["comment", "id"]) ?? "",
  );
}

/**
 * Prepare agent comment text for SMS/iMessage: strip common markdown so
 * agent↔human messaging stays readable on lock screens and group chats.
 */
export function formatReplySms(body: string, max = 1500): string {
  let t = body.trim();
  // Fenced code → plain
  t = t.replace(/```[\w]*\n?([\s\S]*?)```/g, "$1");
  // Headings / bullets / emphasis
  t = t.replace(/^#{1,6}\s+/gm, "");
  t = t.replace(/^\s*[-*+]\s+/gm, "• ");
  t = t.replace(/\*\*([^*]+)\*\*/g, "$1");
  t = t.replace(/__([^_]+)__/g, "$1");
  t = t.replace(/`([^`]+)`/g, "$1");
  t = t.replace(/\[([^\]]+)\]\(([^)]+)\)/g, "$1 ($2)");
  t = t.replace(/\n{3,}/g, "\n\n").trim();
  if (t.length <= max) return t;
  return `${t.slice(0, max - 1)}…`;
}

type CredsOk = {
  ok: true;
  creds: Credentials;
  config: SendBluePluginConfig;
};

type CredsFail = { ok: false; error: string };

function credentialsReady(secrets: ResolvedSecrets): CredsOk | CredsFail {
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
  if (!v.ok) return { ok: false, error: v.errors.join("; ") };
  try {
    return {
      ok: true,
      creds: credentialsFromConfig(v.config),
      config: v.config,
    };
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) };
  }
}

/** @see sendTypingPulse — agent-facing start pulse (long duration). */
export async function sendTypingTo(
  secrets: ResolvedSecrets,
  toNumber: string,
  fetchImpl: typeof fetch,
  options?: TypingPulseOptions,
): Promise<{
  ok: boolean;
  error?: string;
  status?: string;
  markRead?: boolean;
}> {
  return sendTypingPulse(secrets, toNumber, fetchImpl, {
    markReadFirst: true,
    maxDurationMs: 120_000,
    state: "start",
    ...options,
  });
}

export async function stopTypingTo(
  secrets: ResolvedSecrets,
  toNumber: string,
  fetchImpl: typeof fetch,
  options?: Pick<TypingPulseOptions, "fromLine">,
): Promise<{ ok: boolean; error?: string }> {
  return sendTypingPulse(secrets, toNumber, fetchImpl, {
    ...options,
    state: "stop",
    markReadFirst: false,
  });
}

export async function sendReplyTo(
  secrets: ResolvedSecrets,
  toNumber: string,
  content: string,
  fetchImpl: typeof fetch,
  options?: { fromLine?: string },
): Promise<{ ok: boolean; error?: string }> {
  const prep = credentialsReady(secrets);
  if (!prep.ok) return prep;
  const text = formatReplySms(content);
  if (!text) return { ok: false, error: "empty reply body" };
  try {
    const policy = allowlistPolicyFromConfig(prep.config);
    const number = assertAllowlisted(toNumber, policy);
    const fromRaw = options?.fromLine?.trim() ?? prep.config.fromNumber ?? "";
    const from_number = fromRaw.trim();
    if (!from_number) return { ok: false, error: "fromNumber not configured" };
    // Clear typing bubble before the outbound message lands.
    await stopTypingTo(secrets, number, fetchImpl, { fromLine: from_number });
    const raw = await executePrepared(
      prepareSendMessage(prep.creds, {
        number,
        from_number,
        content: text,
      }),
      fetchImpl,
    );
    assertSendBlueOutboundOk(raw, "send-message");
    return { ok: true };
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) };
  }
}
