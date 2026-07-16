/**
 * Provisioned worker credentials on the host filesystem / env.
 * First-class sources for webhook workers (alongside plain config + vault).
 */

import { readFileSync } from "node:fs";

export const SENDBLUE_API_KEY_PATHS = [
  "/paperclip/instances/default/secrets/sendblue-api-key",
  "/paperclip/secrets/sendblue-api-key",
] as const;

export const SENDBLUE_API_SECRET_PATHS = [
  "/paperclip/instances/default/secrets/sendblue-api-secret",
  "/paperclip/secrets/sendblue-api-secret",
] as const;

export const SENDBLUE_WEBHOOK_SECRET_PATHS = [
  "/paperclip/instances/default/secrets/sendblue-webhook-secret",
  "/paperclip/secrets/sendblue-webhook-secret",
] as const;

/** Board Bearer for the webhook board REST client (resolve / create / wake). */
export const BOARD_API_KEY_PATHS = [
  "/paperclip/instances/default/secrets/board-api-key-sendblue-ops",
  "/paperclip/secrets/board-api-key-sendblue-ops",
] as const;

/** Env names for the board REST client token. */
export const BOARD_API_KEY_ENV = [
  "BOARD_API_KEY",
  "PAPERCLIP_API_KEY",
  "PAPERCLIP_BOARD_API_KEY",
] as const;

export function readFirstExistingFile(
  paths: readonly string[],
): string | undefined {
  for (const p of paths) {
    try {
      const v = readFileSync(p, "utf8").trim();
      if (v) return v;
    } catch {
      /* missing */
    }
  }
  return undefined;
}

export function boardPublicOrigin(): string {
  const a = process.env.PAPERCLIP_PUBLIC_URL?.replace(/\/$/, "");
  const b = process.env.PAPERCLIP_AUTH_PUBLIC_BASE_URL?.replace(/\/$/, "");
  return a ?? b ?? "https://paperclip-t8tg.srv1829398.hstgr.cloud";
}

export function boardApiBase(): string {
  const a = process.env.PAPERCLIP_INTERNAL_API_BASE?.replace(/\/$/, "");
  return a ?? "http://127.0.0.1:3100";
}
