//! Agent and tool main panels.

use crate::nav::{normalize_agent_tab, AgentTabDef, AGENT_TABS};
use leptos::prelude::*;

#[component]
pub(crate) fn AgentPanel(
    id: String,
    tab: String,
    settings_json: String,
    cert_json: String,
    system_body: String,
    registry_json: String,
    allowed_tools_json: String,
) -> impl IntoView {
    let tab = normalize_agent_tab(&tab).to_string();
    let parts: Vec<&str> = id.splitn(2, '/').collect();
    let (category, name) = if parts.len() == 2 {
        (parts[0].to_string(), parts[1].to_string())
    } else {
        (String::new(), id.clone())
    };

    let tabs: Vec<(AgentTabDef, String, String)> = AGENT_TABS
        .iter()
        .map(|t| {
            let href = format!("/agents/{category}/{name}?tab={}", t.id);
            let class = if t.id == tab {
                "tab active".to_string()
            } else {
                "tab".to_string()
            };
            (*t, href, class)
        })
        .collect();

    let tab_meta = AGENT_TABS
        .iter()
        .find(|t| t.id == tab)
        .copied()
        .unwrap_or(AGENT_TABS[0]);

    view! {
        <header class="agent-header">
            <div class="agent-title-row">
                <h1 class="agent-title">{id.clone()}</h1>
                <span class="src-pill">{format!("agents/{category}/{name}.md")}</span>
            </div>
            <nav class="tabs" aria-label="Agent src modules">
                {tabs.into_iter().map(|(tdef, href, class)| {
                    view! {
                        <a class=class href=href title=tdef.src_module>
                            {tdef.label}
                            <span class="tab-src">{tdef.src_module}</span>
                        </a>
                    }
                }).collect_view()}
            </nav>
            <p class="tab-desc muted">{format!("{} · {}", tab_meta.src_module, tab_meta.description)}</p>
        </header>
        <section class="panel">
            {if tab == "schema" {
                view! {
                    <div class="settings" data-tab="schema">
                        <h2>"Frontmatter " <code>"(schema.rs)"</code></h2>
                        <pre class="code-block" id="settings-json">{settings_json}</pre>
                        <h2>"Certification"</h2>
                        <pre class="code-block" id="cert-json">{cert_json}</pre>
                        <h2>"System prompt body"</h2>
                        <pre class="code-block body-block">{system_body}</pre>
                        <h2>"Allowed tools " <code>"(frontmatter tools:)"</code></h2>
                        <pre class="code-block" id="allowed-tools-json">{allowed_tools_json}</pre>
                    </div>
                }.into_any()
            } else if tab == "registry" {
                view! {
                    <div class="settings" data-tab="registry">
                        <h2>"Model registry " <code>"(registry.rs)"</code></h2>
                        <p class="muted">"Resolved from this agent's default_model against models/registry.yaml."</p>
                        <pre class="code-block" id="registry-json">{registry_json}</pre>
                    </div>
                }.into_any()
            } else if tab == "sessions" {
                view! {
                    <div class="sessions" data-agent=id.clone() data-tab="sessions">
                        <h2>"Sessions " <code>"(sessions.rs)"</code></h2>
                        <p class="muted">
                            "Live store: data/sessions/{category}/{name}/ — tools under tools/sessions/ can read these."
                        </p>
                        <ul id="session-list" class="session-list"></ul>
                        <button type="button" class="btn ghost" id="btn-new-session" data-agent=id.clone()>
                            "New session"
                        </button>
                    </div>
                }.into_any()
            } else {
                let id_chat = id.clone();
                let id_form = id.clone();
                view! {
                    <div class="chat" data-agent=id_chat data-tab="chat" id="chat-root">
                        <div class="chat-hint muted">
                            "src/chat.rs -- tool loop uses tools/ allowlisted in frontmatter"
                        </div>
                        <div class="transcript" id="transcript" aria-live="polite"></div>
                        <form class="composer" id="chat-form" data-agent=id_form>
                            <textarea
                                id="chat-input"
                                name="message"
                                rows="2"
                                placeholder="Message this agent…"
                                autocomplete="off"
                            ></textarea>
                            <button type="submit" class="btn primary" id="btn-send">"Send"</button>
                        </form>
                    </div>
                }.into_any()
            }}
        </section>
    }
}

#[component]
pub(crate) fn ToolPanel(category: String, name: String, detail_json: String) -> impl IntoView {
    let path_label = format!("tools/{category}/{name}.rs");
    view! {
        <header class="agent-header">
            <div class="agent-title-row">
                <h1 class="agent-title">{format!("{category}/{name}")}</h1>
                <span class="src-pill">{path_label}</span>
            </div>
            <p class="tab-desc muted">"Tool implementation on disk; invocable via chat or POST /api/tools/invoke"</p>
        </header>
        <section class="panel">
            <div class="settings" data-tab="tool">
                <h2>"Tool spec"</h2>
                <pre class="code-block" id="tool-detail-json">{detail_json}</pre>
                <h2>"Invoke"</h2>
                <p class="muted">
                    "POST /api/tools/invoke with "
                    <code>{format!(r#"{{"name":"{name}","arguments":{{}},"caller_agent":"core/orchestrator"}}"#)}</code>
                </p>
            </div>
        </section>
    }
}
