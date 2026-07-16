/** Safe string coercion for unknown tool/event payloads. */
export function asString(value: unknown, fallback = ""): string {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  if (value == null) return fallback;
  return fallback;
}
