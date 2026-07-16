/** Shared SendBlue HTTP types and error class. */

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

export type SendMessageInput = {
  number: string;
  from_number: string;
  content?: string;
  media_url?: string;
  status_callback?: string;
  send_style?: string;
};

export type SendGroupMessageInput = {
  numbers: string[];
  from_number: string;
  content?: string;
  media_url?: string;
  status_callback?: string;
  group_id?: string;
};

export type SendReactionInput = {
  number: string;
  from_number: string;
  message_handle: string;
  reaction: string;
};

export type ListMessagesFilters = {
  limit?: number;
  /** true = outbound only, false = inbound only, omit = both */
  is_outbound?: boolean;
  number?: string;
};

export type LinesResult = { numbers: string[] };

export type SendMessageResult = {
  status?: string;
  message_handle?: string;
  raw: unknown;
};

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
