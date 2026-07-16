/**
 * Agent-tool schemas and pure handlers (no Paperclip runtime).
 * Worker maps tool RPC onto these after resolving credentials/allowlist.
 */

import { assertAllowlisted, type AllowlistPolicy } from "./allowlist.js";
import { assertE164 } from "./e164.js";
import {
  type Credentials,
  prepareAddContact,
  prepareCreateAccountWebhook,
  prepareCreateGroup,
  prepareDeleteAccountWebhooks,
  prepareEvaluateService,
  prepareGetStatus,
  prepareListAccountWebhooks,
  prepareListContacts,
  prepareListLines,
  prepareListMessages,
  prepareMarkRead,
  prepareSendGroupMessage,
  prepareSendMessage,
  prepareSendReaction,
  prepareSendTypingIndicator,
  type PreparedRequest,
  type SendMessageInput,
} from "./sendblue-api.js";
import { asString } from "./util.js";

export type ToolContext = {
  creds: Credentials;
  fromNumber?: string;
  allowlist: AllowlistPolicy;
};

export type ToolResult = {
  ok: boolean;
  /** Prepared HTTP request (tests assert no send when denied). */
  request?: PreparedRequest;
  error?: string;
  data?: unknown;
};

export const TOOL_NAMES = [
  "send_message",
  "send_group_message",
  "send_reaction",
  "send_typing_indicator",
  "mark_read",
  "create_group",
  "list_lines",
  "list_messages",
  "get_status",
  "list_contacts",
  "add_contact",
  "evaluate_service",
  "list_account_webhooks",
  "create_account_webhook",
  "delete_account_webhook",
] as const;

export type ToolName = (typeof TOOL_NAMES)[number];

export const toolParameterSchemas: Record<
  ToolName,
  { type: string; properties: Record<string, unknown>; required?: string[] }
> = {
  send_message: {
    type: "object",
    properties: {
      number: { type: "string", description: "Recipient E.164" },
      content: { type: "string", description: "Message body" },
      media_url: { type: "string", description: "Optional media CDN URL" },
      from_number: { type: "string", description: "Override line E.164" },
      status_callback: {
        type: "string",
        description: "Per-message status webhook URL (https)",
      },
      send_style: {
        type: "string",
        description: "iMessage expressive style if supported",
      },
    },
    required: ["number"],
  },
  send_group_message: {
    type: "object",
    properties: {
      numbers: { type: "array", items: { type: "string" } },
      content: { type: "string" },
      media_url: { type: "string" },
      from_number: { type: "string" },
      group_id: { type: "string" },
    },
    required: ["numbers"],
  },
  send_reaction: {
    type: "object",
    properties: {
      number: { type: "string" },
      message_handle: { type: "string" },
      reaction: {
        type: "string",
        description: "love | like | dislike | laugh | emphasize | question",
      },
      from_number: { type: "string" },
    },
    required: ["number", "message_handle", "reaction"],
  },
  send_typing_indicator: {
    type: "object",
    properties: {
      number: { type: "string" },
      from_number: { type: "string" },
    },
    required: ["number"],
  },
  mark_read: {
    type: "object",
    properties: {
      number: { type: "string" },
      from_number: { type: "string" },
    },
    required: ["number"],
  },
  create_group: {
    type: "object",
    properties: {
      numbers: { type: "array", items: { type: "string" } },
      from_number: { type: "string" },
    },
    required: ["numbers"],
  },
  list_lines: { type: "object", properties: {} },
  list_messages: {
    type: "object",
    properties: {
      limit: { type: "number" },
      is_outbound: { type: "boolean" },
      number: { type: "string" },
    },
  },
  get_status: {
    type: "object",
    properties: {
      message_handle: { type: "string" },
    },
    required: ["message_handle"],
  },
  list_contacts: { type: "object", properties: {} },
  add_contact: {
    type: "object",
    properties: {
      number: { type: "string" },
      first_name: { type: "string" },
      last_name: { type: "string" },
    },
    required: ["number"],
  },
  evaluate_service: {
    type: "object",
    properties: { number: { type: "string" } },
    required: ["number"],
  },
  list_account_webhooks: { type: "object", properties: {} },
  create_account_webhook: {
    type: "object",
    properties: {
      url: { type: "string", description: "HTTPS webhook URL" },
      type: {
        type: "string",
        description:
          "receive | outbound | typing_indicator | call_log | contact_created | …",
      },
      secret: { type: "string", description: "Per-webhook secret" },
      globalSecret: { type: "string" },
    },
    required: ["url"],
  },
  delete_account_webhook: {
    type: "object",
    properties: {
      url: { type: "string" },
      type: { type: "string" },
    },
    required: ["url"],
  },
};

function str(params: Record<string, unknown>, key: string): string | undefined {
  const v = params[key];
  return typeof v === "string" ? v : undefined;
}

function resolveFrom(
  params: Record<string, unknown>,
  ctx: ToolContext,
): string {
  const from = str(params, "from_number") ?? ctx.fromNumber;
  if (!from) {
    throw new Error(
      "from_number required (config fromNumber or tool param)",
    );
  }
  return assertE164(from, "from_number");
}

