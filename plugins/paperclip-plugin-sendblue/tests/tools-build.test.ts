/**
 * Lean coverage: every agent tool builds a valid request with minimal params.
 * No network — pure prepare/allowlist only.
 */
import { describe, it, expect } from "vitest";
import {
  buildToolRequest,
  TOOL_META,
  TOOL_NAMES,
  type ToolName,
} from "../src/tools.js";

const creds = { apiKey: "k", apiSecret: "s" };
const ctx = {
  creds,
  fromNumber: "+16452067656",
  allowlist: {
    numbers: ["+15551112222", "+16452067656"],
    emptyMeansDeny: true,
  },
};

/** Minimal valid params per tool (must stay in sync with TOOL_NAMES). */
const SAMPLE_PARAMS: Record<ToolName, Record<string, unknown>> = {
  send_message: { number: "+15551112222", content: "hello" },
  send_group_message: {
    numbers: ["+15551112222", "+16452067656"],
    content: "hi",
  },
  send_reaction: {
    number: "+15551112222",
    message_handle: "h1",
    reaction: "love",
  },
  send_typing_indicator: {
    number: "+15551112222",
    state: "start",
    max_duration_ms: 60_000,
  },
  mark_read: { number: "+15551112222" },
  create_group: { numbers: ["+15551112222", "+16452067656"] },
  list_lines: {},
  list_messages: { limit: 10 },
  get_status: { message_handle: "mh-1" },
  list_contacts: {},
  add_contact: { number: "+15551112222", first_name: "A" },
  evaluate_service: { number: "+15551112222" },
  list_account_webhooks: {},
  create_account_webhook: {
    url: "https://board.example/api/plugins/x/webhooks/inbound",
    secret: "s",
  },
  delete_account_webhook: {
    url: "https://board.example/api/plugins/x/webhooks/inbound",
  },
};

describe("buildToolRequest — all tools", () => {
  it("has meta + sample params for every TOOL_NAMES entry", () => {
    expect(TOOL_NAMES.length).toBe(15);
    for (const name of TOOL_NAMES) {
      expect(TOOL_META[name].displayName).toBeTruthy();
      expect(TOOL_META[name].description.length).toBeGreaterThan(10);
      expect(SAMPLE_PARAMS[name]).toBeDefined();
    }
  });

  it.each(TOOL_NAMES)("%s builds a valid prepared request", (name) => {
    const r = buildToolRequest(name, SAMPLE_PARAMS[name], ctx);
    expect(r.ok, r.error).toBe(true);
    expect(r.request?.url).toMatch(/^https:\/\/api\.sendblue\.com\//);
    expect(r.request?.method).toMatch(/^(GET|POST|DELETE|PUT|PATCH)$/);
  });

  it("send_message denylists unknown numbers", () => {
    const r = buildToolRequest(
      "send_message",
      { number: "+19998887777", content: "nope" },
      ctx,
    );
    expect(r.ok).toBe(false);
    expect(r.error).toMatch(/allowlist|denied/i);
  });

  it("send_typing_indicator carries state + duration", () => {
    const r = buildToolRequest(
      "send_typing_indicator",
      SAMPLE_PARAMS.send_typing_indicator,
      ctx,
    );
    expect(r.ok).toBe(true);
    const body = JSON.parse(r.request!.body!);
    expect(body.state).toBe("start");
    expect(body.max_duration_ms).toBe(60_000);
  });
});
