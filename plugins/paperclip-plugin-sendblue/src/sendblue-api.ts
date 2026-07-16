/**
 * Pure SendBlue HTTP layer: credentials, request builders, response parsers.
 * Unit-tested without network. Optional live Client uses injected fetch.
 *
 * API surface matches https://docs.sendblue.com/api-v2/ (as of 2026-07).
 */

import {
  ENV_API_KEY,
  ENV_API_SECRET,
  ENV_FROM_NUMBER,
  HEADER_API_KEY,
  HEADER_API_SECRET,
  PATHS,
  SENDBLUE_API_BASE,
} from "./constants.js";
import { assertE164 } from "./e164.js";

export { HEADER_API_KEY, HEADER_API_SECRET, SENDBLUE_API_BASE, PATHS };

export type Credentials = {
  apiKey: string;
  apiSecret: string;
};

export type PreparedRequest = {
  method: "GET" | "POST" | "PUT" | "DELETE" | "PATCH";
  url: string;
  headers: Record<string, string>;
  body?: string;
};

export class SendBlueClientError extends Error {
  constructor(
    message: string,
    readonly code:
      | "missing_credentials"
      | "http"
      | "api"
      | "json"
      | "validation" = "http",
    readonly status?: number,
    readonly body?: string,
  ) {
    super(message);
    this.name = "SendBlueClientError";
  }
}

function firstEnv(names: readonly string[]): string | undefined {
  for (const n of names) {
    const v = process.env[n];
    if (typeof v === "string" && v.trim()) return v.trim();
  }
  return undefined;
}

export function credentialsFromEnv(): Credentials {
  const apiKey = firstEnv(ENV_API_KEY);
  const apiSecret = firstEnv(ENV_API_SECRET);
  if (!apiKey || !apiSecret) {
    throw new SendBlueClientError(
      "missing Sendblue credentials: set SENDBLUE_API_KEY and SENDBLUE_API_SECRET (or SENDBLUE_API_API_KEY / SENDBLUE_API_API_SECRET)",
      "missing_credentials",
    );
  }
  return { apiKey, apiSecret };
}

export function fromNumberFromEnv(): string | undefined {
  return firstEnv(ENV_FROM_NUMBER);
}

export function authHeaders(creds: Credentials): Record<string, string> {
  return {
    [HEADER_API_KEY]: creds.apiKey,
    [HEADER_API_SECRET]: creds.apiSecret,
  };
}

function jsonHeaders(creds: Credentials): Record<string, string> {
  return {
    ...authHeaders(creds),
    "Content-Type": "application/json",
    Accept: "application/json",
  };
}

function getUrl(pathAndQuery: string): string {
  if (pathAndQuery.startsWith("http")) return pathAndQuery;
  return `${SENDBLUE_API_BASE}${pathAndQuery.startsWith("/") ? "" : "/"}${pathAndQuery}`;
}

function prepareGet(creds: Credentials, pathAndQuery: string): PreparedRequest {
  return {
    method: "GET",
    url: getUrl(pathAndQuery),
    headers: authHeaders(creds),
  };
}

function prepareJson(
  creds: Credentials,
  method: PreparedRequest["method"],
  path: string,
  body: unknown,
): PreparedRequest {
  return {
    method,
    url: getUrl(path),
    headers: jsonHeaders(creds),
    body: JSON.stringify(body),
  };
}

// --- Request builders ---

export function prepareListLines(creds: Credentials): PreparedRequest {
  return prepareGet(creds, PATHS.lines);
}

export type SendMessageInput = {
  number: string;
  from_number: string;
  content?: string;
  media_url?: string;
  status_callback?: string;
  send_style?: string;
};

export function prepareSendMessage(
  creds: Credentials,
  input: SendMessageInput,
): PreparedRequest {
  const number = assertE164(input.number, "number");
  const from_number = assertE164(input.from_number, "from_number");
  if (!input.content?.trim() && !input.media_url?.trim()) {
    throw new SendBlueClientError(
      "send-message requires content and/or media_url",
      "validation",
    );
  }
  const body: Record<string, unknown> = { number, from_number };
  if (input.content?.trim()) body.content = input.content.trim();
  if (input.media_url?.trim()) body.media_url = input.media_url.trim();
  if (input.status_callback?.trim()) {
    body.status_callback = input.status_callback.trim();
  }
  if (input.send_style?.trim()) body.send_style = input.send_style.trim();
  return prepareJson(creds, "POST", PATHS.sendMessage, body);
}

