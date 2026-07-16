/** Normalized inbound webhook payload shapes. */

export type InboundMessage = {
  kind: "inbound_message";
  from_number?: string;
  to_number?: string;
  content?: string;
  media_url?: string;
  message_handle?: string;
  service?: string;
  is_outbound: false;
  raw: unknown;
};

export type OutboundStatus = {
  kind: "outbound_status";
  message_handle?: string;
  status?: string;
  number?: string;
  error_message?: string;
  is_outbound: true;
  raw: unknown;
};

export type CallLogEvent = {
  kind: "call_log";
  call_id?: string;
  from_number?: string;
  to_number?: string;
  status?: string;
  duration?: number;
  raw: unknown;
};

export type NormalizedWebhook =
  | InboundMessage
  | OutboundStatus
  | CallLogEvent
  | { kind: "unknown"; raw: unknown };

export type WebhookHandlerResult = {
  status: number;
  body: {
    ok: boolean;
    kind?: string;
    from_number?: string;
    content?: string;
    message_handle?: string;
    status?: string;
    error?: string;
  };
  normalized: NormalizedWebhook;
};
