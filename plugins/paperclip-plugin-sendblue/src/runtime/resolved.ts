/** Runtime secrets shape derived from validated plugin config. */

import {
  validateConfig,
  type InboundMode,
  type SendBluePluginConfig,
} from "../config.js";

export type ResolvedSecrets = {
  apiKey: string;
  apiSecret: string;
  fromNumber?: string;
  webhookSecret?: string;
  allowlist: string[];
  emptyMeansDeny: boolean;
  notifyOnIssueDone: boolean;
  notifyOnIssueCreated: boolean;
  notifyOnApprovalCreated: boolean;
  notifyOnAgentError: boolean;
  notifyNumber?: string;
  inboundMode: InboundMode;
};

/** Board routing is resolved live — never stored in plugin config. */
export type BoardRouting = {
  companyId?: string;
  projectId?: string;
  assigneeAgentId?: string;
};

export function resolvedFromValidated(
  config: SendBluePluginConfig,
): ResolvedSecrets {
  return {
    apiKey: config.apiKey ?? "",
    apiSecret: config.apiSecret ?? "",
    fromNumber: config.fromNumber,
    webhookSecret: config.webhookSecret,
    allowlist: config.allowlist,
    emptyMeansDeny: config.emptyMeansDeny,
    notifyOnIssueDone: config.notifyOnIssueDone,
    notifyOnIssueCreated: config.notifyOnIssueCreated,
    notifyOnApprovalCreated: config.notifyOnApprovalCreated,
    notifyOnAgentError: config.notifyOnAgentError,
    notifyNumber: config.notifyNumber,
    inboundMode: config.inboundMode,
  };
}

export function resolveRuntimeConfig(
  raw: Record<string, unknown>,
): ReturnType<typeof validateConfig> {
  return validateConfig(raw, { requireCredentials: true });
}
