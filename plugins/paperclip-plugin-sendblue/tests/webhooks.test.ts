import { describe, it, expect } from "vitest";
import {
  handleWebhookRequest,
  normalizeWebhookBody,
  verifyWebhookSecret,
  processInboundWebhook,
} from "../src/index.js";
import { createHmac } from "node:crypto";

describe("webhooks", () => {
  it("normalizes official inbound payload shape", () => {
    const n = normalizeWebhookBody({
      accountEmail: "a@b.com",
      content: "hi board",
      is_outbound: false,
      status: "RECEIVED",
      message_handle: "mh1",
      from_number: "+15551112222",
      number: "+15551112222",
      to_number: "+16452067656",
      sendblue_number: "+16452067656",
      service: "iMessage",
    });
    expect(n.kind).toBe("inbound_message");
    if (n.kind === "inbound_message") {
      expect(n.content).toBe("hi board");
      expect(n.from_number).toBe("+15551112222");
      expect(n.service).toBe("iMessage");
    }
  });

  it("normalizes outbound status", () => {
    const n = normalizeWebhookBody({
      status: "DELIVERED",
      message_handle: "mh2",
      is_outbound: true,
    });
    expect(n.kind).toBe("outbound_status");
  });

  it("normalizes call_log", () => {
    const n = normalizeWebhookBody({
      event_type: "call_log",
      call_id: "cs_1",
      from_number: "+1555",
      to_number: "+1666",
      status: "COMPLETED",
      duration: 12,
    });
    expect(n.kind).toBe("call_log");
  });

  it("verifyWebhookSecret skip when no secret", () => {
    expect(verifyWebhookSecret({ secret: "" }).ok).toBe(true);
  });

  it("verifyWebhookSecret plain header", () => {
    expect(
      verifyWebhookSecret({
        secret: "sekrit",
        headers: { "x-sendblue-secret": "sekrit" },
      }).ok,
    ).toBe(true);
    expect(
      verifyWebhookSecret({
        secret: "sekrit",
        headers: { "x-webhook-secret": "sekrit" },
      }).ok,
    ).toBe(true);
    expect(
      verifyWebhookSecret({
        secret: "sekrit",
        headers: { "x-sendblue-secret": "wrong" },
      }).ok,
    ).toBe(false);
  });

  it("verifyWebhookSecret hmac", () => {
    const rawBody = '{"content":"x"}';
    const secret = "sekrit";
    const hex = createHmac("sha256", secret).update(rawBody).digest("hex");
    expect(
      verifyWebhookSecret({
        secret,
        rawBody,
        headers: { "x-sendblue-signature": hex },
      }).ok,
    ).toBe(true);
  });

  it("handleWebhookRequest returns 401 on bad secret", () => {
    const r = handleWebhookRequest({
      secret: "sekrit",
      headers: {},
      parsedBody: { content: "hi", is_outbound: false },
    });
    expect(r.status).toBe(401);
    expect(r.body.ok).toBe(false);
  });

  it("handleWebhookRequest 200 + structured body", () => {
    const r = handleWebhookRequest({
      secret: undefined,
      parsedBody: {
        content: "ping",
        from_number: "+15551112222",
        is_outbound: false,
      },
    });
    expect(r.status).toBe(200);
    expect(r.body.ok).toBe(true);
    expect(r.body.kind).toBe("inbound_message");
  });

  it("processInboundWebhook allowlist deny", () => {
    const r = processInboundWebhook({
      secrets: {
        webhookSecret: undefined,
        allowlist: ["+16452067656"],
        emptyMeansDeny: true,
      },
      parsedBody: {
        content: "hi",
        from_number: "+15551112222",
        is_outbound: false,
      },
    });
    expect(r.status).toBe(403);
  });

  it("processInboundWebhook allowlist allow", () => {
    const r = processInboundWebhook({
      secrets: {
        webhookSecret: undefined,
        allowlist: ["+15551112222"],
        emptyMeansDeny: true,
      },
      parsedBody: {
        content: "hi",
        from_number: "+15551112222",
        is_outbound: false,
      },
    });
    expect(r.status).toBe(200);
    expect(r.body.ok).toBe(true);
  });

  it("processInboundWebhook denies missing from_number (emptyMeansDeny)", () => {
    const r = processInboundWebhook({
      secrets: {
        webhookSecret: undefined,
        allowlist: [],
        emptyMeansDeny: true,
      },
      parsedBody: {
        content: "hi board",
        is_outbound: false,
      },
    });
    expect(r.status).toBe(403);
    expect(r.body.ok).toBe(false);
    expect(r.body.error).toMatch(/missing or not E\.164/i);
  });

  it("processInboundWebhook denies missing from_number even with non-empty allowlist", () => {
    const r = processInboundWebhook({
      secrets: {
        webhookSecret: undefined,
        allowlist: ["+15551112222"],
        emptyMeansDeny: true,
      },
      parsedBody: {
        content: "spoofed",
        is_outbound: false,
      },
    });
    expect(r.status).toBe(403);
    expect(r.body.ok).toBe(false);
  });

  it("processInboundWebhook denies invalid non-E.164 from_number", () => {
    const r = processInboundWebhook({
      secrets: {
        webhookSecret: undefined,
        allowlist: ["+15551112222"],
        emptyMeansDeny: true,
      },
      parsedBody: {
        content: "hi",
        from_number: "5551112222",
        is_outbound: false,
      },
    });
    expect(r.status).toBe(403);
    expect(r.body.ok).toBe(false);
    expect(r.body.error).toMatch(/not E\.164|allowlist denied/i);
  });

  it("processInboundWebhook denies empty allowlist + valid sender (emptyMeansDeny)", () => {
    const r = processInboundWebhook({
      secrets: {
        webhookSecret: undefined,
        allowlist: [],
        emptyMeansDeny: true,
      },
      parsedBody: {
        content: "hi",
        from_number: "+15551112222",
        is_outbound: false,
      },
    });
    expect(r.status).toBe(403);
    expect(r.body.ok).toBe(false);
  });
});

