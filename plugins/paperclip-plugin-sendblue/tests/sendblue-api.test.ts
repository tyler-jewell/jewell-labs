import { describe, it, expect, beforeEach, afterEach } from "vitest";
import {
  credentialsFromEnv,
  prepareListLines,
  prepareListMessages,
  prepareSendMessage,
  prepareSendGroupMessage,
  prepareSendReaction,
  prepareSendTypingIndicator,
  prepareMarkRead,
  prepareCreateGroup,
  prepareListContacts,
  prepareAddContact,
  prepareEvaluateService,
  prepareListAccountWebhooks,
  prepareCreateAccountWebhook,
  prepareReplaceAccountWebhooks,
  prepareDeleteAccountWebhooks,
  prepareGetStatus,
  parseLinesResponse,
  parseSendMessageResponse,
  parseMessagesResponse,
  HEADER_API_KEY,
  HEADER_API_SECRET,
  SENDBLUE_API_BASE,
  PATHS,
  executePrepared,
  SendBlueClient,
} from "../src/sendblue-api.js";
import {
  HEADER_API_KEY as HK,
  HEADER_API_SECRET as HS,
  SENDBLUE_API_BASE as BASE,
} from "../src/constants.js";

const creds = { apiKey: "key-test", apiSecret: "secret-test" };

describe("credentialsFromEnv", () => {
  const keys = [
    "SENDBLUE_API_KEY",
    "SENDBLUE_API_SECRET",
    "SENDBLUE_API_API_KEY",
    "SENDBLUE_API_API_SECRET",
  ];
  beforeEach(() => {
    for (const k of keys) {
      // eslint-disable-next-line @typescript-eslint/no-dynamic-delete -- test env reset
      delete process.env[k];
    }
  });
  afterEach(() => {
    for (const k of keys) {
      // eslint-disable-next-line @typescript-eslint/no-dynamic-delete -- test env reset
      delete process.env[k];
    }
  });

  it("errors when missing", () => {
    expect(() => credentialsFromEnv()).toThrow(/missing Sendblue credentials/);
  });

  it("accepts short env names", () => {
    process.env.SENDBLUE_API_KEY = "a";
    process.env.SENDBLUE_API_SECRET = "b";
    expect(credentialsFromEnv()).toEqual({ apiKey: "a", apiSecret: "b" });
  });

  it("accepts MCP-style env names", () => {
    process.env.SENDBLUE_API_API_KEY = "mk";
    process.env.SENDBLUE_API_API_SECRET = "ms";
    expect(credentialsFromEnv()).toEqual({ apiKey: "mk", apiSecret: "ms" });
  });

  it("prefers short names over MCP aliases", () => {
    process.env.SENDBLUE_API_KEY = "short";
    process.env.SENDBLUE_API_SECRET = "short-s";
    process.env.SENDBLUE_API_API_KEY = "long";
    process.env.SENDBLUE_API_API_SECRET = "long-s";
    expect(credentialsFromEnv()).toEqual({
      apiKey: "short",
      apiSecret: "short-s",
    });
  });
});

