/** List / get / evaluate request builders. */

import { PATHS } from "../constants.js";
import { assertE164 } from "../e164.js";
import { prepareGet, prepareJson } from "./http.js";
import {
  type Credentials,
  type ListMessagesFilters,
  type PreparedRequest,
  SendBlueClientError,
} from "./types.js";

export function prepareListLines(creds: Credentials): PreparedRequest {
  return prepareGet(creds, PATHS.lines);
}

export function prepareListMessages(
  creds: Credentials,
  filters: ListMessagesFilters = {},
): PreparedRequest {
  const limit = Math.min(Math.max(filters.limit ?? 20, 1), 100);
  const q = new URLSearchParams();
  q.set("limit", String(limit));
  if (typeof filters.is_outbound === "boolean") {
    q.set("is_outbound", String(filters.is_outbound));
  }
  if (filters.number?.trim()) {
    q.set("number", assertE164(filters.number, "number"));
  }
  return prepareGet(creds, `${PATHS.messages}?${q.toString()}`);
}

export function prepareGetMessage(
  creds: Credentials,
  messageId: string,
): PreparedRequest {
  if (!messageId.trim()) {
    throw new SendBlueClientError("message id is required", "validation");
  }
  return prepareGet(
    creds,
    `${PATHS.messages}/${encodeURIComponent(messageId.trim())}`,
  );
}

export function prepareListContacts(creds: Credentials): PreparedRequest {
  return prepareGet(creds, PATHS.contacts);
}

export function prepareAddContact(
  creds: Credentials,
  input: { number: string; first_name?: string; last_name?: string },
): PreparedRequest {
  const number = assertE164(input.number, "number");
  const body: Record<string, unknown> = { number };
  if (input.first_name?.trim()) body.first_name = input.first_name.trim();
  if (input.last_name?.trim()) body.last_name = input.last_name.trim();
  return prepareJson(creds, "POST", PATHS.contacts, body);
}

export function prepareEvaluateService(
  creds: Credentials,
  number: string,
): PreparedRequest {
  const n = assertE164(number, "number");
  const q = new URLSearchParams({ number: n });
  return prepareGet(creds, `${PATHS.evaluateService}?${q.toString()}`);
}

export function prepareGetStatus(
  creds: Credentials,
  messageHandle: string,
): PreparedRequest {
  if (!messageHandle.trim()) {
    throw new SendBlueClientError("message_handle is required", "validation");
  }
  const q = new URLSearchParams({ message_handle: messageHandle.trim() });
  return prepareGet(creds, `${PATHS.status}?${q.toString()}`);
}
