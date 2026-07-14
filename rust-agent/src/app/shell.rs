//! App shell: agents sidebar + evals nav + main focus (no tools tree).

use super::evals_panel::EvalsPanel;
use super::panels::AgentPanel;
use super::views::{AgentRowView, CategoryAgents, ShellFocus};
use leptos::prelude::*;

#[component]
pub fn AppShell(
    agent_groups: Vec<CategoryAgents>,
    focus: ShellFocus,
    evals_selected: bool,
) -> impl IntoView {
    let evals_class = if evals_selected {
        "nav-item selected"
    } else {
        "nav-item"
    };
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <title>"Jewell Labs"</title>
                <link rel="stylesheet" href="/static/css/base.css"/>
                <link rel="stylesheet" href="/static/css/sidebar.css"/>
                <link rel="stylesheet" href="/static/css/main.css"/>
                <link rel="stylesheet" href="/static/css/chat.css"/>
                <link rel="stylesheet" href="/static/css/settings.css"/>
            </head>
            <body>
                <div class="app">
                    <aside class="sidebar">
                        <div class="brand">
                            <div class="brand-mark">"JL"</div>
                            <div class="brand-title">"Jewell Labs"</div>
                        </div>
                        <div class="sidebar-scroll">
                            <section class="sidebar-section" aria-label="Agents">
                                <div class="sidebar-label">"Agents"</div>
                                <nav class="nav-tree" aria-label="Agents by category">
                                    {agent_groups.into_iter().map(|g| {
                                        let cat = g.category.clone();
                                        view! {
                                            <div class="nav-category">
                                                <div class="nav-category-name">{cat}</div>
                                                <div class="nav-items">
                                                    {g.agents.into_iter().map(|a: AgentRowView| {
                                                        let href = format!("/agents/{}/{}?tab=chat", a.category, a.stem);
                                                        let class = if a.selected { "nav-item selected" } else { "nav-item" };
                                                        let badge = if a.role == "orchestrator" { "ORCH" } else if a.cert_ok { "OK" } else { "!" };
                                                        let badge_class = if !a.cert_ok { "badge bad" } else if a.role == "orchestrator" { "badge orch" } else { "badge ok" };
                                                        let presence = a.presence.clone();
                                                        let presence_class = format!("presence presence-{presence}");
                                                        view! {
                                                            <a class=class href=href title=a.id.clone() data-presence=presence.clone() data-agent=a.id.clone()>
                                                                <div class="nav-item-top">
                                                                    <span class="nav-item-name">
                                                                        <span class=presence_class aria-label=presence.clone() title=presence.clone()></span>
                                                                        {a.stem.clone()}
                                                                    </span>
                                                                    <span class=badge_class>{badge}</span>
                                                                </div>
                                                            </a>
                                                        }
                                                    }).collect_view()}
                                                </div>
                                            </div>
                                        }
                                    }).collect_view()}
                                </nav>
                            </section>
                            <section class="sidebar-section" aria-label="Evals">
                                <div class="sidebar-label">"Evals"</div>
                                <nav class="nav-tree">
                                    <a class=evals_class href="/evals" data-nav="evals">
                                        <div class="nav-item-top">
                                            <span class="nav-item-name">"Runs"</span>
                                        </div>
                                    </a>
                                </nav>
                            </section>
                        </div>
                    </aside>
                    <main class="main">
                        {match focus {
                            ShellFocus::Empty => view! {
                                <div class="empty">
                                    <h1>"Loading…"</h1>
                                </div>
                            }.into_any(),
                            ShellFocus::Evals => view! { <EvalsPanel /> }.into_any(),
                            ShellFocus::Agent {
                                id, tab, settings_json, cert_json, system_body,
                                registry_json, allowed_tools_json,
                            } => view! {
                                <AgentPanel
                                    id=id
                                    tab=tab
                                    settings_json=settings_json
                                    cert_json=cert_json
                                    system_body=system_body
                                    registry_json=registry_json
                                    allowed_tools_json=allowed_tools_json
                                />
                            }.into_any(),
                        }}
                    </main>
                </div>
                <script type="module" src="/static/pkg/boot.js"></script>
            </body>
        </html>
    }
}
