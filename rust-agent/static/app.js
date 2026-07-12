/**
 * Agent console UI bootstrap (ES module).
 * Chat SSE parsing lives in static/js/sse.js and is unit-tested.
 */
import { parseSseDataLine } from "./js/sse.js";
import {
  activeSessionKey,
  fetchSessionDetail,
  loadLocalSessions,
  loadSessionsMerged,
  saveLocalSessions,
  splitAgent,
  syncSessionToServer,
} from "./js/sessions.js";

const $ = (sel, el = document) => el.querySelector(sel);

import { initToolChips } from "./js/tools.js";
initToolChips();

async function renderSessions(agent) {
  const list = $("#session-list");
  if (!list) return;
  list.innerHTML = `<li class="muted">Loading…</li>`;
  const sessions = await loadSessionsMerged(agent);
  list.innerHTML = "";
  if (!sessions.length) {
    list.innerHTML = `<li class="muted">No sessions yet.</li>`;
    return;
  }
  const active = localStorage.getItem(activeSessionKey(agent));
  const { category, name } = splitAgent(agent);
  for (const s of sessions) {
    const li = document.createElement("li");
    const label = document.createElement("span");
    const count = s.messages?.length || s._message_count || 0;
    label.textContent = `${s.id.slice(0, 8)} · ${count} msgs · ${s.updated || ""}`;
    if (s.id === active) label.style.color = "var(--text)";
    const btn = document.createElement("button");
    btn.type = "button";
    btn.textContent = "Open";
    btn.addEventListener("click", () => {
      localStorage.setItem(activeSessionKey(agent), s.id);
      location.href = `/agents/${category}/${name}?tab=chat`;
    });
    li.append(label, btn);
    list.appendChild(li);
  }
}

const newBtn = $("#btn-new-session");
if (newBtn) {
  const agent = newBtn.dataset.agent;
  newBtn.addEventListener("click", async () => {
    const id = crypto.randomUUID();
    const session = {
      id,
      created: new Date().toISOString(),
      updated: new Date().toISOString(),
      messages: [],
    };
    const sessions = loadLocalSessions(agent);
    sessions.unshift(session);
    saveLocalSessions(agent, sessions);
    localStorage.setItem(activeSessionKey(agent), id);
    await syncSessionToServer(agent, session);
    renderSessions(agent);
  });
  renderSessions(agent);
}

