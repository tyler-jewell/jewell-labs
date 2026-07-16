/** Shared-secret / HMAC verification for inbound webhooks. */

import { createHmac, timingSafeEqual } from "node:crypto";

/**
 * When secret is empty/undefined, verification is skipped (ok=true).
 */
export function verifyWebhookSecret(input: {
  secret?: string | null;
  headers?: Record<string, string | string[] | undefined>;
  rawBody?: string;
}): { ok: true } | { ok: false; reason: string } {
  const secret = input.secret?.trim();
  if (!secret) return { ok: true };

  const headers: Record<string, string> = {};
  for (const [k, v] of Object.entries(input.headers ?? {})) {
    if (v == null) continue;
    const first = Array.isArray(v) ? v[0] : v;
    if (typeof first === "string") headers[k.toLowerCase()] = first;
  }

  const plain =
    headers["x-sendblue-secret"] ??
    headers["x-webhook-secret"] ??
    headers["x-sendblue-token"] ??
    headers["sb-signing-secret"];
  if (plain) {
    try {
      const a = Buffer.from(plain);
      const b = Buffer.from(secret);
      if (a.length === b.length && timingSafeEqual(a, b)) return { ok: true };
    } catch {
      /* fall through */
    }
    return { ok: false, reason: "secret header mismatch" };
  }

  const sig =
    headers["x-sendblue-signature"] ??
    headers["x-hub-signature-256"] ??
    headers["x-signature"];
  if (sig && input.rawBody != null) {
    const hex = createHmac("sha256", secret).update(input.rawBody).digest("hex");
    const provided = sig.replace(/^sha256=/i, "").trim();
    try {
      const a = Buffer.from(hex);
      const b = Buffer.from(provided);
      if (a.length === b.length && timingSafeEqual(a, b)) return { ok: true };
    } catch {
      /* fall through */
    }
    return { ok: false, reason: "hmac signature mismatch" };
  }

  return {
    ok: false,
    reason: "webhook secret configured but no secret/signature header present",
  };
}