/**
 * Build a PreparedRequest for a tool call, enforcing allowlist.
 * Does not perform HTTP — caller executes or tests the request.
 */
export function buildToolRequest(
  name: string,
  params: Record<string, unknown>,
  ctx: ToolContext,
): ToolResult {
  try {
    if (!TOOL_NAMES.includes(name as ToolName)) {
      return { ok: false, error: `unknown tool: ${name}` };
    }
    switch (name as ToolName) {
      case "send_message": {
        const number = assertAllowlisted(asString(params.number), ctx.allowlist);
        const from_number = resolveFrom(params, ctx);
        const input: SendMessageInput = {
          number,
          from_number,
          content: str(params, "content"),
          media_url: str(params, "media_url"),
          status_callback: str(params, "status_callback"),
          send_style: str(params, "send_style"),
        };
        return { ok: true, request: prepareSendMessage(ctx.creds, input) };
      }
      case "send_group_message": {
        const numbersRaw = params.numbers;
        if (!Array.isArray(numbersRaw)) {
          return { ok: false, error: "numbers array required" };
        }
        const numbers = numbersRaw.map((n) =>
          assertAllowlisted(asString(n), ctx.allowlist),
        );
        const from_number = resolveFrom(params, ctx);
        return {
          ok: true,
          request: prepareSendGroupMessage(ctx.creds, {
            numbers,
            from_number,
            content: str(params, "content"),
            media_url: str(params, "media_url"),
            group_id: str(params, "group_id"),
          }),
        };
      }
      case "send_reaction": {
        const number = assertAllowlisted(asString(params.number), ctx.allowlist);
        return {
          ok: true,
          request: prepareSendReaction(ctx.creds, {
            number,
            from_number: resolveFrom(params, ctx),
            message_handle: asString(params.message_handle),
            reaction: asString(params.reaction),
          }),
        };
      }
      case "send_typing_indicator": {
        const number = assertAllowlisted(asString(params.number), ctx.allowlist);
        return {
          ok: true,
          request: prepareSendTypingIndicator(ctx.creds, {
            number,
            from_number: resolveFrom(params, ctx),
          }),
        };
      }
      case "mark_read": {
        const number = assertAllowlisted(asString(params.number), ctx.allowlist);
        return {
          ok: true,
          request: prepareMarkRead(ctx.creds, {
            number,
            from_number: resolveFrom(params, ctx),
          }),
        };
      }
      case "create_group": {
        const numbersRaw = params.numbers;
        if (!Array.isArray(numbersRaw)) {
          return { ok: false, error: "numbers array required" };
        }
        const numbers = numbersRaw.map((n) =>
          assertAllowlisted(asString(n), ctx.allowlist),
        );
        return {
          ok: true,
          request: prepareCreateGroup(ctx.creds, {
            numbers,
            from_number: resolveFrom(params, ctx),
          }),
        };
      }
      case "list_lines":
        return { ok: true, request: prepareListLines(ctx.creds) };
      case "list_messages": {
        const number = str(params, "number");
        if (number) assertAllowlisted(number, ctx.allowlist);
        return {
          ok: true,
          request: prepareListMessages(ctx.creds, {
            limit: typeof params.limit === "number" ? params.limit : undefined,
            is_outbound:
              typeof params.is_outbound === "boolean"
                ? params.is_outbound
                : undefined,
            number,
          }),
        };
      }
      case "get_status":
        return {
          ok: true,
          request: prepareGetStatus(ctx.creds, asString(params.message_handle)),
        };
      case "list_contacts":
        return { ok: true, request: prepareListContacts(ctx.creds) };
      case "add_contact": {
        const number = assertAllowlisted(asString(params.number), ctx.allowlist);
        return {
          ok: true,
          request: prepareAddContact(ctx.creds, {
            number,
            first_name: str(params, "first_name"),
            last_name: str(params, "last_name"),
          }),
        };
      }
      case "evaluate_service": {
        const number = assertE164(asString(params.number), "number");
        return {
          ok: true,
          request: prepareEvaluateService(ctx.creds, number),
        };
      }
      case "list_account_webhooks":
        return { ok: true, request: prepareListAccountWebhooks(ctx.creds) };
      case "create_account_webhook":
        return {
          ok: true,
          request: prepareCreateAccountWebhook(ctx.creds, {
            url: asString(params.url),
            type: str(params, "type"),
            secret: str(params, "secret"),
            globalSecret: str(params, "globalSecret"),
          }),
        };
      case "delete_account_webhook":
        return {
          ok: true,
          request: prepareDeleteAccountWebhooks(ctx.creds, {
            urls: [asString(params.url)],
            type: str(params, "type"),
          }),
        };
      default:
        return { ok: false, error: `unhandled tool: ${name}` };
    }
  } catch (e) {
    return {
      ok: false,
      error: e instanceof Error ? e.message : String(e),
    };
  }
}
