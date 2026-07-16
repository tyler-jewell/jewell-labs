import { isValidE164, normalizeE164 } from "./e164.js";

export type AllowlistPolicy = {
  /** When empty and emptyMeansDeny is true, all numbers are denied. */
  numbers: string[];
  /** Default true: empty allowlist denies everyone (safe for SMS). */
  emptyMeansDeny?: boolean;
};

export type AllowlistDecision =
  | { allowed: true; number: string }
  | { allowed: false; number: string; reason: string };

function normalizeList(numbers: string[]): string[] {
  const out: string[] = [];
  const seen = new Set<string>();
  for (const raw of numbers) {
    if (typeof raw !== "string") continue;
    const n = normalizeE164(raw);
    if (!n || seen.has(n)) continue;
    seen.add(n);
    out.push(n);
  }
  return out;
}

/**
 * Check whether `candidate` may send/receive under the policy.
 * Invalid E.164 is always denied.
 */
export function checkAllowlist(
  candidate: string,
  policy: AllowlistPolicy,
): AllowlistDecision {
  const number = normalizeE164(candidate);
  if (!isValidE164(number)) {
    return {
      allowed: false,
      number,
      reason: `invalid E.164: "${candidate}"`,
    };
  }
  const list = normalizeList(policy.numbers);
  const emptyMeansDeny = policy.emptyMeansDeny !== false;
  if (list.length === 0) {
    if (emptyMeansDeny) {
      return {
        allowed: false,
        number,
        reason: "allowlist is empty (emptyMeansDeny)",
      };
    }
    return { allowed: true, number };
  }
  if (!list.includes(number)) {
    return {
      allowed: false,
      number,
      reason: "number not on allowlist",
    };
  }
  return { allowed: true, number };
}

export function assertAllowlisted(
  candidate: string,
  policy: AllowlistPolicy,
): string {
  const d = checkAllowlist(candidate, policy);
  if (!d.allowed) {
    throw new Error(`SendBlue allowlist denied: ${d.reason}`);
  }
  return d.number;
}

/** Parse allowlist from config: array of strings or comma/newline-separated string. */
export function parseAllowlistInput(raw: unknown): string[] {
  if (raw == null) return [];
  if (Array.isArray(raw)) {
    return raw.filter((x): x is string => typeof x === "string").map((s) => s.trim());
  }
  if (typeof raw === "string") {
    return raw
      .split(/[\s,]+/)
      .map((s) => s.trim())
      .filter(Boolean);
  }
  return [];
}
