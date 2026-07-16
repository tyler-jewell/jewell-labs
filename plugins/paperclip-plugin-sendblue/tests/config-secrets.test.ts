import { describe, expect, it } from "vitest";
import { resolveSecretValue } from "../src/worker/config-load.js";
import { writeFileSync, mkdtempSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

describe("resolveSecretValue first-class order", () => {
  it("prefers plain over env/file/vault", async () => {
    const r = await resolveSecretValue({
      plain: " from-plain ",
      envNames: ["TEST_SB_KEY_NEVER"],
      resolveVault: async () => "from-vault",
    });
    expect(r).toEqual({ value: "from-plain", source: "plain" });
  });

  it("uses env before vault", async () => {
    const envName = "TEST_SENDBLUE_SECRET_ORDER";
    const prev = process.env[envName];
    process.env[envName] = "from-env";
    try {
      const r = await resolveSecretValue({
        envNames: [envName],
        resolveVault: async () => "from-vault",
      });
      expect(r).toEqual({ value: "from-env", source: "env" });
    } finally {
      if (prev === undefined) delete process.env[envName];
      else process.env[envName] = prev;
    }
  });

  it("uses ops file before vault", async () => {
    const dir = mkdtempSync(join(tmpdir(), "sb-secret-"));
    const path = join(dir, "key");
    writeFileSync(path, "from-file\n", "utf8");
    const r = await resolveSecretValue({
      filePaths: [path],
      resolveVault: async () => "from-vault",
    });
    expect(r).toEqual({ value: "from-file", source: "file" });
  });

  it("uses vault ref when plain/env/file absent", async () => {
    const r = await resolveSecretValue({
      ref: "my-ref",
      canonicalKey: "sendblue-api-key",
      resolveVault: async (key) =>
        key === "my-ref" ? "from-vault-ref" : undefined,
    });
    expect(r).toEqual({ value: "from-vault-ref", source: "vault_ref" });
  });

  it("uses vault canonical when ref misses", async () => {
    const r = await resolveSecretValue({
      ref: "missing-ref",
      canonicalKey: "sendblue-api-key",
      resolveVault: async (key) =>
        key === "sendblue-api-key" ? "from-canonical" : undefined,
    });
    expect(r).toEqual({ value: "from-canonical", source: "vault_canonical" });
  });

  it("returns empty when all sources miss (vault silent)", async () => {
    const r = await resolveSecretValue({
      ref: "x",
      canonicalKey: "y",
      envNames: ["TEST_SB_MISSING_XYZ"],
      filePaths: ["/nonexistent/sb-secret-path"],
      resolveVault: async () => undefined,
    });
    expect(r).toEqual({});
  });
});
