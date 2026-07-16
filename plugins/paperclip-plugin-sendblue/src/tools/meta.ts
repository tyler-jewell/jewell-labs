/** Agent-facing display names + descriptions (tool-call context for LLMs). */

import type { ToolName } from "./names.js";

export type ToolMeta = { displayName: string; description: string };

export const TOOL_META: Record<ToolName, ToolMeta> = {
  send_message: {
    displayName: "Send message",
    description:
      "Send an iMessage/SMS to an allowlisted E.164 number. Prefer short plain text. Optional media_url.",
  },
  send_group_message: {
    displayName: "Send group message",
    description:
      "Send a message to a group (numbers[] or group_id). All recipients must be allowlisted.",
  },
  send_reaction: {
    displayName: "Send reaction",
    description:
      "React to a prior message (love|like|dislike|laugh|emphasize|question) via message_handle.",
  },
  send_typing_indicator: {
    displayName: "Send typing indicator",
    description:
      "Show the iMessage typing bubble to a number while you prepare a reply.",
  },
  mark_read: {
    displayName: "Mark read",
    description: "Mark the conversation with a number as read on SendBlue.",
  },
  create_group: {
    displayName: "Create group",
    description: "Create an iMessage group chat with allowlisted numbers.",
  },
  list_lines: {
    displayName: "List lines",
    description: "List SendBlue phone lines on this account (from-numbers).",
  },
  list_messages: {
    displayName: "List messages",
    description:
      "List recent messages; optional filter by number and is_outbound.",
  },
  get_status: {
    displayName: "Get message status",
    description: "Look up delivery status for a message_handle.",
  },
  list_contacts: {
    displayName: "List contacts",
    description: "List contacts stored in the SendBlue account.",
  },
  add_contact: {
    displayName: "Add contact",
    description: "Add or update a contact (E.164 number + optional names).",
  },
  evaluate_service: {
    displayName: "Evaluate service",
    description:
      "Check whether a number supports iMessage vs SMS (SendBlue evaluate).",
  },
  list_account_webhooks: {
    displayName: "List account webhooks",
    description: "List webhooks registered on the SendBlue account.",
  },
  create_account_webhook: {
    displayName: "Create account webhook",
    description:
      "Register a SendBlue webhook URL (type receive|status_callback|…). Mutates account.",
  },
  delete_account_webhook: {
    displayName: "Delete account webhook",
    description: "Remove a SendBlue account webhook by url + type.",
  },
};
