import { describe, it, expect } from "vitest";
import {
  prepareListLines,
  parseLinesResponse,
  parseSendMessageResponse,
  parseMessagesResponse,
  executePrepared,
  SendBlueClient,
} from "../src/sendblue-api.js";

const creds = { apiKey: "key-test", apiSecret: "secret-test" };

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
