/**
 * Inbound receive + outbound status webhook normalization + optional secret verify.
 * Pure functions for unit tests; worker maps HTTP body onto these.
 */

export type {
  InboundMessage,
  OutboundStatus,
  CallLogEvent,
  NormalizedWebhook,
  WebhookHandlerResult,
} from "./webhooks/types.js";
export { normalizeWebhookBody } from "./webhooks/normalize.js";
export { verifyWebhookSecret } from "./webhooks/verify.js";
export {
  handleWebhookRequest,
  inboundSenderE164,
} from "./webhooks/handle.js";
