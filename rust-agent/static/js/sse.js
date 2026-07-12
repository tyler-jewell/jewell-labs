/**
 * Parse one SSE line from /api/chat/stream.
 * Pure functions — unit tested in tests/js/sse.test.mjs.
 */

/** @param {string} line */
export function parseSseDataLine(line) {
  if (typeof line !== "string") return { kind: "skip" };
  const trimmed = line.trim();
  if (typeof trimmed.startsWith !== "function") {
    throw new Error("String.startsWith missing");
  }
  if (!trimmed.startsWith("data:")) return { kind: "skip" };
  const data = trimmed.slice(5).trim();
  if (data === "[DONE]") return { kind: "done" };
  try {
    const obj = JSON.parse(data);
    if (obj.error) return { kind: "error", error: String(obj.error) };
    if (obj.tool_call) return { kind: "tool_call", tool_call: obj.tool_call };
    if (obj.tool_result) return { kind: "tool_result", tool_result: obj.tool_result };
    if (obj.delta != null) return { kind: "delta", delta: String(obj.delta) };
    return { kind: "other", value: obj };
  } catch (err) {
    if (err instanceof SyntaxError) return { kind: "skip" };
    throw err;
  }
}

/** Fail CI if source uses Python-style snake-case string prefix API */
export function assertNoPythonStartsWith(source) {
  // Build the forbidden token without embedding the exact call form as a substring
  // that naive CI greps for in this file itself.
  const bad = "." + "starts_with" + "(";
  if (source.includes(bad)) {
    throw new Error("forbidden Python-style string prefix API in JS; use startsWith");
  }
}
