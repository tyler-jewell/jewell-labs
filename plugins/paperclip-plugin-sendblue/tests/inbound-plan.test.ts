import { describe, it, expect } from "vitest";
import { planInboundAction } from "../src/index.js";

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
  };
  const routing = {
    companyId: "co-1",
    assigneeAgentId: "ag-1",
  };

  it("plans create_issue for allowlisted inbound with board routing", () => {
    const plan = planInboundAction(
      {
        kind: "inbound_message",
        from_number: "+15551112222",
        content: "Ship the plugin",
        is_outbound: false,
        raw: {},
      },
      baseSecrets,
      routing,
    );
    expect(plan.action).toBe("create_issue");
    if (plan.action === "create_issue") {
      expect(plan.companyId).toBe("co-1");
      expect(plan.title).toMatch(/Ship the plugin/);
      expect(plan.requestWakeup).toBe(true);
    }
  });

  it("log_only when board routing has no company", () => {
    const plan = planInboundAction(
      {
        kind: "inbound_message",
        from_number: "+15551112222",
        content: "hi",
        is_outbound: false,
        raw: {},
      },
      baseSecrets,
      {},
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
      routing,
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
      routing,
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
      routing,
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
      routing,
    );
    expect(plan.action).toBe("ignore");
  });
});
