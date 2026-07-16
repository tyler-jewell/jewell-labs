/** Pure tool → PreparedRequest builder with allowlist enforcement. */

import { assertAllowlisted, type AllowlistPolicy } from "../allowlist.js";
import { assertE164 } from "../e164.js";
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
} from "../sendblue-api.js";
import { asString } from "../util.js";
import { TOOL_NAMES, type ToolName } from "./names.js";

export type ToolContext = {
  creds: Credentials;
  fromNumber?: string;
  allowlist: AllowlistPolicy;
};

export type ToolResult = {
  ok: boolean;
  request?: PreparedRequest;
  error?: string;
  data?: unknown;
};

function str(params: Record<string, unknown>, key: string): string | undefined {
  const v = params[key];
  return typeof v === "string" ? v : undefined;
}

function resolveFrom(params: Record<string, unknown>, ctx: ToolContext): string {
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
        return {
          ok: true,
          request: prepareSendGroupMessage(ctx.creds, {
            numbers,
            from_number: resolveFrom(params, ctx),
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
        const maxRaw = params.max_duration_ms ?? params.maxDurationMs;
        const max_duration_ms =
          typeof maxRaw === "number"
            ? maxRaw
            : typeof maxRaw === "string" && maxRaw.trim()
              ? Number(maxRaw)
              : 120_000;
        const stateRaw = asString(params.state);
        const state =
          stateRaw === "stop" ? ("stop" as const) : ("start" as const);
        return {
          ok: true,
          request: prepareSendTypingIndicator(ctx.creds, {
            number,
            from_number: resolveFrom(params, ctx),
            state,
            max_duration_ms:
              state === "start" && Number.isFinite(max_duration_ms)
                ? max_duration_ms
                : undefined,
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
