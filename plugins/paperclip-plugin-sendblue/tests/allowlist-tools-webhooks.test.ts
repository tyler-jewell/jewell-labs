import { describe, it, expect } from "vitest";
import {
  checkAllowlist,
  assertAllowlisted,
  parseAllowlistInput,
} from "../src/allowlist.js";
import { isValidE164 } from "../src/e164.js";
import { buildToolRequest, TOOL_NAMES } from "../src/tools.js";
import {
  handleWebhookRequest,
  normalizeWebhookBody,
  verifyWebhookSecret,
  processInboundWebhook,
  executeToolForHost,
  maybeNotifyFromEvent,
  validateConfig,
  planInboundAction,
  formatIssueDone,
  formatApprovalRequested,
} from "../src/index.js";
import { createHmac } from "node:crypto";

const creds = { apiKey: "k", apiSecret: "s" };
const policy = {
  numbers: ["+15551112222", "+16452067656"],
  emptyMeansDeny: true,
};

describe("e164", () => {
  it("accepts valid", () => {
    expect(isValidE164("+15551234567")).toBe(true);
    expect(isValidE164("+447911123456")).toBe(true);
  });
  it("rejects invalid", () => {
    expect(isValidE164("5551234567")).toBe(false);
    expect(isValidE164("+1 555")).toBe(false);
    expect(isValidE164("")).toBe(false);
  });
});

describe("allowlist", () => {
  it("allows listed number", () => {
    const d = checkAllowlist("+15551112222", policy);
    expect(d.allowed).toBe(true);
  });
  it("denies unlisted", () => {
    const d = checkAllowlist("+19998887777", policy);
    expect(d.allowed).toBe(false);
    expect(() => assertAllowlisted("+19998887777", policy)).toThrow(
      /allowlist/,
    );
  });
  it("empty allowlist denies by default", () => {
    const d = checkAllowlist("+15551112222", {
      numbers: [],
      emptyMeansDeny: true,
    });
    expect(d.allowed).toBe(false);
  });
  it("empty allowlist can allow when emptyMeansDeny false", () => {
    const d = checkAllowlist("+15551112222", {
      numbers: [],
      emptyMeansDeny: false,
    });
    expect(d.allowed).toBe(true);
  });
  it("denies invalid E.164", () => {
    expect(checkAllowlist("555", policy).allowed).toBe(false);
  });
  it("parseAllowlistInput", () => {
    expect(parseAllowlistInput("+1a, +1b\n+1c")).toEqual(["+1a", "+1b", "+1c"]);
  });
});

describe("buildToolRequest allowlist", () => {
  const ctx = {
    creds,
    fromNumber: "+16452067656",
    allowlist: policy,
  };

  it("send_message allow builds POST to send-message", () => {
    const r = buildToolRequest(
      "send_message",
      { number: "+15551112222", content: "hello" },
      ctx,
    );
    expect(r.ok).toBe(true);
    expect(r.request?.url).toBe("https://api.sendblue.com/api/send-message");
    expect(r.request?.method).toBe("POST");
    expect(JSON.parse(r.request!.body!).content).toBe("hello");
  });

  it("send_message deny produces no request", () => {
    const r = buildToolRequest(
      "send_message",
      { number: "+19998887777", content: "nope" },
      ctx,
    );
    expect(r.ok).toBe(false);
    expect(r.request).toBeUndefined();
    expect(r.error).toMatch(/allowlist|denied/i);
  });

  it("list_lines always builds GET", () => {
    const r = buildToolRequest("list_lines", {}, ctx);
    expect(r.ok).toBe(true);
    expect(r.request?.url).toBe("https://api.sendblue.com/api/lines");
  });

  it("mark_read and get_status build correct paths", () => {
    const mr = buildToolRequest(
      "mark_read",
      { number: "+15551112222" },
      ctx,
    );
    expect(mr.ok).toBe(true);
    expect(mr.request?.url).toContain("/api/mark-read");
    const gs = buildToolRequest(
      "get_status",
      { message_handle: "mh-1" },
      ctx,
    );
    expect(gs.ok).toBe(true);
    expect(gs.request?.url).toContain("message_handle=mh-1");
  });

  it("create_account_webhook builds official body", () => {
    const r = buildToolRequest(
      "create_account_webhook",
      {
        url: "https://board.example/api/plugins/x/webhooks/inbound",
        secret: "s",
      },
      ctx,
    );
    expect(r.ok).toBe(true);
    const body = JSON.parse(r.request!.body!);
    expect(body.webhooks[0].url).toMatch(/^https:/);
    expect(body.type).toBe("receive");
  });

  it("exports all tool names", () => {
    expect(TOOL_NAMES.length).toBeGreaterThanOrEqual(12);
    expect(TOOL_NAMES).toContain("send_message");
    expect(TOOL_NAMES).toContain("list_account_webhooks");
  });
});

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

