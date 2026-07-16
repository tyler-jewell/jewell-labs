import { describe, it, expect } from "vitest";
import {
  validateConfig,
  formatIssueDone,
  formatApprovalRequested,
  maybeNotifyFromEvent,
  executeToolForHost,
} from "../src/index.js";

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
        notifyOnIssueDone: false,
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

  it("defaults notifyOnIssueDone to off", () => {
    const v = validateConfig(
      {
        apiKey: "a",
        apiSecret: "b",
        fromNumber: "+16452067656",
        allowlist: ["+15551112222"],
      },
      { requireCredentials: true },
    );
    expect(v.ok).toBe(true);
    if (v.ok) expect(v.config.notifyOnIssueDone).toBe(false);
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

  it("does not SMS on issue done when notifyOnIssueDone is false (default)", async () => {
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
