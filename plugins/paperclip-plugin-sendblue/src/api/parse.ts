/** Response parsers for list/send endpoints. */

import {
  type LinesResult,
  type MessageRow,
  type SendMessageResult,
  SendBlueClientError,
} from "./types.js";

export function parseLinesResponse(json: unknown): LinesResult {
  if (!json || typeof json !== "object") {
    throw new SendBlueClientError("lines response not an object", "json");
  }
  const o = json as Record<string, unknown>;
  let raw: unknown = o.numbers ?? o.data ?? o.lines;
  if (Array.isArray(json)) raw = json;
  if (!Array.isArray(raw)) {
    throw new SendBlueClientError(
      "lines response missing numbers array",
      "json",
    );
  }
  const numbers: string[] = [];
  for (const item of raw) {
    if (typeof item === "string") numbers.push(item);
    else if (item && typeof item === "object") {
      const n =
        (item as Record<string, unknown>).number ??
        (item as Record<string, unknown>).phone_number ??
        (item as Record<string, unknown>).sendblue_number;
      if (typeof n === "string") numbers.push(n);
    }
  }
  return { numbers };
}

export function parseSendMessageResponse(json: unknown): SendMessageResult {
  if (!json || typeof json !== "object") {
    return { raw: json };
  }
  const o = json as Record<string, unknown>;
  return {
    status: typeof o.status === "string" ? o.status : undefined,
    message_handle:
      typeof o.message_handle === "string"
        ? o.message_handle
        : typeof o.handle === "string"
          ? o.handle
          : undefined,
    raw: json,
  };
}

export function parseMessagesResponse(json: unknown): {
  status?: string;
  data: MessageRow[];
} {
  if (!json || typeof json !== "object") {
    throw new SendBlueClientError("messages response not an object", "json");
  }
  const o = json as Record<string, unknown>;
  const arr = Array.isArray(o.data) ? o.data : Array.isArray(json) ? json : [];
  const data: MessageRow[] = arr.map((item) => {
    if (!item || typeof item !== "object") return {};
    const m = item as Record<string, unknown>;
    return {
      content: typeof m.content === "string" ? m.content : undefined,
      is_outbound:
        typeof m.is_outbound === "boolean" ? m.is_outbound : undefined,
      status: typeof m.status === "string" ? m.status : undefined,
      message_handle:
        typeof m.message_handle === "string" ? m.message_handle : undefined,
      from_number: typeof m.from_number === "string" ? m.from_number : undefined,
      to_number: typeof m.to_number === "string" ? m.to_number : undefined,
      number: typeof m.number === "string" ? m.number : undefined,
      date_sent: typeof m.date_sent === "string" ? m.date_sent : undefined,
      service: typeof m.service === "string" ? m.service : undefined,
    };
  });
  return {
    status: typeof o.status === "string" ? o.status : undefined,
    data,
  };
}