describe("request builders", () => {
  it("prepareListLines uses official base and auth headers", () => {
    const prep = prepareListLines(creds);
    expect(prep.method).toBe("GET");
    expect(prep.url).toBe(`${SENDBLUE_API_BASE}${PATHS.lines}`);
    expect(prep.url).toBe("https://api.sendblue.com/api/lines");
    expect(prep.headers[HEADER_API_KEY]).toBe("key-test");
    expect(prep.headers[HEADER_API_SECRET]).toBe("secret-test");
    expect(prep.headers[HK]).toBe("key-test");
    expect(prep.headers[HS]).toBe("secret-test");
    expect(prep.body).toBeUndefined();
  });

  it("prepareListMessages encodes limit and filters", () => {
    const prep = prepareListMessages(creds, {
      limit: 3,
      is_outbound: false,
      number: "+15551112222",
    });
    expect(prep.url.startsWith(`${BASE}/api/v2/messages?`)).toBe(true);
    const u = new URL(prep.url);
    expect(u.searchParams.get("limit")).toBe("3");
    expect(u.searchParams.get("is_outbound")).toBe("false");
    expect(u.searchParams.get("number")).toBe("+15551112222");
  });

  it("prepareSendMessage shapes body and headers", () => {
    const prep = prepareSendMessage(creds, {
      number: "+15551112222",
      from_number: "+16452067656",
      content: "hi",
      media_url: "https://example.com/a.jpg",
      status_callback: "https://example.com/cb",
    });
    expect(prep.method).toBe("POST");
    expect(prep.url).toBe("https://api.sendblue.com/api/send-message");
    expect(prep.headers["Content-Type"]).toBe("application/json");
    const body = JSON.parse(prep.body!);
    expect(body).toEqual({
      number: "+15551112222",
      from_number: "+16452067656",
      content: "hi",
      media_url: "https://example.com/a.jpg",
      status_callback: "https://example.com/cb",
    });
  });

  it("prepareSendMessage rejects invalid E.164", () => {
    expect(() =>
      prepareSendMessage(creds, {
        number: "5551112222",
        from_number: "+16452067656",
        content: "x",
      }),
    ).toThrow(/E\.164/);
  });

  it("prepareSendGroupMessage requires 2+ numbers", () => {
    expect(() =>
      prepareSendGroupMessage(creds, {
        numbers: ["+15551112222"],
        from_number: "+16452067656",
        content: "hi",
      }),
    ).toThrow(/at least 2/);
  });

  it("prepareSendGroupMessage builds body", () => {
    const prep = prepareSendGroupMessage(creds, {
      numbers: ["+15551112222", "+15553334444"],
      from_number: "+16452067656",
      content: "group hi",
    });
    expect(prep.url).toBe("https://api.sendblue.com/api/send-group-message");
    expect(JSON.parse(prep.body!).numbers).toHaveLength(2);
  });

  it("prepareSendReaction", () => {
    const prep = prepareSendReaction(creds, {
      number: "+15551112222",
      from_number: "+16452067656",
      message_handle: "h1",
      reaction: "love",
    });
    expect(prep.url).toBe("https://api.sendblue.com/api/send-reaction");
    expect(JSON.parse(prep.body!).reaction).toBe("love");
  });

  it("prepareSendTypingIndicator", () => {
    const prep = prepareSendTypingIndicator(creds, {
      number: "+15551112222",
      from_number: "+16452067656",
    });
    expect(prep.url).toBe(
      "https://api.sendblue.com/api/send-typing-indicator",
    );
  });

  it("prepareMarkRead and prepareCreateGroup", () => {
    const mr = prepareMarkRead(creds, {
      number: "+15551112222",
      from_number: "+16452067656",
    });
    expect(mr.url).toBe("https://api.sendblue.com/api/mark-read");
    const cg = prepareCreateGroup(creds, {
      numbers: ["+15551112222", "+15553334444"],
      from_number: "+16452067656",
    });
    expect(cg.url).toBe("https://api.sendblue.com/api/create-group");
  });

  it("prepareListContacts and prepareAddContact", () => {
    expect(prepareListContacts(creds).url).toBe(
      "https://api.sendblue.com/api/v2/contacts",
    );
    const add = prepareAddContact(creds, {
      number: "+15551112222",
      first_name: "A",
    });
    expect(add.method).toBe("POST");
    expect(JSON.parse(add.body!).first_name).toBe("A");
  });

  it("prepareEvaluateService", () => {
    const prep = prepareEvaluateService(creds, "+15551112222");
    expect(prep.url).toContain("/api/evaluate-service?");
    expect(prep.url).toContain("number=%2B15551112222");
  });

  it("account webhooks match official body shapes", () => {
    expect(prepareListAccountWebhooks(creds).url).toBe(
      "https://api.sendblue.com/api/account/webhooks",
    );
    const create = prepareCreateAccountWebhook(creds, {
      url: "https://paperclip.example/hooks/inbound",
      type: "receive",
      secret: "whsec",
    });
    expect(create.method).toBe("POST");
    const createBody = JSON.parse(create.body!);
    expect(createBody.type).toBe("receive");
    expect(createBody.webhooks).toEqual([
      { url: "https://paperclip.example/hooks/inbound", secret: "whsec" },
    ]);

    const replace = prepareReplaceAccountWebhooks(creds, {
      receive: ["https://paperclip.example/hooks/inbound"],
      globalSecret: "gsec",
    });
    expect(replace.method).toBe("PUT");
    expect(JSON.parse(replace.body!).webhooks.globalSecret).toBe("gsec");

    const del = prepareDeleteAccountWebhooks(creds, {
      urls: ["https://paperclip.example/hooks/inbound"],
      type: "receive",
    });
    expect(del.method).toBe("DELETE");
    expect(JSON.parse(del.body!).webhooks).toEqual([
      "https://paperclip.example/hooks/inbound",
    ]);
  });

  it("prepareCreateAccountWebhook rejects non-https", () => {
    expect(() =>
      prepareCreateAccountWebhook(creds, {
        url: "http://insecure.example/hook",
      }),
    ).toThrow(/https/);
  });

  it("prepareGetStatus", () => {
    const prep = prepareGetStatus(creds, "mh-abc");
    expect(prep.url).toContain("/api/status?");
    expect(prep.url).toContain("message_handle=mh-abc");
  });
});

describe("parsers", () => {
  it("parseLinesResponse numbers array", () => {
    expect(parseLinesResponse({ numbers: ["+1a", "+1b"] }).numbers).toEqual([
      "+1a",
      "+1b",
    ]);
  });

  it("parseLinesResponse object items", () => {
    expect(
      parseLinesResponse({
        data: [{ number: "+1555" }, { phone_number: "+1666" }],
      }).numbers,
    ).toEqual(["+1555", "+1666"]);
  });

  it("parseSendMessageResponse", () => {
    const r = parseSendMessageResponse({
      status: "QUEUED",
      message_handle: "h1",
    });
    expect(r.status).toBe("QUEUED");
    expect(r.message_handle).toBe("h1");
  });

  it("parseMessagesResponse", () => {
    const r = parseMessagesResponse({
      status: "OK",
      data: [{ content: "hi", is_outbound: false, message_handle: "m1" }],
    });
    expect(r.data[0].content).toBe("hi");
    expect(r.data[0].is_outbound).toBe(false);
  });
});

describe("executePrepared + client with mock fetch", () => {
  it("throws on non-2xx", async () => {
    const fetchImpl = async () =>
      new Response(JSON.stringify({ status: "ERROR" }), { status: 401 });
    await expect(
      executePrepared(prepareListLines(creds), fetchImpl as typeof fetch),
    ).rejects.toThrow(/HTTP 401/);
  });

  it("SendBlueClient.listLines uses mock", async () => {
    const fetchImpl = async () =>
      new Response(JSON.stringify({ numbers: ["+1645"] }), { status: 200 });
    const client = new SendBlueClient(creds, fetchImpl);
    const r = await client.listLines();
    expect(r.numbers).toEqual(["+1645"]);
  });
});
