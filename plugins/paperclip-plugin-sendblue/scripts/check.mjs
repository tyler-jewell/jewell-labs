#!/usr/bin/env node
/**
 * Single quality gate: version · lint · typecheck · dead-code · test · build
 * Exit non-zero on first failure. Used by CI and prepublishOnly.
 */
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const npm = process.platform === "win32" ? "npm.cmd" : "npm";

const steps = [
  ["version:check", ["run", "version:check"]],
  ["lint", ["run", "lint"]],
  ["typecheck", ["run", "typecheck"]],
  ["deadcode", ["run", "deadcode"]],
  ["test", ["run", "test"]],
  ["build", ["run", "build"]],
];

const started = Date.now();
console.log("check: starting quality gate\n");

for (const [name, args] of steps) {
  console.log(`── ${name} ──`);
  const r = spawnSync(npm, args, {
    cwd: root,
    stdio: "inherit",
    env: process.env,
  });
  if (r.status !== 0) {
    console.error(`\ncheck: FAILED at step "${name}" (exit ${String(r.status)})`);
    process.exit(r.status ?? 1);
  }
  console.log("");
}

const secs = ((Date.now() - started) / 1000).toFixed(1);
console.log(`check: all steps passed (${secs}s)`);
