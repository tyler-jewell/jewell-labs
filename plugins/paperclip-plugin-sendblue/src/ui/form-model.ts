/**
 * Settings form state ↔ configJson (pure, no React).
 * Soft fields only — vault secrets use canonical company secret keys;
 * company/agent UUIDs are never plugin config.
 */

import { parseAllowlistInput } from "../allowlist.js";
import type { InboundMode } from "../config.js";
import { PLUGIN_ID, SECRET_KEYS } from "../constants.js";

export type SettingsFormState = {
  apiKeyRef: string;
  apiSecretRef: string;
  webhookSecretRef: string;
  fromNumber: string;
  notifyNumber: string;
  allowlistText: string;
  emptyMeansDeny: boolean;
  inboundMode: InboundMode;
  notifyOnIssueDone: boolean;
  notifyOnIssueCreated: boolean;
  notifyOnApprovalCreated: boolean;
  notifyOnAgentError: boolean;
};

export const DEFAULT_SETTINGS_FORM: SettingsFormState = {
  apiKeyRef: "",
  apiSecretRef: "",
  webhookSecretRef: "",
  fromNumber: "",
  notifyNumber: "",
  allowlistText: "",
  emptyMeansDeny: true,
  inboundMode: "create_issue",
  notifyOnIssueDone: false,
  notifyOnIssueCreated: false,
  notifyOnApprovalCreated: true,
  notifyOnAgentError: true,
};

export const SUGGESTED_SECRET_KEYS = SECRET_KEYS;

function asStr(v: unknown): string {
  return typeof v === "string" ? v : "";
}

function asBool(v: unknown, d: boolean): boolean {
  if (typeof v === "boolean") return v;
  if (v === "true" || v === "1") return true;
  if (v === "false" || v === "0") return false;
  return d;
}

function asInboundMode(v: unknown): InboundMode {
  if (v === "log_only" || v === "ignore" || v === "create_issue") return v;
  return "create_issue";
}

export function formFromConfigJson(
  raw: Record<string, unknown> | null | undefined,
): SettingsFormState {
  const r = raw ?? {};
  const allowlist = Array.isArray(r.allowlist)
    ? r.allowlist.filter((n): n is string => typeof n === "string")
    : parseAllowlistInput(r.allowlist);
  return {
    apiKeyRef: asStr(r.apiKeyRef),
    apiSecretRef: asStr(r.apiSecretRef),
    webhookSecretRef: asStr(r.webhookSecretRef),
    fromNumber: asStr(r.fromNumber),
    notifyNumber: asStr(r.notifyNumber),
    allowlistText: allowlist.join("\n"),
    emptyMeansDeny: asBool(r.emptyMeansDeny, true),
    inboundMode: asInboundMode(r.inboundMode),
    notifyOnIssueDone: asBool(r.notifyOnIssueDone, false),
    notifyOnIssueCreated: asBool(r.notifyOnIssueCreated, false),
    notifyOnApprovalCreated: asBool(r.notifyOnApprovalCreated, true),
    notifyOnAgentError: asBool(r.notifyOnAgentError, true),
  };
}

export function configJsonFromForm(
  form: SettingsFormState,
): Record<string, unknown> {
  const allowlist = parseAllowlistInput(form.allowlistText);
  const out: Record<string, unknown> = {
    allowlist,
    emptyMeansDeny: form.emptyMeansDeny,
    inboundMode: form.inboundMode,
    notifyOnIssueDone: form.notifyOnIssueDone,
    notifyOnIssueCreated: form.notifyOnIssueCreated,
    notifyOnApprovalCreated: form.notifyOnApprovalCreated,
    notifyOnAgentError: form.notifyOnAgentError,
  };
  // Optional explicit refs; empty omitted (host may 422 secret refs).
  // Worker always also tries canonical SECRET_KEYS via vault.
  if (form.apiKeyRef.trim()) out.apiKeyRef = form.apiKeyRef.trim();
  if (form.apiSecretRef.trim()) out.apiSecretRef = form.apiSecretRef.trim();
  if (form.webhookSecretRef.trim()) {
    out.webhookSecretRef = form.webhookSecretRef.trim();
  }
  if (form.fromNumber.trim()) out.fromNumber = form.fromNumber.trim();
  if (form.notifyNumber.trim()) out.notifyNumber = form.notifyNumber.trim();
  return out;
}

export function pluginConfigUrl(companyId: string | null): string {
  const base = `/api/plugins/${PLUGIN_ID}/config`;
  if (!companyId) return base;
  return `${base}?companyId=${encodeURIComponent(companyId)}`;
}

export function inboundWebhookPath(): string {
  return `/api/plugins/${PLUGIN_ID}/webhooks/inbound`;
}
