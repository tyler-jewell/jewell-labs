#!/usr/bin/env node
/**
 * Bump package version + src/version.ts + CHANGELOG.md.
 *
 * Usage:
 *   node scripts/release.mjs patch|minor|major|x.y.z [--dry-run]
 *   npm run release -- patch
 *
 * After a successful bump, run `npm run check` then publish:
 *   npm publish --access public
 *
 * Paperclip consumers that installed via packageName get the new version on
 * update/reinstall from npm. Manifest `version` comes from PLUGIN_VERSION.
 * CHANGELOG.md is shipped in the package (`files`) so operators can read notes.
 */
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2).filter((a) => a !== "--");
const dryRun = args.includes("--dry-run");
const bump = args.find((a) => a !== "--dry-run");

if (!bump || bump.startsWith("-")) {
  console.error(
    "Usage: node scripts/release.mjs <patch|minor|major|x.y.z> [--dry-run]",
  );
  process.exit(1);
}

function parseSemver(v) {
  const m = /^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?$/.exec(v);
  if (!m) throw new Error(`invalid semver: ${v}`);
  return {
    major: Number(m[1]),
    minor: Number(m[2]),
    patch: Number(m[3]),
    pre: m[4],
  };
}

function formatSemver({ major, minor, patch, pre }) {
  return pre ? `${major}.${minor}.${patch}-${pre}` : `${major}.${minor}.${patch}`;
}

function nextVersion(current, kind) {
  if (/^\d+\.\d+\.\d+/.test(kind)) {
    parseSemver(kind); // validate
    return kind;
  }
  const s = parseSemver(current);
  if (kind === "major") return formatSemver({ major: s.major + 1, minor: 0, patch: 0 });
  if (kind === "minor") return formatSemver({ major: s.major, minor: s.minor + 1, patch: 0 });
  if (kind === "patch") return formatSemver({ major: s.major, minor: s.minor, patch: s.patch + 1 });
  throw new Error(`unknown bump kind: ${kind}`);
}

const pkgPath = join(root, "package.json");
const pkg = JSON.parse(readFileSync(pkgPath, "utf8"));
const oldVersion = pkg.version;
const newVersion = nextVersion(oldVersion, bump);

console.log(`release: ${oldVersion} → ${newVersion}${dryRun ? " (dry-run)" : ""}`);

const versionTsPath = join(root, "src/version.ts");
const versionTs = `/**
 * Plugin semver — keep in lockstep with package.json "version".
 * Enforced by \`npm run version:check\` and bumped by \`npm run release\`.
 * Paperclip surfaces this via the manifest \`version\` field after install/update.
 */
export const PLUGIN_VERSION = "${newVersion}";
`;

const changelogPath = join(root, "CHANGELOG.md");
let changelog = readFileSync(changelogPath, "utf8");
const today = new Date().toISOString().slice(0, 10);
const unreleasedHeader = "## [Unreleased]";
if (!changelog.includes(unreleasedHeader)) {
  console.error("CHANGELOG.md must contain '## [Unreleased]' section");
  process.exit(1);
}
const section = `## [${newVersion}] - ${today}`;
if (changelog.includes(`## [${newVersion}]`)) {
  console.error(`CHANGELOG already has section for ${newVersion}`);
  process.exit(1);
}
// Move Unreleased body under the new version, leave empty Unreleased.
const re = /## \[Unreleased\]\n([\s\S]*?)(?=\n## \[|$)/;
const match = changelog.match(re);
if (!match) {
  console.error("Could not parse Unreleased section");
  process.exit(1);
}
const body = match[1].trim();
const notes =
  body.length > 0
    ? body
    : "### Changed\n\n- Release housekeeping (no user-facing notes recorded).\n";
changelog = changelog.replace(
  re,
  `${unreleasedHeader}\n\n${section}\n\n${notes}\n\n`,
);

pkg.version = newVersion;

if (dryRun) {
  console.log("--- version.ts preview ---\n" + versionTs);
  console.log("--- changelog head ---\n" + changelog.split("\n").slice(0, 40).join("\n"));
  process.exit(0);
}

writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + "\n");
writeFileSync(versionTsPath, versionTs);
writeFileSync(changelogPath, changelog);

console.log("release: wrote package.json, src/version.ts, CHANGELOG.md");
console.log("Next:");
console.log("  npm run check");
console.log("  git add -A && git commit -m \"release(sendblue): v" + newVersion + "\"");
console.log("  git tag paperclip-plugin-sendblue-v" + newVersion);
console.log("  npm publish --access public   # from this package dir");
console.log("Paperclip: reinstall/update packageName @jewell-labs/paperclip-plugin-sendblue@" + newVersion);