describe("planInboundAction", () => {
  const baseSecrets = {
    apiKey: "k",
    apiSecret: "s",
    allowlist: ["+15551112222"],
    emptyMeansDeny: true,
    notifyOnIssueDone: true,
    notifyOnIssueCreated: false,
    notifyOnApprovalCreated: true,
    notifyOnAgentError: true,
    inboundMode: "create_issue" as const,
    defaultCompanyId: "co-1",
    defaultAssigneeAgentId: "ag-1",
  };

  it("plans create_issue for allowlisted inbound", () => {
    const plan = planInboundAction(
      {
        kind: "inbound_message",
        from_number: "+15551112222",
        content: "Ship the plugin",
        is_outbound: false,
        raw: {},
      },
      baseSecrets,
    );
    expect(plan.action).toBe("create_issue");
    if (plan.action === "create_issue") {
      expect(plan.companyId).toBe("co-1");
      expect(plan.title).toMatch(/Ship the plugin/);
      expect(plan.requestWakeup).toBe(true);
    }
  });

  it("log_only when company missing", () => {
    const plan = planInboundAction(
      {
        kind: "inbound_message",
        from_number: "+15551112222",
        content: "hi",
        is_outbound: false,
        raw: {},
      },
      { ...baseSecrets, defaultCompanyId: undefined },
    );
    expect(plan.action).toBe("log_only");
  });

  it("never create_issue for missing from_number", () => {
    const plan = planInboundAction(
      {
        kind: "inbound_message",
        content: "anonymous",
        is_outbound: false,
        raw: {},
      },
      baseSecrets,
    );
    expect(plan.action).toBe("ignore");
    if (plan.action === "ignore") {
      expect(plan.reason).toMatch(/missing or not E\.164/i);
    }
  });

  it("never create_issue for invalid from_number", () => {
    const plan = planInboundAction(
      {
        kind: "inbound_message",
        from_number: "5551112222",
        content: "bad",
        is_outbound: false,
        raw: {},
      },
      baseSecrets,
    );
    expect(plan.action).toBe("ignore");
  });

  it("never create_issue for unallowlisted sender", () => {
    const plan = planInboundAction(
      {
        kind: "inbound_message",
        from_number: "+19998887777",
        content: "stranger",
        is_outbound: false,
        raw: {},
      },
      baseSecrets,
    );
    expect(plan.action).toBe("ignore");
    if (plan.action === "ignore") {
      expect(plan.reason).toMatch(/allowlist/i);
    }
  });

  it("never create_issue when empty allowlist + emptyMeansDeny", () => {
    const plan = planInboundAction(
      {
        kind: "inbound_message",
        from_number: "+15551112222",
        content: "hi",
        is_outbound: false,
        raw: {},
      },
      { ...baseSecrets, allowlist: [], emptyMeansDeny: true },
    );
    expect(plan.action).toBe("ignore");
  });
});

describe("config validation", () => {
  it("rejects missing secrets when required", () => {
    const v = validateConfig({}, { requireCredentials: true });
    expect(v.ok).toBe(false);
  });
  it("accepts secret refs without plain keys", () => {
    const v = validateConfig(
      {
        apiKeyRef: "sendblue-api-key",
        apiSecretRef: "sendblue-api-secret",
        fromNumber: "+16452067656",
        allowlist: ["+15551112222"],
      },
      { requireCredentials: true },
    );
    expect(v.ok).toBe(true);
  });
  it("rejects bad E.164 fromNumber", () => {
    const v = validateConfig(
      {
        apiKey: "a",
        apiSecret: "b",
        fromNumber: "555",
        allowlist: ["+15551112222"],
      },
      { requireCredentials: true },
    );
    expect(v.ok).toBe(false);
  });
  it("accepts valid config", () => {
    const v = validateConfig(
      {
        apiKey: "a",
        apiSecret: "b",
        fromNumber: "+16452067656",
        allowlist: ["+15551112222", "+16452067656"],
        notifyNumber: "+15551112222",
      },
      { requireCredentials: true },
    );
    expect(v.ok).toBe(true);
  });
  it("soft validate without credentials warns", () => {
    const v = validateConfig({}, { requireCredentials: false });
    expect(v.ok).toBe(true);
    if (v.ok) expect(v.warnings.length).toBeGreaterThan(0);
  });
});

