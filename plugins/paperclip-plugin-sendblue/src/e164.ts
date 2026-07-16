/**
 * Strict E.164 validation: leading +, country code 1–9, total 8–15 digits after +.
 * Rejects bare local numbers, parentheses, spaces.
 */
export function isValidE164(value: string): boolean {
  if (typeof value !== "string") return false;
  const s = value.trim();
  return /^\+[1-9]\d{7,14}$/.test(s);
}

export function normalizeE164(value: string): string {
  return value.trim();
}

export function assertE164(value: string, field = "number"): string {
  const n = normalizeE164(value);
  if (!isValidE164(n)) {
    throw new Error(`Invalid E.164 ${field}: "${value}" (expected +1… form, no spaces)`);
  }
  return n;
}
