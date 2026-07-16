import { describe, it, expect } from "vitest";
import {
  checkAllowlist,
  assertAllowlisted,
  parseAllowlistInput,
} from "../src/allowlist.js";
import { isValidE164 } from "../src/e164.js";

const policy = {
  numbers: ["+15551112222", "+16452067656"],
  emptyMeansDeny: true,
};

describe("e164", () => {
  it("accepts valid", () => {
    expect(isValidE164("+15551234567")).toBe(true);
    expect(isValidE164("+447911123456")).toBe(true);
  });
  it("rejects invalid", () => {
    expect(isValidE164("5551234567")).toBe(false);
    expect(isValidE164("+1 555")).toBe(false);
    expect(isValidE164("")).toBe(false);
  });
});

describe("allowlist", () => {
  it("allows listed number", () => {
    const d = checkAllowlist("+15551112222", policy);
    expect(d.allowed).toBe(true);
  });
  it("denies unlisted", () => {
    const d = checkAllowlist("+19998887777", policy);
    expect(d.allowed).toBe(false);
    expect(() => assertAllowlisted("+19998887777", policy)).toThrow(
      /allowlist/,
    );
  });
  it("empty allowlist denies by default", () => {
    const d = checkAllowlist("+15551112222", {
      numbers: [],
      emptyMeansDeny: true,
    });
    expect(d.allowed).toBe(false);
  });
  it("empty allowlist can allow when emptyMeansDeny false", () => {
    const d = checkAllowlist("+15551112222", {
      numbers: [],
      emptyMeansDeny: false,
    });
    expect(d.allowed).toBe(true);
  });
  it("denies invalid E.164", () => {
    expect(checkAllowlist("555", policy).allowed).toBe(false);
  });
  it("parseAllowlistInput", () => {
    expect(parseAllowlistInput("+1a, +1b\n+1c")).toEqual(["+1a", "+1b", "+1c"]);
  });
});

