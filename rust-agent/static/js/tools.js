/** Tool chips from allowed_tools JSON → click shows definition (from /api/tools). */
async function initToolChips() {
  const root = $("#tool-meta");
  const chips = $("#tool-chips");
  const def = $("#tool-def");
  if (!root || !chips || !def) return;
  let tools = [];
  try {
    tools = JSON.parse(root.dataset.tools || "[]");
  } catch {
    tools = [];
  }
  if (!Array.isArray(tools) || !tools.length) {
    chips.innerHTML = `<span class="muted">No tools allowlisted</span>`;
    return;
  }
  let catalog = [];
  try {
    const r = await fetch("/api/tools");
    const j = await r.json();
    catalog = j.tools || j.registered || j.from_disk || [];
    if (j.tools && !Array.isArray(j.tools) && j.registered) catalog = j.registered;
  } catch {
    catalog = [];
  }
  const byName = new Map();
  for (const t of catalog) {
    if (t?.name) byName.set(t.name, t);
  }
  chips.innerHTML = "";
  for (const t of tools) {
    const name = typeof t === "string" ? t : t.name;
    if (!name) continue;
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "tool-chip";
    btn.textContent = name;
    btn.addEventListener("click", () => {
      const spec = byName.get(name) || t;
      def.hidden = false;
      def.textContent = JSON.stringify(spec, null, 2);
    });
    chips.appendChild(btn);
  }
}

export { initToolChips };
