/**
 * Frontend SSE parser tests — would have caught starts_with regression.
 * Run: node --test tests/js/sse.test.mjs
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import {
  parseSseDataLine,
  assertNoPythonStartsWith,
} from "../../static/js/sse.js";

const root = join(dirname(fileURLToPath(import.meta.url)), "../..");

test("parseSseDataLine: delta", () => {
  const ev = parseSseDataLine('data: {"delta":"hello"}');
  assert.equal(ev.kind, "delta");
  assert.equal(ev.delta, "hello");
});

test("parseSseDataLine: done", () => {
  assert.equal(parseSseDataLine("data: [DONE]").kind, "done");
});

test("parseSseDataLine: skip non-data", () => {
  assert.equal(parseSseDataLine(": keepalive").kind, "skip");
  assert.equal(parseSseDataLine("").kind, "skip");
});

test("parseSseDataLine: error event", () => {
  const ev = parseSseDataLine('data: {"error":"boom"}');
  assert.equal(ev.kind, "error");
  assert.equal(ev.error, "boom");
});

test("parseSseDataLine: tool_call", () => {
  const ev = parseSseDataLine(
    'data: {"tool_call":{"name":"list_tools","arguments":{}}}'
  );
  assert.equal(ev.kind, "tool_call");
  assert.equal(ev.tool_call.name, "list_tools");
});

test("parseSseDataLine: tool_result", () => {
  const ev = parseSseDataLine(
    'data: {"tool_result":{"name":"list_tools","ok":true,"result":{}}}'
  );
  assert.equal(ev.kind, "tool_result");
  assert.equal(ev.tool_result.ok, true);
});

test("regression: no Python starts_with in static JS sources", () => {
  const files = [
    "static/app.js",
    "static/js/sse.js",
    "static/js/sessions.js",
  ];
  for (const f of files) {
    const src = readFileSync(join(root, f), "utf8");
    assertNoPythonStartsWith(src);
  }
  // sse.js must use startsWith for data: prefix
  const sse = readFileSync(join(root, "static/js/sse.js"), "utf8");
  assert.match(sse, /\.startsWith\(/);
});

test("regression: starts_with call would throw path via assert", () => {
  const badSample = "if (!trimmed." + "starts_with" + "('data:')) {}";
  assert.throws(() => assertNoPythonStartsWith(badSample), /prefix API|startsWith/i);
});
