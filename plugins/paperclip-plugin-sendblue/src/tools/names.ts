/** Canonical agent tool name list. */

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
