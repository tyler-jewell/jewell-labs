/**
 * Board REST client — primary board mechanism for webhook inbound actions
 * (resolve company/agent, create issue, wake assignee).
 *
 * Webhook workers use the board HTTP API with a provisioned board API key
 * (env or ops files). Agent tool handlers still use host SDK RPCs, which run
 * under tool invocation scope.
 */

import { PLUGIN_ID } from "../constants.js";
import {
  BOARD_API_KEY_ENV,
  BOARD_API_KEY_PATHS,
  boardApiBase,
  boardPublicOrigin,
  readFirstExistingFile,
} from "../ops-paths.js";
import type { BoardRouting } from "../runtime/resolved.js";

export type BoardHttpDeps = {
  fetch: typeof fetch;
  readToken: () => string | undefined;
  apiBase: () => string;
  origin: () => string;
};

function envBoardToken(): string | undefined {
  for (const n of BOARD_API_KEY_ENV) {
    const v = process.env[n]?.trim();
    if (v) return v;
  }
  return undefined;
}

export const defaultBoardHttpDeps: BoardHttpDeps = {
  fetch: globalThis.fetch.bind(globalThis),
  readToken: () =>
    envBoardToken() ?? readFirstExistingFile(BOARD_API_KEY_PATHS),
  apiBase: boardApiBase,
  origin: boardPublicOrigin,
};

function boardHeaders(
  deps: BoardHttpDeps,
): Record<string, string> | null {
  const token = deps.readToken();
  if (!token) return null;
  return {
    Authorization: `Bearer ${token}`,
    "Content-Type": "application/json",
    Origin: deps.origin(),
  };
}

