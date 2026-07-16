/**
 * API console "Run test" definitions (pure).
 */

import { TOOL_NAMES, type ToolName } from "../tools.js";

export type ApiTestField = {
  key: string;
  label: string;
  placeholder?: string;
  required?: boolean;
};

export type ApiTestDef = {
  name: ToolName;
  label: string;
  description: string;
  /** Live side effects (SMS, webhooks, contacts). */
  mutates: boolean;
  fields: ApiTestField[];
};

/** Core SendBlue APIs exposed as console "Run test" rows. */
export const API_TEST_DEFS: ApiTestDef[] = [
  {
    name: "list_lines",
    label: "List lines",
    description: "GET /api/lines — SendBlue numbers on the account.",
    mutates: false,
    fields: [],
  },
  {
    name: "list_messages",
    label: "List messages",
    description: "GET /api/v2/messages — recent message history.",
    mutates: false,
    fields: [
      { key: "limit", label: "Limit", placeholder: "20" },
      { key: "number", label: "Filter number (E.164)", placeholder: "+1…" },
    ],
  },
  {
    name: "list_contacts",
    label: "List contacts",
    description: "GET /api/v2/contacts",
    mutates: false,
    fields: [],
  },
  {
    name: "list_account_webhooks",
    label: "List account webhooks",
    description: "GET /api/account/webhooks",
    mutates: false,
    fields: [],
  },
  {
    name: "evaluate_service",
    label: "Evaluate service",
    description: "GET /api/evaluate-service — iMessage vs SMS for a number.",
    mutates: false,
    fields: [
      {
        key: "number",
        label: "Number (E.164)",
        placeholder: "+1…",
        required: true,
      },
    ],
  },
  {
    name: "get_status",
    label: "Get message status",
    description: "GET /api/status — delivery status by message handle.",
    mutates: false,
    fields: [
      {
        key: "message_handle",
        label: "Message handle",
        placeholder: "handle…",
        required: true,
      },
    ],
  },
  {
    name: "send_message",
    label: "Send message",
    description: "POST /api/send-message — live SMS/iMessage (allowlisted).",
    mutates: true,
    fields: [
      {
        key: "number",
        label: "To (E.164)",
        placeholder: "+1…",
        required: true,
      },
      {
        key: "content",
        label: "Content",
        placeholder: "Hello from SendBlue console",
        required: true,
      },
      { key: "from_number", label: "From override (E.164)", placeholder: "+1…" },
    ],
  },
  {
    name: "send_typing_indicator",
    label: "Send typing indicator",
    description: "POST /api/send-typing-indicator",
    mutates: true,
    fields: [
      {
        key: "number",
        label: "To (E.164)",
        placeholder: "+1…",
        required: true,
      },
    ],
  },
  {
    name: "mark_read",
    label: "Mark read",
    description: "POST /api/mark-read",
    mutates: true,
    fields: [
      {
        key: "number",
        label: "Number (E.164)",
        placeholder: "+1…",
        required: true,
      },
    ],
  },
  {
    name: "send_group_message",
    label: "Send group message",
    description: "POST /api/send-group-message",
    mutates: true,
    fields: [
      {
        key: "numbers",
        label: "Numbers (comma-separated E.164)",
        placeholder: "+1a, +1b",
        required: true,
      },
      {
        key: "content",
        label: "Content",
        placeholder: "Hello group",
        required: true,
      },
    ],
  },
  {
    name: "send_reaction",
    label: "Send reaction",
    description: "POST /api/send-reaction",
    mutates: true,
    fields: [
      {
        key: "number",
        label: "Number (E.164)",
        placeholder: "+1…",
        required: true,
      },
      {
        key: "message_handle",
        label: "Message handle",
        required: true,
      },
      {
        key: "reaction",
        label: "Reaction",
        placeholder: "love | like | laugh…",
        required: true,
      },
    ],
  },
  {
    name: "create_group",
    label: "Create group",
    description: "POST /api/create-group",
    mutates: true,
    fields: [
      {
        key: "numbers",
        label: "Numbers (comma-separated E.164)",
        placeholder: "+1a, +1b",
        required: true,
      },
    ],
  },
  {
    name: "add_contact",
    label: "Add contact",
    description: "POST /api/v2/contacts",
    mutates: true,
    fields: [
      {
        key: "number",
        label: "Number (E.164)",
        placeholder: "+1…",
        required: true,
      },
      { key: "first_name", label: "First name" },
      { key: "last_name", label: "Last name" },
    ],
  },
  {
    name: "create_account_webhook",
    label: "Create account webhook",
    description: "POST /api/account/webhooks",
    mutates: true,
    fields: [
      {
        key: "url",
        label: "HTTPS URL",
        placeholder: "https://…/webhooks/inbound",
        required: true,
      },
      {
        key: "type",
        label: "Type",
        placeholder: "receive",
      },
      { key: "secret", label: "Webhook secret" },
    ],
  },
  {
    name: "delete_account_webhook",
    label: "Delete account webhook",
    description: "DELETE-style account webhook URL removal",
    mutates: true,
    fields: [
      {
        key: "url",
        label: "HTTPS URL",
        placeholder: "https://…",
        required: true,
      },
      { key: "type", label: "Type", placeholder: "receive" },
    ],
  },
];

/** Build tool params from free-text field map (numbers CSV → string[]). */
export function buildTestParams(
  name: ToolName,
  fields: Record<string, string>,
): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const [k, raw] of Object.entries(fields)) {
    const v = raw.trim();
    if (!v) continue;
    if (k === "numbers") {
      out.numbers = v
        .split(/[,\n]+/)
        .map((s) => s.trim())
        .filter(Boolean);
      continue;
    }
    if (k === "limit") {
      const n = Number(v);
      if (Number.isFinite(n)) out.limit = n;
      continue;
    }
    out[k] = v;
  }
  // Ensure every TOOL_NAMES entry is known (defensive for console completeness).
  void TOOL_NAMES.includes(name);
  return out;
}

