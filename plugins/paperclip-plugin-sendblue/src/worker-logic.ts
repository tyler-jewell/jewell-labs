/**
 * Host-facing orchestration without SDK side effects (no runWorker).
 * Safe to import from tests and package index.
 */

export {
  type ResolvedSecrets,
  type BoardRouting,
  resolvedFromValidated,
  resolveRuntimeConfig,
} from "./runtime/resolved.js";
export { executeToolForHost } from "./runtime/execute-tool.js";
export {
  processInboundWebhook,
  denyInboundIfNotAllowlisted,
  planInboundAction,
  type InboundAction,
} from "./runtime/inbound.js";
export { maybeNotifyFromEvent } from "./runtime/notify.js";
// Conversation helpers are used via worker/*; keep package index lean (see index.ts).