export async function boardRestJson(
  method: string,
  path: string,
  body?: unknown,
  deps: BoardHttpDeps = defaultBoardHttpDeps,
): Promise<{ ok: boolean; status: number; json: unknown; text: string }> {
  const headers = boardHeaders(deps);
  if (!headers) {
    return {
      ok: false,
      status: 0,
      json: null,
      text: "no board API key (set BOARD_API_KEY / ops file)",
    };
  }
  try {
    const res = await deps.fetch(`${deps.apiBase()}${path}`, {
      method,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    const text = await res.text();
    let json: unknown = null;
    if (text) {
      try {
        json = JSON.parse(text) as unknown;
      } catch {
        json = text;
      }
    }
    return { ok: res.ok, status: res.status, json, text };
  } catch (e) {
    return {
      ok: false,
      status: 0,
      json: null,
      text: e instanceof Error ? e.message : String(e),
    };
  }
}

function asRecordList(payload: unknown): Record<string, unknown>[] {
  if (Array.isArray(payload)) {
    return payload.filter(
      (x): x is Record<string, unknown> =>
        Boolean(x) && typeof x === "object",
    );
  }
  if (payload && typeof payload === "object") {
    const o = payload as Record<string, unknown>;
    const nested = o.items ?? o.companies ?? o.agents ?? o.projects;
    if (Array.isArray(nested)) {
      return nested.filter(
        (x): x is Record<string, unknown> =>
          Boolean(x) && typeof x === "object",
      );
    }
  }
  return [];
}

/**
 * Pick best company for inbound SMS work: prefer Jewell-named + engineer agents.
 */
export async function resolveRoutingViaBoardRest(
  deps: BoardHttpDeps = defaultBoardHttpDeps,
): Promise<BoardRouting> {
  const companiesRes = await boardRestJson("GET", "/api/companies", undefined, deps);
  const companies = asRecordList(companiesRes.json).filter((c) => {
    const status = typeof c.status === "string" ? c.status : "active";
    return status === "active" || !c.status;
  });

  type Scored = BoardRouting & { score: number; name: string };
  const scored: Scored[] = [];

  for (const c of companies) {
    const companyId = typeof c.id === "string" ? c.id : undefined;
    if (!companyId) continue;
    const name = typeof c.name === "string" ? c.name : "";
    const agentsRes = await boardRestJson(
      "GET",
      `/api/companies/${companyId}/agents`,
      undefined,
      deps,
    );
    const agents = asRecordList(agentsRes.json);
    const eng = agents.find(
      (a) => a.role === "engineer" || a.role === "ceo",
    );
    const assigneeAgentId =
      typeof eng?.id === "string"
        ? eng.id
        : typeof agents[0]?.id === "string"
          ? agents[0].id
          : undefined;
    let score = 0;
    if (assigneeAgentId) score += 2;
    if (eng) score += 2;
    if (/jewell/i.test(name)) score += 5;
    if (agents.length > 0) score += 1;
    const projectsRes = await boardRestJson(
      "GET",
      `/api/companies/${companyId}/projects`,
      undefined,
      deps,
    );
    const projects = asRecordList(projectsRes.json);
    const projectId =
      typeof projects[0]?.id === "string" ? projects[0].id : undefined;
    scored.push({ companyId, projectId, assigneeAgentId, score, name });
  }

  scored.sort((a, b) => b.score - a.score);
  const best = scored[0];
  if (!best) return {};
  return {
    companyId: best.companyId,
    projectId: best.projectId,
    assigneeAgentId: best.assigneeAgentId,
  };
}

export type BoardCreatedIssue = {
  id: string;
  identifier?: string;
};

export async function createIssueViaBoardRest(
  input: {
    companyId: string;
    title: string;
    description: string;
    projectId?: string;
    assigneeAgentId?: string;
    status?: string;
  },
  deps: BoardHttpDeps = defaultBoardHttpDeps,
): Promise<{ ok: true; issue: BoardCreatedIssue } | { ok: false; error: string }> {
  const body: Record<string, unknown> = {
    title: input.title,
    description: input.description,
    status: input.status ?? "todo",
    originKind: `plugin:${PLUGIN_ID}`,
    originId: PLUGIN_ID,
  };
  if (input.projectId) body.projectId = input.projectId;
  if (input.assigneeAgentId) body.assigneeAgentId = input.assigneeAgentId;

  const r = await boardRestJson(
    "POST",
    `/api/companies/${input.companyId}/issues`,
    body,
    deps,
  );
  if (!r.ok || !r.json || typeof r.json !== "object") {
    return {
      ok: false,
      error: `board create issue HTTP ${r.status}: ${r.text.slice(0, 300)}`,
    };
  }
  const o = r.json as Record<string, unknown>;
  const id = typeof o.id === "string" ? o.id : undefined;
  if (!id) {
    return {
      ok: false,
      error: `board create issue: no id in ${r.text.slice(0, 200)}`,
    };
  }
  return {
    ok: true,
    issue: {
      id,
      identifier: typeof o.identifier === "string" ? o.identifier : undefined,
    },
  };
}

/**
 * Ensure issue is todo and POST agent wakeup (assignment-style).
 */
export async function wakeAssigneeViaBoardRest(
  input: {
    issueId: string;
    companyId: string;
    assigneeAgentId: string;
  },
  deps: BoardHttpDeps = defaultBoardHttpDeps,
): Promise<{ ok: true } | { ok: false; error: string }> {
  await boardRestJson(
    "PATCH",
    `/api/issues/${input.issueId}`,
    { status: "todo" },
    deps,
  );

  const wake = await boardRestJson(
    "POST",
    `/api/agents/${input.assigneeAgentId}/wakeup`,
    {
      source: "assignment",
      triggerDetail: "system",
      reason: "sendblue inbound message",
      payload: {
        issueId: input.issueId,
        companyId: input.companyId,
        contextSource: PLUGIN_ID,
      },
    },
    deps,
  );
  if (!wake.ok) {
    return {
      ok: false,
      error: `board agent wakeup HTTP ${wake.status}: ${wake.text.slice(0, 300)}`,
    };
  }
  return { ok: true };
}