export type SendGroupMessageInput = {
  numbers: string[];
  from_number: string;
  content?: string;
  media_url?: string;
  status_callback?: string;
  group_id?: string;
};

export function prepareSendGroupMessage(
  creds: Credentials,
  input: SendGroupMessageInput,
): PreparedRequest {
  if (!Array.isArray(input.numbers) || input.numbers.length < 2) {
    throw new SendBlueClientError(
      "send-group-message requires at least 2 numbers",
      "validation",
    );
  }
  const numbers = input.numbers.map((n, i) => assertE164(n, `numbers[${i}]`));
  const from_number = assertE164(input.from_number, "from_number");
  if (!input.content?.trim() && !input.media_url?.trim()) {
    throw new SendBlueClientError(
      "send-group-message requires content and/or media_url",
      "validation",
    );
  }
  const body: Record<string, unknown> = { numbers, from_number };
  if (input.content?.trim()) body.content = input.content.trim();
  if (input.media_url?.trim()) body.media_url = input.media_url.trim();
  if (input.status_callback?.trim()) {
    body.status_callback = input.status_callback.trim();
  }
  if (input.group_id?.trim()) body.group_id = input.group_id.trim();
  return prepareJson(creds, "POST", PATHS.sendGroupMessage, body);
}

export type SendReactionInput = {
  number: string;
  from_number: string;
  message_handle: string;
  reaction: string;
};

export function prepareSendReaction(
  creds: Credentials,
  input: SendReactionInput,
): PreparedRequest {
  const number = assertE164(input.number, "number");
  const from_number = assertE164(input.from_number, "from_number");
  if (!input.message_handle.trim()) {
    throw new SendBlueClientError("message_handle is required", "validation");
  }
  if (!input.reaction.trim()) {
    throw new SendBlueClientError("reaction is required", "validation");
  }
  return prepareJson(creds, "POST", PATHS.sendReaction, {
    number,
    from_number,
    message_handle: input.message_handle.trim(),
    reaction: input.reaction.trim(),
  });
}

export function prepareSendTypingIndicator(
  creds: Credentials,
  input: { number: string; from_number: string },
): PreparedRequest {
  const number = assertE164(input.number, "number");
  const from_number = assertE164(input.from_number, "from_number");
  return prepareJson(creds, "POST", PATHS.sendTyping, { number, from_number });
}

export function prepareMarkRead(
  creds: Credentials,
  input: { number: string; from_number: string },
): PreparedRequest {
  const number = assertE164(input.number, "number");
  const from_number = assertE164(input.from_number, "from_number");
  return prepareJson(creds, "POST", PATHS.markRead, { number, from_number });
}

export function prepareCreateGroup(
  creds: Credentials,
  input: { numbers: string[]; from_number: string },
): PreparedRequest {
  if (!Array.isArray(input.numbers) || input.numbers.length < 2) {
    throw new SendBlueClientError(
      "create-group requires at least 2 numbers",
      "validation",
    );
  }
  const numbers = input.numbers.map((n, i) => assertE164(n, `numbers[${i}]`));
  const from_number = assertE164(input.from_number, "from_number");
  return prepareJson(creds, "POST", PATHS.createGroup, {
    numbers,
    from_number,
  });
}

export type ListMessagesFilters = {
  limit?: number;
  /** true = outbound only, false = inbound only, omit = both */
  is_outbound?: boolean;
  number?: string;
};

export function prepareListMessages(
  creds: Credentials,
  filters: ListMessagesFilters = {},
): PreparedRequest {
  const limit = Math.min(Math.max(filters.limit ?? 20, 1), 100);
  const q = new URLSearchParams();
  q.set("limit", String(limit));
  if (typeof filters.is_outbound === "boolean") {
    q.set("is_outbound", String(filters.is_outbound));
  }
  if (filters.number?.trim()) {
    q.set("number", assertE164(filters.number, "number"));
  }
  return prepareGet(creds, `${PATHS.messages}?${q.toString()}`);
}

