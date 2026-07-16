/** Load and resolve plugin config + credentials for the worker. */

import type { PluginContext } from "@paperclipai/plugin-sdk";
import { validateConfig } from "../config.js";
import {
  ENV_API_KEY,
  ENV_API_SECRET,
  ENV_FROM_NUMBER,
  SECRET_KEYS,
} from "../constants.js";
import {
  readFirstExistingFile,
  SENDBLUE_API_KEY_PATHS,
  SENDBLUE_API_SECRET_PATHS,
  SENDBLUE_WEBHOOK_SECRET_PATHS,
} from "../ops-paths.js";
import {
  resolvedFromValidated,
  type ResolvedSecrets,
} from "../worker-logic.js";

export async function readRawConfig(
  ctx: PluginContext,
): Promise<Record<string, unknown>> {
  try {
    return await ctx.config.get();
  } catch {
    return {};
  }
}

function envFallback(names: readonly string[]): string | undefined {
  for (const n of names) {
    const v = process.env[n]?.trim();
    if (v) return v;
  }
  return undefined;
}

export type SecretSourceKind =
  | "plain"
  | "env"
  | "file"
  | "vault_ref"
  | "vault_canonical";

export type ResolveSecretSources = {
  plain?: string;
  ref?: string;
  canonicalKey?: string;
  envNames?: readonly string[];
  filePaths?: readonly string[];
  /** Injected vault lookup (defaults to ctx.secrets when using resolveSecret). */
  resolveVault?: (key: string) => Promise<string | undefined>;
};

export type ResolveSecretResult = {
  value?: string;
  source?: SecretSourceKind;
};

/**
 * First-class credential resolution for worker paths (webhook + tools).
 * Order: plain → env → ops files → vault ref → vault canonical.
 * All steps are supported production sources. Vault misses are silent
 * (expected when the host secrets API is not available in webhook scope).
 */
export async function resolveSecretValue(
  sources: ResolveSecretSources,
): Promise<ResolveSecretResult> {
  if (sources.plain?.trim()) {
    return { value: sources.plain.trim(), source: "plain" };
  }
  if (sources.envNames) {
    const fromEnv = envFallback(sources.envNames);
    if (fromEnv) return { value: fromEnv, source: "env" };
  }
  if (sources.filePaths) {
    const fromFile = readFirstExistingFile(sources.filePaths);
    if (fromFile) return { value: fromFile, source: "file" };
  }
  if (sources.resolveVault) {
    if (sources.ref?.trim()) {
      const fromRef = await sources.resolveVault(sources.ref.trim());
      if (fromRef?.trim()) {
        return { value: fromRef.trim(), source: "vault_ref" };
      }
    }
    if (sources.canonicalKey) {
      const fromCanon = await sources.resolveVault(sources.canonicalKey);
      if (fromCanon?.trim()) {
        return { value: fromCanon.trim(), source: "vault_canonical" };
      }
    }
  }
  return {};
}

/**
 * Resolve a secret via first-class sources (plain / env / file / vault).
 */
export async function resolveSecret(
  ctx: PluginContext,
  ref: string | undefined,
  plain: string | undefined,
  extra?: {
    canonicalKey?: string;
    envNames?: readonly string[];
    filePaths?: readonly string[];
  },
): Promise<string | undefined> {
  const resolveVault = async (key: string): Promise<string | undefined> => {
    try {
      const v = await ctx.secrets.resolve(key);
      if (typeof v === "string" && v.trim()) return v.trim();
    } catch {
      /* vault unavailable or key missing — expected under some scopes */
    }
    return undefined;
  };

  const { value } = await resolveSecretValue({
    plain,
    ref,
    canonicalKey: extra?.canonicalKey,
    envNames: extra?.envNames,
    filePaths: extra?.filePaths,
    resolveVault,
  });
  return value;
}

/**
 * Merge credentials into validated config for runtime use.
 */
export async function loadResolvedSecrets(
  ctx: PluginContext,
): Promise<
  | { ok: true; secrets: ResolvedSecrets; warnings: string[] }
  | { ok: false; errors: string[]; warnings: string[] }
> {
  const raw = await readRawConfig(ctx);
  const soft = validateConfig(raw, { requireCredentials: false });
  if (!soft.ok) {
    return { ok: false, errors: soft.errors, warnings: soft.warnings };
  }

  const apiKey = await resolveSecret(
    ctx,
    soft.config.apiKeyRef,
    soft.config.apiKey,
    {
      canonicalKey: SECRET_KEYS.apiKey,
      envNames: ENV_API_KEY,
      filePaths: SENDBLUE_API_KEY_PATHS,
    },
  );
  const apiSecret = await resolveSecret(
    ctx,
    soft.config.apiSecretRef,
    soft.config.apiSecret,
    {
      canonicalKey: SECRET_KEYS.apiSecret,
      envNames: ENV_API_SECRET,
      filePaths: SENDBLUE_API_SECRET_PATHS,
    },
  );
  const webhookSecret = await resolveSecret(
    ctx,
    soft.config.webhookSecretRef,
    soft.config.webhookSecret,
    {
      canonicalKey: SECRET_KEYS.webhookSecret,
      envNames: ["SENDBLUE_WEBHOOK_SECRET"],
      filePaths: SENDBLUE_WEBHOOK_SECRET_PATHS,
    },
  );

  const fromNumber =
    soft.config.fromNumber ??
    envFallback(ENV_FROM_NUMBER) ??
    (await resolveSecret(ctx, undefined, undefined, {
      canonicalKey: SECRET_KEYS.fromNumber,
    }));

  const merged: Record<string, unknown> = {
    ...raw,
    apiKey,
    apiSecret,
    webhookSecret,
    fromNumber,
    allowlist: soft.config.allowlist,
    emptyMeansDeny: soft.config.emptyMeansDeny,
    notifyNumber: soft.config.notifyNumber,
    notifyOnIssueDone: soft.config.notifyOnIssueDone,
    notifyOnIssueCreated: soft.config.notifyOnIssueCreated,
    notifyOnApprovalCreated: soft.config.notifyOnApprovalCreated,
    notifyOnAgentError: soft.config.notifyOnAgentError,
    inboundMode: soft.config.inboundMode,
  };

  const hard = validateConfig(merged, { requireCredentials: true });
  if (!hard.ok) {
    return { ok: false, errors: hard.errors, warnings: hard.warnings };
  }
  return {
    ok: true,
    secrets: resolvedFromValidated(hard.config),
    warnings: hard.warnings,
  };
}

export function fetchFromCtx(ctx: PluginContext): typeof fetch {
  return ((input: string | URL, init?: RequestInit) =>
    ctx.http.fetch(String(input), init)) as typeof fetch;
}
