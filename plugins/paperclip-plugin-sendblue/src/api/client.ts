/** Optional live Client wrapping prepare + execute + parse. */

import { credentialsFromEnv, executePrepared } from "./http.js";
import {
  parseLinesResponse,
  parseMessagesResponse,
  parseSendMessageResponse,
} from "./parse.js";
import {
  prepareCreateAccountWebhook,
  prepareListAccountWebhooks,
} from "./prepare-account.js";
import {
  prepareMarkRead,
  prepareSendGroupMessage,
  prepareSendMessage,
  prepareSendReaction,
  prepareSendTypingIndicator,
} from "./prepare-outbound.js";
import {
  prepareAddContact,
  prepareEvaluateService,
  prepareGetStatus,
  prepareListContacts,
  prepareListLines,
  prepareListMessages,
} from "./prepare-query.js";
import type {
  Credentials,
  ListMessagesFilters,
  LinesResult,
  PreparedRequest,
  SendGroupMessageInput,
  SendMessageInput,
  SendMessageResult,
  SendReactionInput,
} from "./types.js";

export class SendBlueClient {
  constructor(
    readonly creds: Credentials,
    private readonly fetchImpl: typeof fetch = fetch,
  ) {}

  static fromEnv(fetchImpl: typeof fetch = fetch): SendBlueClient {
    return new SendBlueClient(credentialsFromEnv(), fetchImpl);
  }

  private exec(prep: PreparedRequest): Promise<unknown> {
    return executePrepared(prep, this.fetchImpl);
  }

  async listLines(): Promise<LinesResult> {
    return parseLinesResponse(await this.exec(prepareListLines(this.creds)));
  }

  async sendMessage(input: SendMessageInput): Promise<SendMessageResult> {
    return parseSendMessageResponse(
      await this.exec(prepareSendMessage(this.creds, input)),
    );
  }

  async sendGroupMessage(
    input: SendGroupMessageInput,
  ): Promise<SendMessageResult> {
    return parseSendMessageResponse(
      await this.exec(prepareSendGroupMessage(this.creds, input)),
    );
  }

  async sendReaction(input: SendReactionInput): Promise<unknown> {
    return this.exec(prepareSendReaction(this.creds, input));
  }

  async sendTypingIndicator(input: {
    number: string;
    from_number: string;
  }): Promise<unknown> {
    return this.exec(prepareSendTypingIndicator(this.creds, input));
  }

  async markRead(input: {
    number: string;
    from_number: string;
  }): Promise<unknown> {
    return this.exec(prepareMarkRead(this.creds, input));
  }

  async listMessages(filters?: ListMessagesFilters) {
    return parseMessagesResponse(
      await this.exec(prepareListMessages(this.creds, filters)),
    );
  }

  async listContacts(): Promise<unknown> {
    return this.exec(prepareListContacts(this.creds));
  }

  async addContact(input: {
    number: string;
    first_name?: string;
    last_name?: string;
  }): Promise<unknown> {
    return this.exec(prepareAddContact(this.creds, input));
  }

  async evaluateService(number: string): Promise<unknown> {
    return this.exec(prepareEvaluateService(this.creds, number));
  }

  async getStatus(messageHandle: string): Promise<unknown> {
    return this.exec(prepareGetStatus(this.creds, messageHandle));
  }

  async listAccountWebhooks(): Promise<unknown> {
    return this.exec(prepareListAccountWebhooks(this.creds));
  }

  async createAccountWebhook(input: {
    url: string;
    type?: string;
    secret?: string;
    globalSecret?: string;
  }): Promise<unknown> {
    return this.exec(prepareCreateAccountWebhook(this.creds, input));
  }
}
