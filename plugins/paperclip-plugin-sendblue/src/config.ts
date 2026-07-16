import { isValidE164, normalizeE164 } from "./e164.js";
import { parseAllowlistInput, type AllowlistPolicy } from "./allowlist.js";
import type { Credentials } from "./sendblue-api.js";

export type InboundMode = "create_issue" | "log_only" | "ignore";

export type SendBluePluginConfig = {
  /** Resolved API key (from secret_ref or plain for tests). */
  apiKey?: string;
  apiSecret?: string;
  /** Vault secret ref keys (Paperclip resolves via ctx.secrets). */
  apiKeyRef?: string;
  apiSecretRef?: string;
  webhookSecretRef?: string;
  fromNumber?: string;
  webhookSecret?: string;
  /** E.164 allowlist for outbound recipients and inbound senders. */
  allowlist: string[];
  emptyMeansDeny: boolean;
  notifyOnIssueDone: boolean;
  notifyOnIssueCreated: boolean;
  notifyOnApprovalCreated: boolean;
  notifyOnAgentError: boolean;
  /** Default board notify recipient (must be on allowlist). */
  notifyNumber?: string;
  /** How inbound SMS/iMessage is handled on the board. */
  inboundMode: InboundMode;
  /** Company UUID for inbound issue creation / comments. */
  defaultCompanyId?: string;
  /** Optional project UUID for created issues. */
  defaultProjectId?: string;
  /** Optional agent UUID to assign + wake on inbound. */
  defaultAssigneeAgentId?: string;
};

export type ConfigValidation =
  | { ok: true; config: SendBluePluginConfig; warnings: string[] }
  | { ok: false; errors: string[]; warnings: string[] };

function asBool(v: unknown, d: boolean): boolean {
  if (typeof v === "boolean") return v;
  if (v === "true" || v === "1") return true;
  if (v === "false" || v === "0") return false;
  return d;
}

function asStr(v: unknown): string | undefined {
  return typeof v === "string" && v.trim() ? v.trim() : undefined;
}

function asInboundMode(v: unknown): InboundMode {
  if (v === "log_only" || v === "ignore" || v === "create_issue") return v;
  return "create_issue";
}

/**
 * Parse + validate plugin instance config.
 * Missing credentials are errors for “ready to send”; probe mode can soft-warn.
 *
 * Production configs should set apiKeyRef/apiSecretRef (vault). Plain
 * apiKey/apiSecret are accepted for local contract tests.
 */
export function validateConfig(
  raw: Record<string, unknown> | null | undefined,
  opts?: { requireCredentials?: boolean },
): ConfigValidation {
  const requireCredentials = opts?.requireCredentials !== false;
  const r = raw ?? {};
  const errors: string[] = [];
  const warnings: string[] = [];

  const apiKey = asStr(r.apiKey) ?? asStr(r.api_key);
  const apiSecret = asStr(r.apiSecret) ?? asStr(r.api_secret);
  const apiKeyRef = asStr(r.apiKeyRef) ?? asStr(r.api_key_ref);
  const apiSecretRef = asStr(r.apiSecretRef) ?? asStr(r.api_secret_ref);
  const webhookSecretRef =
    asStr(r.webhookSecretRef) ?? asStr(r.webhook_secret_ref);
  const fromNumberRaw = asStr(r.fromNumber) ?? asStr(r.from_number);
  const webhookSecret = asStr(r.webhookSecret) ?? asStr(r.webhook_secret);
  const notifyNumberRaw = asStr(r.notifyNumber) ?? asStr(r.notify_number);
  const defaultCompanyId =
    asStr(r.defaultCompanyId) ?? asStr(r.default_company_id);
  const defaultProjectId =
    asStr(r.defaultProjectId) ?? asStr(r.default_project_id);
  const defaultAssigneeAgentId =
    asStr(r.defaultAssigneeAgentId) ?? asStr(r.default_assignee_agent_id);

  const hasPlainCreds = Boolean(apiKey && apiSecret);
  const hasSecretRefs = Boolean(apiKeyRef && apiSecretRef);
  const hasCreds = hasPlainCreds || hasSecretRefs;
  if (requireCredentials && !hasCreds) {
    errors.push(
      "credentials required: set apiKey+apiSecret or apiKeyRef+apiSecretRef (vault)",
    );
  }
  if (!requireCredentials && !hasCreds) {
    warnings.push(
      "no SendBlue credentials configured yet (apiKey/apiSecret or secret refs)",
    );
  }

  let fromNumber: string | undefined;
  if (fromNumberRaw) {
    if (!isValidE164(fromNumberRaw)) {
      errors.push(`fromNumber must be E.164, got "${fromNumberRaw}"`);
    } else {
      fromNumber = normalizeE164(fromNumberRaw);
    }
  } else if (requireCredentials) {
    warnings.push("fromNumber not set; each send must supply from_number");
  }

  let notifyNumber: string | undefined;
  if (notifyNumberRaw) {
    if (!isValidE164(notifyNumberRaw)) {
      errors.push(`notifyNumber must be E.164, got "${notifyNumberRaw}"`);
    } else {
      notifyNumber = normalizeE164(notifyNumberRaw);
    }
  }

  const allowlist = parseAllowlistInput(r.allowlist ?? r.allowlistNumbers);
  for (const n of allowlist) {
    if (!isValidE164(n)) {
      errors.push(`allowlist entry is not E.164: "${n}"`);
    }
  }

  const emptyMeansDeny = asBool(r.emptyMeansDeny, true);
  if (allowlist.length === 0 && emptyMeansDeny) {
    warnings.push(
      "allowlist is empty — all SMS denied until numbers are added",
    );
  }

  if (
    notifyNumber &&
    allowlist.length > 0 &&
    !allowlist.includes(notifyNumber)
  ) {
    errors.push("notifyNumber must be present on allowlist");
  }

  const inboundMode = asInboundMode(r.inboundMode ?? r.inbound_mode);
  if (inboundMode === "create_issue" && !defaultCompanyId) {
    warnings.push(
      "inboundMode=create_issue but defaultCompanyId unset — inbound issues cannot be created until configured",
    );
  }

  if (errors.length) {
    return { ok: false, errors, warnings };
  }

  return {
    ok: true,
    warnings,
    config: {
      apiKey,
      apiSecret,
      apiKeyRef,
      apiSecretRef,
      webhookSecretRef,
      fromNumber,
      webhookSecret,
      allowlist,
      emptyMeansDeny,
      notifyOnIssueDone: asBool(r.notifyOnIssueDone, true),
      notifyOnIssueCreated: asBool(r.notifyOnIssueCreated, false),
      notifyOnApprovalCreated: asBool(r.notifyOnApprovalCreated, true),
      notifyOnAgentError: asBool(r.notifyOnAgentError, true),
      notifyNumber,
      inboundMode,
      defaultCompanyId,
      defaultProjectId,
      defaultAssigneeAgentId,
    },
  };
}

export function credentialsFromConfig(config: SendBluePluginConfig): Credentials {
  if (!config.apiKey || !config.apiSecret) {
    throw new Error(
      "credentials missing on config (resolve secret refs before calling)",
    );
  }
  return { apiKey: config.apiKey, apiSecret: config.apiSecret };
}

export function allowlistPolicyFromConfig(
  config: SendBluePluginConfig,
): AllowlistPolicy {
  return {
    numbers: config.allowlist,
    emptyMeansDeny: config.emptyMeansDeny,
  };
}