export function prepareGetMessage(
  creds: Credentials,
  messageId: string,
): PreparedRequest {
  if (!messageId.trim()) {
    throw new SendBlueClientError("message id is required", "validation");
  }
  return prepareGet(creds, `${PATHS.messages}/${encodeURIComponent(messageId.trim())}`);
}

export function prepareListContacts(creds: Credentials): PreparedRequest {
  return prepareGet(creds, PATHS.contacts);
}

export function prepareAddContact(
  creds: Credentials,
  input: { number: string; first_name?: string; last_name?: string },
): PreparedRequest {
  const number = assertE164(input.number, "number");
  const body: Record<string, unknown> = { number };
  if (input.first_name?.trim()) body.first_name = input.first_name.trim();
  if (input.last_name?.trim()) body.last_name = input.last_name.trim();
  return prepareJson(creds, "POST", PATHS.contacts, body);
}

export function prepareEvaluateService(
  creds: Credentials,
  number: string,
): PreparedRequest {
  const n = assertE164(number, "number");
  const q = new URLSearchParams({ number: n });
  return prepareGet(creds, `${PATHS.evaluateService}?${q.toString()}`);
}

export function prepareListAccountWebhooks(creds: Credentials): PreparedRequest {
  return prepareGet(creds, PATHS.accountWebhooks);
}

/**
 * Append webhooks. Official body:
 * `{ webhooks: [url | {url, secret}], type?: "receive", globalSecret?: string }`
 */
export function prepareCreateAccountWebhook(
  creds: Credentials,
  input: {
    url: string;
    /** SendBlue webhook type (default receive). */
    type?: string;
    secret?: string;
    globalSecret?: string;
  },
): PreparedRequest {
  const url = input.url.trim();
  if (!url || !/^https:\/\//i.test(url)) {
    throw new SendBlueClientError(
      "webhook url must be absolute https",
      "validation",
    );
  }
  const secret = input.secret?.trim();
  const entry = secret ? { url, secret } : url;
  const body: Record<string, unknown> = {
    webhooks: [entry],
    type: input.type ?? "receive",
  };
  const globalSecret = input.globalSecret?.trim();
  if (globalSecret) body.globalSecret = globalSecret;
  return prepareJson(creds, "POST", PATHS.accountWebhooks, body);
}

/** @deprecated Prefer prepareCreateAccountWebhook — kept as alias for tests/docs. */
export const prepareAddAccountWebhook = prepareCreateAccountWebhook;

/**
 * Replace entire webhook config.
 * Body: `{ webhooks: { receive: [...], outbound: [...], globalSecret?: string } }`
 */
export function prepareReplaceAccountWebhooks(
  creds: Credentials,
  webhooks: Record<string, unknown>,
): PreparedRequest {
  if (typeof webhooks !== "object") {
    throw new SendBlueClientError(
      "webhooks object is required for replace",
      "validation",
    );
  }
  return prepareJson(creds, "PUT", PATHS.accountWebhooks, { webhooks });
}

/** @deprecated Use prepareReplaceAccountWebhooks with full map. */
export function prepareUpdateAccountWebhook(
  creds: Credentials,
  input: { receive?: string[]; secret?: string; globalSecret?: string },
): PreparedRequest {
  const webhooks: Record<string, unknown> = {};
  if (input.receive) webhooks.receive = input.receive;
  if (input.globalSecret ?? input.secret) {
    webhooks.globalSecret = input.globalSecret ?? input.secret;
  }
  return prepareReplaceAccountWebhooks(creds, webhooks);
}

/**
 * Delete specific webhook URLs.
 * Body: `{ webhooks: [url, ...], type?: "receive" }`
 */
export function prepareDeleteAccountWebhooks(
  creds: Credentials,
  input: { urls: string[]; type?: string },
): PreparedRequest {
  if (!Array.isArray(input.urls) || input.urls.length === 0) {
    throw new SendBlueClientError(
      "urls array is required for delete",
      "validation",
    );
  }
  for (const u of input.urls) {
    const url = u.trim();
    if (!url || !/^https:\/\//i.test(url)) {
      throw new SendBlueClientError(
        `delete webhook url must be absolute https: ${u}`,
        "validation",
      );
    }
  }
  return prepareJson(creds, "DELETE", PATHS.accountWebhooks, {
    webhooks: input.urls.map((u) => u.trim()),
    type: input.type ?? "receive",
  });
}

