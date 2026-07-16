import { PLUGIN_VERSION } from "./version.js";

export const PLUGIN_ID = "jewell-labs.sendblue";
export { PLUGIN_VERSION };
export const PLUGIN_DISPLAY_NAME = "SendBlue (iMessage/SMS)";

/** Official SendBlue API origin. */
export const SENDBLUE_API_BASE = "https://api.sendblue.com";

export const PATHS = {
  lines: "/api/lines",
  sendMessage: "/api/send-message",
  sendGroupMessage: "/api/send-group-message",
  sendReaction: "/api/send-reaction",
  sendTyping: "/api/send-typing-indicator",
  markRead: "/api/mark-read",
  createGroup: "/api/create-group",
  messages: "/api/v2/messages",
  contacts: "/api/v2/contacts",
  accountWebhooks: "/api/account/webhooks",
  status: "/api/status",
  evaluateService: "/api/evaluate-service",
} as const;

export const HEADER_API_KEY = "sb-api-key-id";
export const HEADER_API_SECRET = "sb-api-secret-key";

/** Company secret key names (Paperclip vault). */
export const SECRET_KEYS = {
  apiKey: "sendblue-api-key",
  apiSecret: "sendblue-api-secret",
  fromNumber: "sendblue-from-number",
  webhookSecret: "sendblue-webhook-secret",
} as const;

/** Env names accepted when loading credentials (CLI + MCP aliases). */
export const ENV_API_KEY = ["SENDBLUE_API_KEY", "SENDBLUE_API_API_KEY"] as const;
export const ENV_API_SECRET = ["SENDBLUE_API_SECRET", "SENDBLUE_API_API_SECRET"] as const;
export const ENV_FROM_NUMBER = ["SENDBLUE_FROM_NUMBER"] as const;

/** Webhook endpoint keys declared in the manifest. */
export const WEBHOOK_ENDPOINT_KEYS = {
  inbound: "inbound",
} as const;

/** Known SendBlue webhook type strings for account webhook management. */
export const WEBHOOK_TYPES = [
  "receive",
  "outbound",
  "typing_indicator",
  "call_log",
  "line_blocked",
  "line_assigned",
  "contact_created",
] as const;

export type WebhookType = (typeof WEBHOOK_TYPES)[number];
