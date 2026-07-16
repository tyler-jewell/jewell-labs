/** JSON Schema parameter definitions for each tool. */

import type { ToolName } from "./names.js";

export type ToolParamSchema = {
  type: string;
  properties: Record<string, unknown>;
  required?: string[];
};

export const toolParameterSchemas: Record<ToolName, ToolParamSchema> = {
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
    properties: { message_handle: { type: "string" } },
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
