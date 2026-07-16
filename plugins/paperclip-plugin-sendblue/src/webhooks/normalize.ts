/** Heuristic normalize of SendBlue webhook JSON. */

import type { NormalizedWebhook } from "./types.js";

function asStr(v: unknown): string | undefined {
  return typeof v === "string" && v.trim() ? v.trim() : undefined;
}

function asNum(v: unknown): number | undefined {
  return typeof v === "number" && Number.isFinite(v) ? v : undefined;
}

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

/**
 * Normalize SendBlue webhook JSON (receive callback or status_callback).
 */
export function normalizeWebhookBody(body: unknown): NormalizedWebhook {
  if (!body || typeof body !== "object") {
    return { kind: "unknown", raw: body };
  }
  const o = body as Record<string, unknown>;
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

  const from =
    asStr(data.from_number) ??
    asStr(o.from_number) ??
    (!isOutbound ? asStr(data.number) ?? asStr(o.number) : undefined);
  const to =
    asStr(data.to_number) ??
    asStr(o.to_number) ??
    asStr(data.sendblue_number) ??
    asStr(o.sendblue_number);

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
