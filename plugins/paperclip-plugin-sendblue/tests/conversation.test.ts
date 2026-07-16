import { describe, expect, it } from "vitest";
import {
  commentBodyFromPayload,
  conversationFromInbound,
  formatReplySms,
  isAgentAuthoredComment,
  issueIdFromEvent,
  parseConversation,
  sendReplyTo,
  sendTypingTo,
} from "../src/runtime/conversation.js";
import type { ResolvedSecrets } from "../src/runtime/resolved.js";

const secrets: ResolvedSecrets = {
  apiKey: "k",
  apiSecret: "s",
  fromNumber: "+16452067656",
  allowlist: ["+12178551977"],
  emptyMeansDeny: true,
  notifyOnIssueDone: true,
  notifyOnIssueCreated: false,
  notifyOnApprovalCreated: true,
  notifyOnAgentError: true,
  inboundMode: "create_issue",
};

describe("conversation mapping", () => {
  it("round-trips conversation record", () => {
    const rec = conversationFromInbound({
      fromNumber: "+12178551977",
      companyId: "company-1",
      fromLine: "+16452067656",
      messageHandle: "h1",
    });
    const parsed = parseConversation(rec);
    expect(parsed?.fromNumber).toBe("+12178551977");
    expect(parsed?.companyId).toBe("company-1");
    expect(parseConversation(null)).toBeNull();
    expect(parseConversation({ fromNumber: "+1" })).toBeNull();
  });

  it("extracts issue id from payloads", () => {
    expect(
      issueIdFromEvent({
        payload: { issueId: "iss-1" },
      }),
    ).toBe("iss-1");
    expect(
      issueIdFromEvent({ entityType: "issue", entityId: "iss-2", payload: {} }),
    ).toBe("iss-2");
    expect(
      issueIdFromEvent({
        entityType: "run",
        entityId: "run-1",
        payload: { issue: { id: "iss-nested" } },
      }),
    ).toBe("iss-nested");
    expect(
      issueIdFromEvent({
        entityType: "comment",
        entityId: "cmt-1",
        payload: { issueId: "iss-from-comment" },
      }),
    ).toBe("iss-from-comment");
    expect(
      issueIdFromEvent({
        entityType: "run",
        entityId: "run-only",
        payload: {},
      }),
    ).toBeUndefined();
  });

  it("detects agent comments", () => {
    expect(isAgentAuthoredComment({ authorType: "agent" })).toBe(true);
    expect(isAgentAuthoredComment({ authorType: "user" })).toBe(false);
    expect(isAgentAuthoredComment({}, "agent")).toBe(true);
    expect(isAgentAuthoredComment({ agentId: "a1" })).toBe(true);
    expect(
      isAgentAuthoredComment({
        authorType: "agent",
        presentation: { kind: "system_notice" },
      }),
    ).toBe(false);
  });

  it("reads comment body including host bodySnippet", () => {
    expect(commentBodyFromPayload({ body: "  hi  " })).toBe("hi");
    expect(commentBodyFromPayload({ bodySnippet: "snippet only" })).toBe(
      "snippet only",
    );
    expect(commentBodyFromPayload({})).toBeUndefined();
  });

  it("formats agent markdown for SMS", () => {
    const long = "x".repeat(2000);
    expect(formatReplySms(long).length).toBeLessThanOrEqual(1500);
    expect(formatReplySms("short")).toBe("short");
    expect(formatReplySms("## Hello\n**world** and `code`")).toBe(
      "Hello\nworld and code",
    );
  });
});

describe("sendTypingTo / sendReplyTo", () => {
  it("sends mark-read then typing with start + duration", async () => {
    const calls: { url: string; body: string }[] = [];
    const fetchImpl = (async (url: string | URL, init?: RequestInit) => {
      calls.push({ url: String(url), body: String(init?.body ?? "") });
      return new Response(JSON.stringify({ status: "QUEUED" }), {
        status: 200,
      });
    }) as typeof fetch;
    const r = await sendTypingTo(secrets, "+12178551977", fetchImpl, {
      fromLine: "+16452067656",
      maxDurationMs: 180_000,
    });
    expect(r.ok).toBe(true);
    expect(r.status).toBe("QUEUED");
    expect(calls.some((c) => c.url.includes("mark-read"))).toBe(true);
    const typing = calls.find((c) => c.url.includes("typing"));
    expect(typing).toBeTruthy();
    const body = JSON.parse(typing!.body);
    expect(body.state).toBe("start");
    expect(body.max_duration_ms).toBe(180_000);
    expect(body.from_number).toBe("+16452067656");
  });

  it("fails typing when SendBlue returns ERROR status on HTTP 200", async () => {
    const fetchImpl = (async () =>
      new Response(
        JSON.stringify({
          status: "ERROR",
          error_message: "No route mapping found for number",
        }),
        { status: 200 },
      )) as typeof fetch;
    const r = await sendTypingTo(secrets, "+12178551977", fetchImpl, {
      markReadFirst: false,
    });
    expect(r.ok).toBe(false);
    expect(r.error).toMatch(/route mapping|ERROR/i);
  });

  it("sends reply via mock fetch (stops typing first)", async () => {
    const bodies: string[] = [];
    const urls: string[] = [];
    const fetchImpl = (async (url: string | URL, init?: RequestInit) => {
      urls.push(String(url));
      bodies.push(String(init?.body ?? ""));
      return new Response(JSON.stringify({ status: "QUEUED" }), {
        status: 200,
      });
    }) as typeof fetch;
    const r = await sendReplyTo(
      secrets,
      "+12178551977",
      "Agent says hello",
      fetchImpl,
      { fromLine: "+16452067656" },
    );
    expect(r.ok).toBe(true);
    expect(urls.some((u) => u.includes("typing"))).toBe(true);
    expect(bodies.some((b) => b.includes("Agent says hello"))).toBe(true);
  });

  it("denies unallowlisted recipient", async () => {
    const fetchImpl = (async () =>
      new Response(JSON.stringify({ status: "QUEUED" }), {
        status: 200,
      })) as typeof fetch;
    const r = await sendReplyTo(secrets, "+19998887777", "nope", fetchImpl);
    expect(r.ok).toBe(false);
    expect(r.error).toMatch(/allowlist/i);
  });

  it("fails without credentials", async () => {
    const r = await sendTypingTo(
      { ...secrets, apiKey: "", apiSecret: "" },
      "+12178551977",
      fetch,
    );
    expect(r.ok).toBe(false);
  });
});

describe("typing helpers", () => {
  it("clamps duration to SendBlue range", async () => {
    const { clampTypingDurationMs, assertSendBlueOutboundOk } = await import(
      "../src/runtime/typing.js"
    );
    expect(clampTypingDurationMs(undefined)).toBe(120_000);
    expect(clampTypingDurationMs(0)).toBe(1);
    expect(clampTypingDurationMs(999_999)).toBe(300_000);
    expect(assertSendBlueOutboundOk({ status: "QUEUED" })).toBe("QUEUED");
    expect(() =>
      assertSendBlueOutboundOk({
        status: "ERROR",
        message: "bad",
      }),
    ).toThrow(/bad/);
  });
});
