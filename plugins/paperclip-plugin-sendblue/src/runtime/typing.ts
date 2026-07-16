/**
 * SendBlue typing indicators (iMessage … bubble).
 *
 * Docs: POST /api/send-typing-indicator
 * - iMessage only (not SMS/RCS)
 * - Needs an existing conversation / route mapping
 * - Supports state start|stop and max_duration_ms (1–300000)
 */

import { assertAllowlisted } from "../allowlist.js";
import {
  allowlistPolicyFromConfig,
  credentialsFromConfig,
  type SendBluePluginConfig,
  validateConfig,
} from "../config.js";
import {
  executePrepared,
  prepareMarkRead,
  prepareSendTypingIndicator,
  type Credentials,
} from "../sendblue-api.js";
import type { ResolvedSecrets } from "./resolved.js";

export type TypingPulseResult = {
  ok: boolean;
  error?: string;
  /** Raw SendBlue status when present (e.g. QUEUED). */
  status?: string;
  /** Whether mark-read was attempted. */
  markRead?: boolean;
};

export type TypingPulseOptions = {
  /** Prefer the line that received inbound (conversation.fromLine). */
  fromLine?: string;
  /** Visible duration 1–300000 ms (SendBlue). Default 120s for agent work. */
  maxDurationMs?: number;
  state?: "start" | "stop";
  /** Best-effort mark-read before typing (helps route/chat freshness). */
  markReadFirst?: boolean;
};

const DEFAULT_AGENT_TYPING_MS = 120_000;
const MAX_TYPING_MS = 300_000;

export function clampTypingDurationMs(ms: number | undefined): number {
  const n = typeof ms === "number" && Number.isFinite(ms) ? Math.floor(ms) : DEFAULT_AGENT_TYPING_MS;
  if (n < 1) return 1;
  if (n > MAX_TYPING_MS) return MAX_TYPING_MS;
  return n;
}

/** Interpret SendBlue typing/send JSON; throw on ERROR status even if HTTP 200. */
export function assertSendBlueOutboundOk(
  json: unknown,
  label = "SendBlue API",
): string | undefined {
  if (json == null || json === "") return undefined;
  if (typeof json !== "object") return undefined;
  const o = json as Record<string, unknown>;
  const statusRaw = o.status ?? o.Status;
  const status =
    typeof statusRaw === "string" ? statusRaw.trim().toUpperCase() : "";
  if (status === "ERROR" || status === "FAILED") {
    const msg =
      (typeof o.error_message === "string" && o.error_message) ||
      (typeof o.message === "string" && o.message) ||
      (typeof o.error === "string" && o.error) ||
      `${label} returned status=${status}`;
    throw new Error(msg);
  }
  return status || undefined;
}

function credentialsReady(
  secrets: ResolvedSecrets,
):
  | { ok: true; creds: Credentials; config: SendBluePluginConfig }
  | { ok: false; error: string } {
  const v = validateConfig(
    {
      apiKey: secrets.apiKey,
      apiSecret: secrets.apiSecret,
      fromNumber: secrets.fromNumber,
      allowlist: secrets.allowlist,
      emptyMeansDeny: secrets.emptyMeansDeny,
    },
    { requireCredentials: true },
  );
  if (!v.ok) return { ok: false, error: v.errors.join("; ") };
  try {
    return {
      ok: true,
      creds: credentialsFromConfig(v.config),
      config: v.config,
    };
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) };
  }
}

/**
 * Show (or stop) the iMessage typing bubble for an allowlisted recipient.
 */
export async function sendTypingPulse(
  secrets: ResolvedSecrets,
  toNumber: string,
  fetchImpl: typeof fetch,
  options: TypingPulseOptions = {},
): Promise<TypingPulseResult> {
  const prep = credentialsReady(secrets);
  if (!prep.ok) return prep;

  const state = options.state ?? "start";
  try {
    const policy = allowlistPolicyFromConfig(prep.config);
    const number = assertAllowlisted(toNumber, policy);
    const fromRaw = options.fromLine?.trim() ?? prep.config.fromNumber ?? "";
    const from_number = fromRaw.trim();
    if (!from_number) {
      return {
        ok: false,
        error: "fromNumber not configured (need line for typing)",
      };
    }

    let markRead = false;
    if (options.markReadFirst !== false && state === "start") {
      try {
        const mr = await executePrepared(
          prepareMarkRead(prep.creds, { number, from_number }),
          fetchImpl,
        );
        assertSendBlueOutboundOk(mr, "mark-read");
        markRead = true;
      } catch {
        /* mark-read is best-effort */
      }
    }

    const bodyOpts =
      state === "stop"
        ? { number, from_number, state: "stop" as const }
        : {
            number,
            from_number,
            state: "start" as const,
            max_duration_ms: clampTypingDurationMs(options.maxDurationMs),
          };

    const raw = await executePrepared(
      prepareSendTypingIndicator(prep.creds, bodyOpts),
      fetchImpl,
    );
    const status = assertSendBlueOutboundOk(raw, "typing-indicator");
    return { ok: true, status, markRead };
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) };
  }
}
