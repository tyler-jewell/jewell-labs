import { describe, expect, it } from "vitest";
import {
  API_TEST_DEFS,
  buildTestParams,
  configJsonFromForm,
  formFromConfigJson,
  inboundWebhookPath,
  pluginConfigUrl,
} from "../src/ui/settings-model.js";
import { TOOL_NAMES } from "../src/tools.js";
import { PLUGIN_ID, SECRET_KEYS } from "../src/constants.js";

describe("settings-model", () => {
  it("round-trips form ↔ configJson for soft fields only", () => {
    const form = formFromConfigJson({
      apiKeyRef: "my-key",
      apiSecretRef: "my-secret",
      webhookSecretRef: "my-hook",
      fromNumber: "+16452067656",
      notifyNumber: "+15551112222",
      allowlist: ["+15551112222", "+15553334444"],
      emptyMeansDeny: true,
      inboundMode: "log_only",
      notifyOnIssueDone: false,
      notifyOnIssueCreated: true,
      notifyOnApprovalCreated: false,
      notifyOnAgentError: true,
    });
    expect(form.allowlistText).toContain("+15551112222");
    expect(form.inboundMode).toBe("log_only");
    expect(form.notifyOnIssueCreated).toBe(true);

    const json = configJsonFromForm(form);
    expect(json).toMatchObject({
      apiKeyRef: "my-key",
      apiSecretRef: "my-secret",
      fromNumber: "+16452067656",
      allowlist: ["+15551112222", "+15553334444"],
      inboundMode: "log_only",
      notifyOnIssueDone: false,
      notifyOnIssueCreated: true,
    });
    // Never persist plain API secrets or board UUIDs from the form model.
    expect(json).not.toHaveProperty("apiKey");
    expect(json).not.toHaveProperty("apiSecret");
    expect(json).not.toHaveProperty("defaultCompanyId");
    expect(json).not.toHaveProperty("defaultAssigneeAgentId");
  });

  it("omits vault refs when empty (host may 422 on secret refs)", () => {
    const json = configJsonFromForm(formFromConfigJson({}));
    expect(json).not.toHaveProperty("apiKeyRef");
    expect(json).not.toHaveProperty("apiSecretRef");
    expect(json).not.toHaveProperty("webhookSecretRef");
    expect(json.allowlist).toEqual([]);
    expect(json.emptyMeansDeny).toBe(true);
    expect(SECRET_KEYS.apiKey).toBe("sendblue-api-key");
    expect(SECRET_KEYS.apiSecret).toBe("sendblue-api-secret");
    expect(SECRET_KEYS.webhookSecret).toBe("sendblue-webhook-secret");
  });

  it("parses allowlist from CSV in form text", () => {
    const json = configJsonFromForm({
      ...formFromConfigJson({}),
      allowlistText: "+1a, +1b\n+1c",
    });
    expect(json.allowlist).toEqual(["+1a", "+1b", "+1c"]);
  });

  it("covers every agent tool with a Run test definition", () => {
    const covered = new Set(API_TEST_DEFS.map((d) => d.name));
    for (const name of TOOL_NAMES) {
      expect(covered.has(name)).toBe(true);
    }
    expect(API_TEST_DEFS).toHaveLength(TOOL_NAMES.length);
  });

  it("buildTestParams maps numbers CSV and limit", () => {
    expect(
      buildTestParams("send_group_message", {
        numbers: "+1a, +1b",
        content: "hi",
      }),
    ).toEqual({ numbers: ["+1a", "+1b"], content: "hi" });
    expect(buildTestParams("list_messages", { limit: "10" })).toEqual({
      limit: 10,
    });
  });

  it("plugin config + webhook paths use plugin id", () => {
    expect(pluginConfigUrl(null)).toBe(`/api/plugins/${PLUGIN_ID}/config`);
    expect(pluginConfigUrl("abc")).toBe(
      `/api/plugins/${PLUGIN_ID}/config?companyId=abc`,
    );
    expect(inboundWebhookPath()).toContain(PLUGIN_ID);
    expect(inboundWebhookPath()).toContain("/webhooks/inbound");
  });
});