const form = $("#chat-form");
const transcript = $("#transcript");
const input = $("#chat-input");
if (form && transcript && input) {
  const agent = form.dataset.agent;
  let sessions = loadLocalSessions(agent);
  let activeId = localStorage.getItem(activeSessionKey(agent));
  if (!activeId || !sessions.find((s) => s.id === activeId)) {
    activeId = crypto.randomUUID();
    sessions.unshift({
      id: activeId,
      created: new Date().toISOString(),
      updated: new Date().toISOString(),
      messages: [],
    });
    saveLocalSessions(agent, sessions);
    localStorage.setItem(activeSessionKey(agent), activeId);
    syncSessionToServer(agent, sessions[0]);
  }

  function currentSession() {
    sessions = loadLocalSessions(agent);
    return sessions.find((s) => s.id === activeId);
  }

  function persist(session) {
    session.updated = new Date().toISOString();
    const all = loadLocalSessions(agent).filter((s) => s.id !== session.id);
    all.unshift(session);
    saveLocalSessions(agent, all);
    syncSessionToServer(agent, session);
  }

  function addBubble(role, text, streaming = false) {
    const div = document.createElement("div");
    div.className = `msg ${role}${streaming ? " streaming" : ""}`;
    const roleEl = document.createElement("span");
    roleEl.className = "role";
    roleEl.textContent = role;
    const body = document.createElement("div");
    body.className = "content";
    body.textContent = text;
    div.append(roleEl, body);
    transcript.appendChild(div);
    transcript.scrollTop = transcript.scrollHeight;
    return { root: div, body };
  }

  function addToolCard(kind, title, payload) {
    const div = document.createElement("div");
    div.className = `msg tool ${kind}`;
    const roleEl = document.createElement("span");
    roleEl.className = "role";
    roleEl.textContent = kind === "tool_call" ? "tool →" : "tool ←";
    const body = document.createElement("div");
    body.className = "content";
    const head = document.createElement("div");
    head.className = "tool-title";
    head.textContent = title;
    const pre = document.createElement("pre");
    pre.className = "tool-payload";
    pre.textContent =
      typeof payload === "string" ? payload : JSON.stringify(payload, null, 2);
    body.append(head, pre);
    div.append(roleEl, body);
    transcript.appendChild(div);
    transcript.scrollTop = transcript.scrollHeight;
  }

  (async () => {
    const remote = await fetchSessionDetail(agent, activeId);
    if (remote?.messages?.length) {
      persist({
        id: remote.id,
        created: remote.created,
        updated: remote.updated,
        title: remote.title,
        messages: remote.messages,
      });
    }
    const sess = currentSession();
    if (sess?.messages?.length) {
      for (const m of sess.messages) {
        if (m.role === "tool_call" || m.role === "tool_result" || m.role === "tool") {
          addToolCard(
            m.role === "tool_call" ? "tool_call" : "tool_result",
            m.role,
            m.content
          );
        } else {
          addBubble(m.role, m.content);
        }
      }
    }
  })();

  let busy = false;
  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    if (busy) return;
    const message = input.value.trim();
    if (!message) return;
    const session = currentSession();
    if (!session) return;

    busy = true;
    form.querySelector("button").disabled = true;
    input.value = "";
    addBubble("user", message);
    session.messages.push({ role: "user", content: message });
    persist(session);

    const assistant = addBubble("assistant", "", true);
    let full = "";
    let sawTool = false;
    let suppressDeltas = false;
    let finalText = "";

    try {
      const history = session.messages
        .slice(0, -1)
        .filter((m) => m.role === "user" || m.role === "assistant")
        .map((m) => ({ role: m.role, content: m.content }));

      const res = await fetch("/api/chat/stream", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ agent_stem: agent, message, history }),
      });
      if (!res.ok) throw new Error((await res.text()) || res.statusText);

      const reader = res.body.getReader();
      const decoder = new TextDecoder();
      let buf = "";
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        buf += decoder.decode(value, { stream: true });
        const parts = buf.split("\n");
        buf = parts.pop() || "";
        for (const line of parts) {
          const ev = parseSseDataLine(line);
          if (ev.kind === "skip" || ev.kind === "done") continue;
          if (ev.kind === "error") throw new Error(ev.error);
          if (ev.kind === "tool_call") {
            sawTool = true;
            suppressDeltas = true;
            full = "";
            assistant.body.textContent = "";
            addToolCard("tool_call", ev.tool_call.name, ev.tool_call.arguments || {});
            session.messages.push({
              role: "tool_call",
              content: JSON.stringify(ev.tool_call),
            });
            continue;
          }
          if (ev.kind === "tool_result") {
            sawTool = true;
            suppressDeltas = false;
            full = "";
            assistant.body.textContent = "";
            addToolCard(
              "tool_result",
              `${ev.tool_result.name} ${ev.tool_result.ok ? "ok" : "err"}`,
              ev.tool_result.result
            );
            session.messages.push({
              role: "tool_result",
              content: JSON.stringify(ev.tool_result),
            });
            continue;
          }
          if (ev.kind === "delta") {
            if (suppressDeltas) continue;
            full += ev.delta;
            finalText = full;
            assistant.body.textContent = full;
            transcript.scrollTop = transcript.scrollHeight;
          }
        }
      }

      if (!finalText && !sawTool) finalText = "(empty response)";
      if (!finalText && sawTool) finalText = full || "";
      assistant.root.classList.remove("streaming");
      if (finalText) {
        assistant.body.textContent = finalText;
        session.messages.push({ role: "assistant", content: finalText });
      } else if (!sawTool) {
        assistant.body.textContent = "(empty response)";
        session.messages.push({ role: "assistant", content: "(empty response)" });
      } else {
        assistant.root.remove();
      }
      persist(session);
    } catch (err) {
      assistant.root.classList.remove("streaming");
      assistant.root.classList.add("error");
      assistant.body.textContent = `Error: ${err.message || err}`;
    } finally {
      busy = false;
      form.querySelector("button").disabled = false;
      input.focus();
    }
  });

  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      form.requestSubmit();
    }
  });
}
