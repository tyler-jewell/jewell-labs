import { describe, expect, it, vi } from "vitest";
import {
  createIssueViaBoardRest,
  resolveRoutingViaBoardRest,
  wakeAssigneeViaBoardRest,
  type BoardHttpDeps,
} from "../src/worker/board-rest.js";

function jsonResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function makeDeps(
  handler: (method: string, url: string, body?: string) => Response,
): BoardHttpDeps {
  return {
    readToken: () => "test-board-token",
    apiBase: () => "http://board.test",
    origin: () => "https://board.public",
    fetch: vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      const method = (init?.method ?? "GET").toUpperCase();
      return handler(method, url, init?.body ? String(init.body) : undefined);
    }) as typeof fetch,
  };
}

describe("board REST primary path", () => {
  it("resolveRouting prefers Jewell company with engineer", async () => {
    const deps = makeDeps((method, url) => {
      if (method === "GET" && url.endsWith("/api/companies")) {
        return jsonResponse(200, [
          { id: "co-other", name: "Other Co", status: "active" },
          { id: "co-jewell", name: "Jewell Labs", status: "active" },
        ]);
      }
      if (url.includes("/companies/co-other/agents")) {
        return jsonResponse(200, [{ id: "ag-other", role: "researcher" }]);
      }
      if (url.includes("/companies/co-jewell/agents")) {
        return jsonResponse(200, [
          { id: "ag-eng", role: "engineer" },
          { id: "ag-2", role: "researcher" },
        ]);
      }
      if (url.includes("/projects")) {
        return jsonResponse(200, [{ id: "proj-1" }]);
      }
      return jsonResponse(404, { error: "no" });
    });

    const routing = await resolveRoutingViaBoardRest(deps);
    expect(routing.companyId).toBe("co-jewell");
    expect(routing.assigneeAgentId).toBe("ag-eng");
    expect(routing.projectId).toBe("proj-1");
  });

  it("resolveRouting returns empty without board token", async () => {
    const deps: BoardHttpDeps = {
      ...makeDeps(() => jsonResponse(200, [])),
      readToken: () => undefined,
    };
    const routing = await resolveRoutingViaBoardRest(deps);
    expect(routing).toEqual({});
  });

  it("createIssueViaBoardRest posts company issue", async () => {
    const deps = makeDeps((method, url, body) => {
      expect(method).toBe("POST");
      expect(url).toBe("http://board.test/api/companies/co-1/issues");
      const parsed = JSON.parse(body ?? "{}") as Record<string, unknown>;
      expect(parsed.title).toBe("Hello");
      expect(parsed.status).toBe("todo");
      expect(parsed.assigneeAgentId).toBe("ag-1");
      return jsonResponse(201, { id: "iss-9", identifier: "JEW-9" });
    });

    const result = await createIssueViaBoardRest(
      {
        companyId: "co-1",
        title: "Hello",
        description: "body",
        assigneeAgentId: "ag-1",
      },
      deps,
    );
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.issue.id).toBe("iss-9");
      expect(result.issue.identifier).toBe("JEW-9");
    }
  });

  it("createIssueViaBoardRest surfaces HTTP errors", async () => {
    const deps = makeDeps(() => jsonResponse(403, { error: "forbidden" }));
    const result = await createIssueViaBoardRest(
      { companyId: "co-1", title: "x", description: "y" },
      deps,
    );
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toMatch(/HTTP 403/);
    }
  });

  it("wakeAssigneeViaBoardRest patches issue then wakes agent", async () => {
    const calls: string[] = [];
    const deps = makeDeps((method, url) => {
      calls.push(`${method} ${url}`);
      if (method === "PATCH" && url.includes("/api/issues/iss-1")) {
        return jsonResponse(200, { id: "iss-1", status: "todo" });
      }
      if (method === "POST" && url.includes("/api/agents/ag-1/wakeup")) {
        return jsonResponse(200, { ok: true });
      }
      return jsonResponse(500, { error: "unexpected" });
    });

    const result = await wakeAssigneeViaBoardRest(
      {
        issueId: "iss-1",
        companyId: "co-1",
        assigneeAgentId: "ag-1",
      },
      deps,
    );
    expect(result.ok).toBe(true);
    expect(calls).toEqual([
      "PATCH http://board.test/api/issues/iss-1",
      "POST http://board.test/api/agents/ag-1/wakeup",
    ]);
  });

  it("wakeAssigneeViaBoardRest fails when wakeup HTTP errors", async () => {
    const deps = makeDeps((method, url) => {
      if (method === "PATCH") return jsonResponse(200, {});
      if (url.includes("/wakeup")) return jsonResponse(502, { error: "bad" });
      return jsonResponse(404, {});
    });
    const result = await wakeAssigneeViaBoardRest(
      {
        issueId: "iss-1",
        companyId: "co-1",
        assigneeAgentId: "ag-1",
      },
      deps,
    );
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toMatch(/HTTP 502/);
    }
  });
});
