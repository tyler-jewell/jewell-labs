/** Load/store conversation records + live board routing (no plugin UUID config). */

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { PluginContext } from "@paperclipai/plugin-sdk";
import {
  CONVERSATION_STATE_KEY,
  parseConversation,
  type ConversationRecord,
} from "../runtime/conversation.js";
import type { BoardRouting } from "../runtime/resolved.js";
import { resolveRoutingViaBoardRest } from "./board-rest.js";

const FILE_CONV_DIR =
  "/paperclip/instances/default/data/sendblue-conversations";

function filePathForIssue(issueId: string): string {
  return join(FILE_CONV_DIR, `${issueId}.json`);
}

function saveConversationFile(
  issueId: string,
  record: ConversationRecord,
): void {
  mkdirSync(FILE_CONV_DIR, { recursive: true });
  writeFileSync(filePathForIssue(issueId), JSON.stringify(record), {
    encoding: "utf8",
    mode: 0o600,
  });
}

function loadConversationFile(issueId: string): ConversationRecord | null {
  try {
    const raw = readFileSync(filePathForIssue(issueId), "utf8");
    return parseConversation(JSON.parse(raw) as unknown);
  } catch {
    return null;
  }
}

export async function saveConversation(
  ctx: PluginContext,
  issueId: string,
  record: ConversationRecord,
): Promise<void> {
  // Disk is the durable mapping for webhook workers; plugin state is best-effort.
  try {
    saveConversationFile(issueId, record);
  } catch (err) {
    ctx.logger.warn("conversation file save failed", {
      issueId,
      error: String(err),
    });
  }
  try {
    await ctx.state.set(
      {
        scopeKind: "issue",
        scopeId: issueId,
        stateKey: CONVERSATION_STATE_KEY,
      },
      record,
    );
  } catch {
    /* optional — file is primary */
  }
}

export async function loadConversation(
  ctx: PluginContext,
  issueId: string,
): Promise<ConversationRecord | null> {
  const fromFile = loadConversationFile(issueId);
  if (fromFile) return fromFile;
  try {
    const raw = await ctx.state.get({
      scopeKind: "issue",
      scopeId: issueId,
      stateKey: CONVERSATION_STATE_KEY,
    });
    return parseConversation(raw);
  } catch {
    return null;
  }
}

/**
 * Resolve company + assignee from the live board (plugin is instance-wide;
 * company UUIDs are never stored in plugin config).
 * Prefers companies with engineer agents / Jewell-named orgs when multi-company.
 */
export async function resolveCompanyAndAgent(
  ctx: PluginContext,
): Promise<BoardRouting> {
  const viaRest = await resolveRoutingViaBoardRest();
  if (!viaRest.companyId) {
    ctx.logger.warn("board REST routing found no company");
  }
  return viaRest;
}