describe("notify format", () => {
  it("clips issue done", () => {
    expect(formatIssueDone({ identifier: "JEW-1", title: "Ship" })).toMatch(
      /JEW-1/,
    );
    expect(formatApprovalRequested({ summary: "budget" })).toMatch(/budget/);
  });
});

describe("executeToolForHost with mock fetch", () => {
  it("does not call fetch when allowlist denies", async () => {
    let called = false;
    const fetchImpl = async () => {
      called = true;
      return new Response("{}");
    };
    const out = await executeToolForHost(
      "send_message",
      { number: "+19998887777", content: "x" },
      {
        apiKey: "k",
        apiSecret: "s",
        fromNumber: "+16452067656",
        allowlist: ["+15551112222"],
        emptyMeansDeny: true,
        notifyOnIssueDone: true,
        notifyOnIssueCreated: false,
        notifyOnApprovalCreated: true,
        notifyOnAgentError: true,
        inboundMode: "create_issue",
      },
      fetchImpl,
    );
    expect(out.ok).toBe(false);
    expect(called).toBe(false);
  });

  it("calls fetch with correct URL on allow", async () => {
    let url = "";
    let method = "";
    let body = "";
    const fetchImpl = async (input: RequestInfo | URL, init?: RequestInit) => {
      url = String(input);
      method = init?.method ?? "GET";
      body = String(init?.body ?? "");
      return new Response(
        JSON.stringify({ status: "QUEUED", message_handle: "h1" }),
        { status: 200 },
      );
    };
    const out = await executeToolForHost(
      "send_message",
      { number: "+15551112222", content: "hello" },
      {
        apiKey: "k",
        apiSecret: "s",
        fromNumber: "+16452067656",
        allowlist: ["+15551112222"],
        emptyMeansDeny: true,
        notifyOnIssueDone: true,
        notifyOnIssueCreated: false,
        notifyOnApprovalCreated: true,
        notifyOnAgentError: true,
        inboundMode: "create_issue",
      },
      fetchImpl,
    );
    expect(out.ok).toBe(true);
    expect(url).toBe("https://api.sendblue.com/api/send-message");
    expect(method).toBe("POST");
    expect(JSON.parse(body).content).toBe("hello");
    expect((out.result as { message_handle?: string }).message_handle).toBe(
      "h1",
    );
  });
});

describe("maybeNotifyFromEvent", () => {
  it("sends on issue done when configured", async () => {
    let body = "";
    const fetchImpl = async (_u: RequestInfo | URL, init?: RequestInit) => {
      body = String(init?.body ?? "");
      return new Response(JSON.stringify({ status: "QUEUED" }), {
        status: 200,
      });
    };
    const r = await maybeNotifyFromEvent(
      {
        type: "issue.updated",
        payload: { status: "done", identifier: "JEW-1", title: "Ship" },
      },
      {
        apiKey: "k",
        apiSecret: "s",
        fromNumber: "+16452067656",
        allowlist: ["+15551112222"],
        emptyMeansDeny: true,
        notifyOnIssueDone: true,
        notifyOnIssueCreated: false,
        notifyOnApprovalCreated: false,
        notifyOnAgentError: false,
        notifyNumber: "+15551112222",
        inboundMode: "create_issue",
      },
      fetchImpl,
    );
    expect(r.sent).toBe(true);
    expect(JSON.parse(body).content).toMatch(/JEW-1/);
  });

  it("skips when notify flags off", async () => {
    let called = false;
    const fetchImpl = async () => {
      called = true;
      return new Response("{}");
    };
    const r = await maybeNotifyFromEvent(
      {
        type: "issue.updated",
        payload: { status: "done", identifier: "JEW-1" },
      },
      {
        apiKey: "k",
        apiSecret: "s",
        fromNumber: "+16452067656",
        allowlist: ["+15551112222"],
        emptyMeansDeny: true,
        notifyOnIssueDone: false,
        notifyOnIssueCreated: false,
        notifyOnApprovalCreated: false,
        notifyOnAgentError: false,
        notifyNumber: "+15551112222",
        inboundMode: "create_issue",
      },
      fetchImpl,
    );
    expect(r.sent).toBe(false);
    expect(called).toBe(false);
  });
});
