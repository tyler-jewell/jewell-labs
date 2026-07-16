/**
 * Inbound receive + outbound status webhook normalization + optional secret verify.
 * Pure functions for unit tests; worker maps HTTP body onto these.
 *
 * Secret verification matches SendBlue docs + community adapters:
 * - plain header `x-sendblue-secret` / `x-webhook-secret` equals shared secret
 * - optional HMAC `x-sendblue-signature` = hex hmac-sha256(rawBody, secret)
 */

import { createHmac, timingSafeEqual } from "node:crypto";
import { isValidE164, normalizeE164 } from "./e164.js";

export type InboundMessage = {
  kind: "inbound_message";
  from_number?: string;
  to_number?: string;
  content?: string;
  media_url?: string;
  message_handle?: string;
  service?: string;
  is_outbound: false;
  raw: unknown;
};

export type OutboundStatus = {
  kind: "outbound_status";
  message_handle?: string;
  status?: string;
  number?: string;
  error_message?: string;
  is_outbound: true;
  raw: unknown;
};

export type CallLogEvent = {
  kind: "call_log";
  call_id?: string;
  from_number?: string;
  to_number?: string;
  status?: string;
  duration?: number;
  raw: unknown;
};

export type NormalizedWebhook =
  | InboundMessage
  | OutboundStatus
  | CallLogEvent
  | { kind: "unknown"; raw: unknown };

function asStr(v: unknown): string | undefined {
  return typeof v === "string" && v.trim() ? v.trim() : undefined;
}

function asNum(v: unknown): number | undefined {
  return typeof v === "number" && Number.isFinite(v) ? v : undefined;
}

/**
 * Normalize SendBlue webhook JSON (receive callback or status_callback).
 * Heuristic: is_outbound true / status fields without content → status;
 * content or is_outbound false → inbound.
 */
export function normalizeWebhookBody(body: unknown): NormalizedWebhook {
  if (!body || typeof body !== "object") {
    return { kind: "unknown", raw: body };
  }
  const o = body as Record<string, unknown>;
  // Nested data envelope
  const data =
    o.data && typeof o.data === "object"
      ? (o.data as Record<string, unknown>)
      : o;

  if (
    data.event_type === "call_log" ||
    o.event_type === "call_log" ||
    (asStr(data.call_id) && asStr(data.direction))
  ) {
    return {
      kind: "call_log",
      call_id: asStr(data.call_id) ?? asStr(o.call_id),
      from_number: asStr(data.from_number) ?? asStr(o.from_number),
      to_number: asStr(data.to_number) ?? asStr(o.to_number),
      status: asStr(data.status) ?? asStr(o.status),
      duration: asNum(data.duration) ?? asNum(o.duration),
      raw: body,
    };
  }

  const isOutbound =
    data.is_outbound === true ||
    data.is_outbound === "true" ||
    o.is_outbound === true;

  const content = asStr(data.content) ?? asStr(o.content);
  const status = asStr(data.status) ?? asStr(o.status);
  const handle =
    asStr(data.message_handle) ??
    asStr(o.message_handle) ??
    asStr(data.handle);

  // Inbound: from_number is the end-user; number often mirrors from_number.
  // Outbound status: number is the end-user recipient.
  const from =
    asStr(data.from_number) ??
    asStr(o.from_number) ??
    (!isOutbound ? asStr(data.number) ?? asStr(o.number) : undefined);
  const to =
    asStr(data.to_number) ??
    asStr(o.to_number) ??
    asStr(data.sendblue_number) ??
    asStr(o.sendblue_number);

  const STATUS_ONLY = [
    "QUEUED",
    "PENDING",
    "SENT",
    "DELIVERED",
    "ERROR",
    "DECLINED",
    "ACCEPTED",
    "REGISTERED",
  ];

  // Status callbacks often have status + handle and little content
  if (
    isOutbound ||
    (status && handle && !content) ||
    (status &&
      STATUS_ONLY.includes(status.toUpperCase()) &&
      !content &&
      data.is_outbound !== false)
  ) {
    return {
      kind: "outbound_status",
      message_handle: handle,
      status,
      number: asStr(data.number) ?? asStr(o.number) ?? from ?? to,
      error_message: asStr(data.error_message) ?? asStr(o.error_message),
      is_outbound: true,
      raw: body,
    };
  }

  if (
    content ||
    data.is_outbound === false ||
    o.is_outbound === false ||
    status === "RECEIVED"
  ) {
    return {
      kind: "inbound_message",
      from_number: from,
      to_number: to,
      content,
      media_url: asStr(data.media_url) ?? asStr(o.media_url),
      message_handle: handle,
      service: asStr(data.service) ?? asStr(o.service),
      is_outbound: false,
      raw: body,
    };
  }

  return { kind: "unknown", raw: body };
}

/**
 * Verify webhook authenticity when a shared secret is configured.
 * When secret is empty/undefined, verification is skipped (ok=true).
 */
export function verifyWebhookSecret(input: {
  secret?: string | null;
  headers?: Record<string, string | string[] | undefined>;
  rawBody?: string;
}): { ok: true } | { ok: false; reason: string } {
  const secret = input.secret?.trim();
  if (!secret) return { ok: true };

  const headers: Record<string, string> = {};
  for (const [k, v] of Object.entries(input.headers ?? {})) {
    if (v == null) continue;
    const first = Array.isArray(v) ? v[0] : v;
    if (typeof first === "string") headers[k.toLowerCase()] = first;
  }

  const plain =
    headers["x-sendblue-secret"] ??
    headers["x-webhook-secret"] ??
    headers["x-sendblue-token"] ??
    headers["sb-signing-secret"];
  if (plain) {
    try {
      const a = Buffer.from(plain);
      const b = Buffer.from(secret);
      if (a.length === b.length && timingSafeEqual(a, b)) return { ok: true };
    } catch {
      /* fall through */
    }
    return { ok: false, reason: "secret header mismatch" };
  }

  const sig =
    headers["x-sendblue-signature"] ??
    headers["x-hub-signature-256"] ??
    headers["x-signature"];
  if (sig && input.rawBody != null) {
    const hex = createHmac("sha256", secret).update(input.rawBody).digest("hex");
    const provided = sig.replace(/^sha256=/i, "").trim();
    try {
      const a = Buffer.from(hex);
      const b = Buffer.from(provided);
      if (a.length === b.length && timingSafeEqual(a, b)) return { ok: true };
    } catch {
      /* fall through */
    }
    return { ok: false, reason: "hmac signature mismatch" };
  }

  return {
    ok: false,
    reason: "webhook secret configured but no secret/signature header present",
  };
}

/**
 * Shape suitable for HTTP mapping in tests.
 * Host onWebhook is void — worker throws on 401-class failures.
 */
export type WebhookHandlerResult = {
  status: number;
  body: {
    ok: boolean;
    kind?: string;
    from_number?: string;
    content?: string;
    message_handle?: string;
    status?: string;
    error?: string;
  };
  /** Structured parse for worker side-effects (comment / wake). */
  normalized: NormalizedWebhook;
};

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
