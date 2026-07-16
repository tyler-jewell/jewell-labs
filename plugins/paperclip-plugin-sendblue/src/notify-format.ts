/**
 * Short SMS/iMessage notify bodies for Paperclip board events.
 * Keep ~140 chars for lock-screen previews; never put secrets.
 */

const MAX_LEN = 160;

function clip(s: string, max = MAX_LEN): string {
  const t = s.replace(/\s+/g, " ").trim();
  if (t.length <= max) return t;
  return `${t.slice(0, max - 1)}…`;
}

export function formatIssueDone(input: {
  identifier?: string | null;
  title?: string | null;
}): string {
  const id = input.identifier?.trim() ?? "issue";
  const title = input.title?.trim() ?? "";
  return clip(`✅ ${id} done${title ? `: ${title}` : ""}`);
}

export function formatIssueCreated(input: {
  identifier?: string | null;
  title?: string | null;
}): string {
  const id = input.identifier?.trim() ?? "issue";
  const title = input.title?.trim() ?? "";
  return clip(`🆕 ${id}${title ? `: ${title}` : ""}`);
}

export function formatApprovalRequested(input: {
  summary?: string | null;
  id?: string | null;
}): string {
  const s = input.summary?.trim() ?? input.id?.trim() ?? "approval";
  return clip(`⚠️ Approval needed: ${s}`);
}

export function formatAgentError(input: {
  agentName?: string | null;
  message?: string | null;
}): string {
  const agent = input.agentName?.trim() ?? "agent";
  const msg = input.message?.trim() ?? "error";
  return clip(`❌ ${agent}: ${msg}`);
}

export function formatGenericNotify(prefix: string, detail: string): string {
  return clip(`${prefix} ${detail}`.trim());
}
