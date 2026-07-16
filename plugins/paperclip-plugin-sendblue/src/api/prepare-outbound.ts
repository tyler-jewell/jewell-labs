/** Outbound send / typing / read / group builders. */

import { PATHS } from "../constants.js";
import { assertE164 } from "../e164.js";
import { prepareJson } from "./http.js";
import {
  type Credentials,
  type PreparedRequest,
  type SendGroupMessageInput,
  type SendMessageInput,
  type SendReactionInput,
  SendBlueClientError,
} from "./types.js";

export function prepareSendMessage(
  creds: Credentials,
  input: SendMessageInput,
): PreparedRequest {
  const number = assertE164(input.number, "number");
  const from_number = assertE164(input.from_number, "from_number");
  if (!input.content?.trim() && !input.media_url?.trim()) {
    throw new SendBlueClientError(
      "send-message requires content and/or media_url",
      "validation",
    );
  }
  const body: Record<string, unknown> = { number, from_number };
  if (input.content?.trim()) body.content = input.content.trim();
  if (input.media_url?.trim()) body.media_url = input.media_url.trim();
  if (input.status_callback?.trim()) {
    body.status_callback = input.status_callback.trim();
  }
  if (input.send_style?.trim()) body.send_style = input.send_style.trim();
  return prepareJson(creds, "POST", PATHS.sendMessage, body);
}

export function prepareSendGroupMessage(
  creds: Credentials,
  input: SendGroupMessageInput,
): PreparedRequest {
  if (!Array.isArray(input.numbers) || input.numbers.length < 2) {
    throw new SendBlueClientError(
      "send-group-message requires at least 2 numbers",
      "validation",
    );
  }
  const numbers = input.numbers.map((n, i) => assertE164(n, `numbers[${i}]`));
  const from_number = assertE164(input.from_number, "from_number");
  if (!input.content?.trim() && !input.media_url?.trim()) {
    throw new SendBlueClientError(
      "send-group-message requires content and/or media_url",
      "validation",
    );
  }
  const body: Record<string, unknown> = { numbers, from_number };
  if (input.content?.trim()) body.content = input.content.trim();
  if (input.media_url?.trim()) body.media_url = input.media_url.trim();
  if (input.status_callback?.trim()) {
    body.status_callback = input.status_callback.trim();
  }
  if (input.group_id?.trim()) body.group_id = input.group_id.trim();
  return prepareJson(creds, "POST", PATHS.sendGroupMessage, body);
}

export function prepareSendReaction(
  creds: Credentials,
  input: SendReactionInput,
): PreparedRequest {
  const number = assertE164(input.number, "number");
  const from_number = assertE164(input.from_number, "from_number");
  if (!input.message_handle.trim()) {
    throw new SendBlueClientError("message_handle is required", "validation");
  }
  if (!input.reaction.trim()) {
    throw new SendBlueClientError("reaction is required", "validation");
  }
  return prepareJson(creds, "POST", PATHS.sendReaction, {
    number,
    from_number,
    message_handle: input.message_handle.trim(),
    reaction: input.reaction.trim(),
  });
}

export function prepareSendTypingIndicator(
  creds: Credentials,
  input: {
    number: string;
    from_number: string;
    /** "start" (default) | "stop" — typing-v2 */
    state?: "start" | "stop";
    /** 1–300000 ms visible duration while state=start */
    max_duration_ms?: number;
  },
): PreparedRequest {
  const number = assertE164(input.number, "number");
  const from_number = assertE164(input.from_number, "from_number");
  const body: Record<string, unknown> = { number, from_number };
  const state: "start" | "stop" = input.state === "stop" ? "stop" : "start";
  body.state = state;
  if (state === "start" && input.max_duration_ms != null) {
    const ms = input.max_duration_ms;
    if (!Number.isInteger(ms) || ms < 1 || ms > 300_000) {
      throw new SendBlueClientError(
        "max_duration_ms must be an integer 1–300000",
        "validation",
      );
    }
    body.max_duration_ms = ms;
  }
  return prepareJson(creds, "POST", PATHS.sendTyping, body);
}

export function prepareMarkRead(
  creds: Credentials,
  input: { number: string; from_number: string },
): PreparedRequest {
  const number = assertE164(input.number, "number");
  const from_number = assertE164(input.from_number, "from_number");
  return prepareJson(creds, "POST", PATHS.markRead, { number, from_number });
}

export function prepareCreateGroup(
  creds: Credentials,
  input: { numbers: string[]; from_number: string },
): PreparedRequest {
  if (!Array.isArray(input.numbers) || input.numbers.length < 2) {
    throw new SendBlueClientError(
      "create-group requires at least 2 numbers",
      "validation",
    );
  }
  const numbers = input.numbers.map((n, i) => assertE164(n, `numbers[${i}]`));
  const from_number = assertE164(input.from_number, "from_number");
  return prepareJson(creds, "POST", PATHS.createGroup, {
    numbers,
    from_number,
  });
}
