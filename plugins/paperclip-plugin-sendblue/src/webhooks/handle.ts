/** Compose verify + normalize into an HTTP-shaped result. */

import { isValidE164, normalizeE164 } from "../e164.js";
import { normalizeWebhookBody } from "./normalize.js";
import type { NormalizedWebhook, WebhookHandlerResult } from "./types.js";
import { verifyWebhookSecret } from "./verify.js";

export function handleWebhookRequest(input: {
  secret?: string | null;
  headers?: Record<string, string | string[] | undefined>;
  rawBody?: string;
  parsedBody: unknown;
}): WebhookHandlerResult {
  const v = verifyWebhookSecret({
    secret: input.secret,
    headers: input.headers,
    rawBody: input.rawBody,
  });
  if (!v.ok) {
    return {
      status: 401,
      body: { ok: false, error: v.reason },
      normalized: { kind: "unknown", raw: input.parsedBody },
    };
  }
  const normalized = normalizeWebhookBody(input.parsedBody);
  if (normalized.kind === "inbound_message") {
    return {
      status: 200,
      body: {
        ok: true,
        kind: normalized.kind,
        from_number: normalized.from_number,
        content: normalized.content,
        message_handle: normalized.message_handle,
      },
      normalized,
    };
  }
  if (normalized.kind === "outbound_status") {
    return {
      status: 200,
      body: {
        ok: true,
        kind: normalized.kind,
        message_handle: normalized.message_handle,
        status: normalized.status,
      },
      normalized,
    };
  }
  if (normalized.kind === "call_log") {
    return {
      status: 200,
      body: {
        ok: true,
        kind: normalized.kind,
        status: normalized.status,
      },
      normalized,
    };
  }
  return {
    status: 200,
    body: { ok: true, kind: "unknown" },
    normalized,
  };
}

/** Extract sender E.164 from inbound for allowlist checks. */
export function inboundSenderE164(n: NormalizedWebhook): string | undefined {
  if (n.kind !== "inbound_message" || !n.from_number) return undefined;
  const s = normalizeE164(n.from_number);
  return isValidE164(s) ? s : undefined;
}
