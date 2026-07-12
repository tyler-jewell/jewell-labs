//! Agent main panel: chat/sessions/schema + tool chips.

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

    // Primary tabs first; schema/registry secondary
    let primary = ["chat", "sessions"];
    let tabs: Vec<(AgentTabDef, String, String)> = AGENT_TABS
        .iter()
        .map(|t| {
            let href = format!("/agents/{category}/{name}?tab={}", t.id);
            let class = if t.id == tab {
                if primary.contains(&t.id) {
                    "tab active".into()
                } else {
                    "tab active secondary".into()
                }
            } else if primary.contains(&t.id) {
                "tab".into()
            } else {
                "tab secondary".into()
            };
            (*t, href, class)
        })
        .collect();

    view! {
        <header class="agent-header">
            <div class="agent-title-row">
                <h1 class="agent-title">{id.clone()}</h1>
                <span class="src-pill">{format!("agents/{category}/{name}.md")}</span>
            </div>
            <nav class="tabs" aria-label="Agent workspace">
                {tabs.into_iter().map(|(tdef, href, class)| {
                    view! {
                        <a class=class href=href title=tdef.src_module>{tdef.label}</a>
                    }
                }).collect_view()}
            </nav>
            <div class="tool-meta" id="tool-meta" data-tools=allowed_tools_json.clone()>
                <div class="tool-meta-label">"Available tools"</div>
                <div class="tool-chips" id="tool-chips"></div>
                <pre class="tool-def code-block" id="tool-def" hidden></pre>
            </div>
        </header>
        <section class="panel">
            {if tab == "schema" {
                view! {
                    <div class="settings" data-tab="schema">
                        <h2>"Frontmatter"</h2>
                        <pre class="code-block">{settings_json}</pre>
                        <h2>"Certification"</h2>
                        <pre class="code-block">{cert_json}</pre>
                        <h2>"System prompt"</h2>
                        <pre class="code-block body-block">{system_body}</pre>
                        <h2>"Allowed tools"</h2>
                        <pre class="code-block">{allowed_tools_json}</pre>
                    </div>
                }.into_any()
            } else if tab == "registry" {
                view! {
                    <div class="settings" data-tab="registry">
                        <h2>"Model registry"</h2>
                        <pre class="code-block">{registry_json}</pre>
                    </div>
                }.into_any()
            } else if tab == "sessions" {
                view! {
                    <div class="sessions" data-agent=id.clone() data-tab="sessions">
                        <h2>"Sessions"</h2>
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
                        <div class="transcript" id="transcript" aria-live="polite"></div>
                        <form class="composer" id="chat-form" data-agent=id_form>
                            <textarea id="chat-input" name="message" rows="2"
                                placeholder="Message this agent…" autocomplete="off"></textarea>
                            <button type="submit" class="btn primary" id="btn-send">"Send"</button>
                        </form>
                    </div>
                }.into_any()
            }}
        </section>
    }
}
