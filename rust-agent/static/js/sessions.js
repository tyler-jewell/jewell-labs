/** Session localStorage + server sync helpers. */

export function splitAgent(agent) {
  const i = agent.indexOf("/");
  if (i < 0) return { category: "", name: agent };
  return { category: agent.slice(0, i), name: agent.slice(i + 1) };
}

export function sessionKey(agent) {
  return `jl.sessions.${agent}`;
}

export function activeSessionKey(agent) {
  return `jl.activeSession.${agent}`;
}

export function loadLocalSessions(agent) {
  try {
    return JSON.parse(localStorage.getItem(sessionKey(agent)) || "[]");
  } catch {
    return [];
  }
}

export function saveLocalSessions(agent, sessions) {
  localStorage.setItem(sessionKey(agent), JSON.stringify(sessions));
}

export function sessionApiBase(agent) {
  const { category, name } = splitAgent(agent);
  return `/api/sessions/${encodeURIComponent(category)}/${encodeURIComponent(name)}`;
}

export async function fetchServerSessions(agent) {
  try {
    const res = await fetch(`/api/sessions?agent=${encodeURIComponent(agent)}`);
    if (!res.ok) return null;
    const data = await res.json();
    return data.sessions || [];
  } catch {
    return null;
  }
}

export async function fetchSessionDetail(agent, id) {
  try {
    const res = await fetch(`${sessionApiBase(agent)}/${encodeURIComponent(id)}`);
    if (!res.ok) return null;
    return await res.json();
  } catch {
    return null;
  }
}

export async function syncSessionToServer(agent, session) {
  try {
    await fetch(`${sessionApiBase(agent)}/${encodeURIComponent(session.id)}`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        id: session.id,
        agent_stem: agent,
        created: session.created || new Date().toISOString(),
        updated: session.updated || new Date().toISOString(),
        title: session.title || null,
        messages: (session.messages || []).map((m) => ({
          role: m.role,
          content: m.content,
          ts: m.ts || null,
        })),
      }),
    });
  } catch {
    /* offline */
  }
}

export async function loadSessionsMerged(agent) {
  const local = loadLocalSessions(agent);
  const remote = await fetchServerSessions(agent);
  if (!remote) return local;
  const byId = new Map();
  for (const s of remote) {
    byId.set(s.id, {
      id: s.id,
      created: s.created,
      updated: s.updated,
      title: s.title,
      messages: [],
      _message_count: s.message_count,
    });
  }
  for (const s of local) {
    byId.set(s.id, { ...s });
  }
  return Array.from(byId.values()).sort((a, b) =>
    (b.updated || "").localeCompare(a.updated || "")
  );
}
