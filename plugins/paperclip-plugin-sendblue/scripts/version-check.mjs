#!/usr/bin/env node
/**
 * Fail if package.json version !== src/version.ts PLUGIN_VERSION.
 * Paperclip installs read package + manifest version; drift confuses updates.
 */
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const versionTs = readFileSync(join(root, "src/version.ts"), "utf8");
const m = versionTs.match(
  /export const PLUGIN_VERSION\s*=\s*["']([^"']+)["']/,
);
if (!m) {
  console.error("version-check: could not parse PLUGIN_VERSION from src/version.ts");
  process.exit(1);
}
const codeVersion = m[1];
const pkgVersion = pkg.version;
if (codeVersion !== pkgVersion) {
  console.error(
    `version-check: mismatch — package.json="${pkgVersion}" src/version.ts="${codeVersion}"`,
  );
  console.error("Run: npm run release -- <patch|minor|major|x.y.z>");
  process.exit(1);
}
console.log(`version-check: ok (${pkgVersion})`);
