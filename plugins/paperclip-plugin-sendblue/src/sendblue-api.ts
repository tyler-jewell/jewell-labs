/**
 * Pure SendBlue HTTP layer: credentials, request builders, response parsers.
 * Unit-tested without network. Optional live Client uses injected fetch.
 *
 * API surface matches https://docs.sendblue.com/api-v2/ (as of 2026-07).
 */

export {
  HEADER_API_KEY,
  HEADER_API_SECRET,
  SENDBLUE_API_BASE,
  PATHS,
} from "./constants.js";

export type {
  Credentials,
  PreparedRequest,
  SendMessageInput,
  SendGroupMessageInput,
  SendReactionInput,
  ListMessagesFilters,
  LinesResult,
  SendMessageResult,
  MessageRow,
} from "./api/types.js";
export { SendBlueClientError } from "./api/types.js";

export {
  credentialsFromEnv,
  fromNumberFromEnv,
  authHeaders,
  executePrepared,
} from "./api/http.js";

export {
  prepareSendMessage,
  prepareSendGroupMessage,
  prepareSendReaction,
  prepareSendTypingIndicator,
  prepareMarkRead,
  prepareCreateGroup,
} from "./api/prepare-outbound.js";

export {
  prepareListLines,
  prepareListMessages,
  prepareGetMessage,
  prepareListContacts,
  prepareAddContact,
  prepareEvaluateService,
  prepareGetStatus,
} from "./api/prepare-query.js";

export {
  prepareListAccountWebhooks,
  prepareCreateAccountWebhook,
  prepareAddAccountWebhook,
  prepareReplaceAccountWebhooks,
  prepareUpdateAccountWebhook,
  prepareDeleteAccountWebhooks,
  prepareDeleteAccountWebhook,
} from "./api/prepare-account.js";

export {
  parseLinesResponse,
  parseSendMessageResponse,
  parseMessagesResponse,
} from "./api/parse.js";

export { SendBlueClient } from "./api/client.js";