/** @deprecated Prefer prepareDeleteAccountWebhooks with urls. */
export function prepareDeleteAccountWebhook(
  creds: Credentials,
  url: string,
): PreparedRequest {
  return prepareDeleteAccountWebhooks(creds, { urls: [url] });
}

export function prepareGetStatus(
  creds: Credentials,
  messageHandle: string,
): PreparedRequest {
  if (!messageHandle.trim()) {
    throw new SendBlueClientError("message_handle is required", "validation");
  }
  const q = new URLSearchParams({ message_handle: messageHandle.trim() });
  return prepareGet(creds, `${PATHS.status}?${q.toString()}`);
}

// --- Response parsers ---

export type LinesResult = { numbers: string[] };

export function parseLinesResponse(json: unknown): LinesResult {
  if (!json || typeof json !== "object") {
    throw new SendBlueClientError("lines response not an object", "json");
  }
  const o = json as Record<string, unknown>;
  let raw: unknown = o.numbers ?? o.data ?? o.lines;
  if (Array.isArray(json)) raw = json;
  if (!Array.isArray(raw)) {
    throw new SendBlueClientError(
      "lines response missing numbers array",
      "json",
    );
  }
  const numbers: string[] = [];
  for (const item of raw) {
    if (typeof item === "string") numbers.push(item);
    else if (item && typeof item === "object") {
      const n =
        (item as Record<string, unknown>).number ??
        (item as Record<string, unknown>).phone_number ??
        (item as Record<string, unknown>).sendblue_number;
      if (typeof n === "string") numbers.push(n);
    }
  }
  return { numbers };
}

export type SendMessageResult = {
  status?: string;
  message_handle?: string;
  raw: unknown;
};

export function parseSendMessageResponse(json: unknown): SendMessageResult {
  if (!json || typeof json !== "object") {
    return { raw: json };
  }
  const o = json as Record<string, unknown>;
  return {
    status: typeof o.status === "string" ? o.status : undefined,
    message_handle:
      typeof o.message_handle === "string"
        ? o.message_handle
        : typeof o.handle === "string"
          ? o.handle
          : undefined,
    raw: json,
  };
}

export type MessageRow = {
  content?: string;
  is_outbound?: boolean;
  status?: string;
  message_handle?: string;
  from_number?: string;
  to_number?: string;
  number?: string;
  date_sent?: string;
  service?: string;
};

export function parseMessagesResponse(json: unknown): {
  status?: string;
  data: MessageRow[];
} {
  if (!json || typeof json !== "object") {
    throw new SendBlueClientError("messages response not an object", "json");
  }
  const o = json as Record<string, unknown>;
  const arr = Array.isArray(o.data) ? o.data : Array.isArray(json) ? json : [];
  const data: MessageRow[] = arr.map((item) => {
    if (!item || typeof item !== "object") return {};
    const m = item as Record<string, unknown>;
    return {
      content: typeof m.content === "string" ? m.content : undefined,
      is_outbound:
        typeof m.is_outbound === "boolean" ? m.is_outbound : undefined,
      status: typeof m.status === "string" ? m.status : undefined,
      message_handle:
        typeof m.message_handle === "string" ? m.message_handle : undefined,
      from_number: typeof m.from_number === "string" ? m.from_number : undefined,
      to_number: typeof m.to_number === "string" ? m.to_number : undefined,
      number: typeof m.number === "string" ? m.number : undefined,
      date_sent: typeof m.date_sent === "string" ? m.date_sent : undefined,
      service: typeof m.service === "string" ? m.service : undefined,
    };
  });
  return {
    status: typeof o.status === "string" ? o.status : undefined,
    data,
  };
}

// --- Live fetch client ---

export async function executePrepared(
  prep: PreparedRequest,
  fetchImpl: typeof fetch = fetch,
): Promise<unknown> {
  const res = await fetchImpl(prep.url, {
    method: prep.method,
    headers: prep.headers,
    body: prep.body,
  });
  const text = await res.text();
  let json: unknown = undefined;
  if (text) {
    try {
      json = JSON.parse(text);
    } catch {
      json = text;
    }
  }
  if (!res.ok) {
    throw new SendBlueClientError(
      `SendBlue API HTTP ${res.status}`,
      "api",
      res.status,
      text.slice(0, 2000),
    );
  }
  return json;
}

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
