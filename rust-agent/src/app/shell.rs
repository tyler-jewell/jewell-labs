//! App shell layout: sidebar + main focus area.

use super::panels::{AgentPanel, ToolPanel};
use super::views::{CategoryAgents, CategoryTools, ShellFocus};
use leptos::prelude::*;

#[component]
pub fn AppShell(
    agent_groups: Vec<CategoryAgents>,
    tool_groups: Vec<CategoryTools>,
    focus: ShellFocus,
) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <title>"Jewell Labs · Agent Console"</title>
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
                            <div class="brand-mark">JL</div>
                            <div>
                                <div class="brand-title">Jewell Labs</div>
                                <div class="brand-sub">"agents/ · tools/ · src/"</div>
                            </div>
                        </div>

                        <div class="sidebar-scroll">
                            <section class="sidebar-section" aria-label="Agents">
                                <div class="sidebar-label">
                                    <span>"Agents"</span>
                                    <span class="sidebar-path">"agents/{category}/"</span>
                                </div>
                                <nav class="nav-tree" aria-label="Agents by category">
                                    {agent_groups.into_iter().map(|g| {
                                        let cat = g.category.clone();
                                        view! {
                                            <div class="nav-category">
                                                <div class="nav-category-name">{cat}</div>
                                                <div class="nav-items">
                                                    {g.agents.into_iter().map(|a| {
                                                        let href = format!(
                                                            "/agents/{}/{}?tab=chat",
                                                            a.category, a.stem
                                                        );
                                                        let class = if a.selected {
                                                            "nav-item selected"
                                                        } else {
                                                            "nav-item"
                                                        };
                                                        let badge = if a.role == "orchestrator" {
                                                            "ORCH"
                                                        } else if a.cert_ok {
                                                            "OK"
                                                        } else {
                                                            "!"
                                                        };
                                                        let badge_class = if !a.cert_ok {
                                                            "badge bad"
                                                        } else if a.role == "orchestrator" {
                                                            "badge orch"
                                                        } else {
                                                            "badge ok"
                                                        };
                                                        let file = format!("{}.md", a.stem);
                                                        view! {
                                                            <a class=class href=href title=a.id.clone()>
                                                                <div class="nav-item-top">
                                                                    <span class="nav-item-name">{a.stem.clone()}</span>
                                                                    <span class=badge_class>{badge}</span>
                                                                </div>
                                                                <div class="nav-item-meta">{file}</div>
                                                            </a>
                                                        }
                                                    }).collect_view()}
                                                </div>
                                            </div>
                                        }
                                    }).collect_view()}
                                </nav>
                            </section>

                            <section class="sidebar-section" aria-label="Tools">
                                <div class="sidebar-label">
                                    <span>"Tools"</span>
                                    <span class="sidebar-path">"tools/{category}/"</span>
                                </div>
                                <nav class="nav-tree" aria-label="Tools by category">
                                    {tool_groups.into_iter().map(|g| {
                                        let cat = g.category.clone();
                                        view! {
                                            <div class="nav-category">
                                                <div class="nav-category-name">{cat}</div>
                                                <div class="nav-items">
                                                    {g.tools.into_iter().map(|t| {
                                                        let href = format!(
                                                            "/tools/{}/{}",
                                                            t.category, t.name
                                                        );
                                                        let class = if t.selected {
                                                            "nav-item selected"
                                                        } else {
                                                            "nav-item"
                                                        };
                                                        let badge = if t.registered { "RS" } else { "?" };
                                                        let badge_class = if t.registered {
                                                            "badge ok"
                                                        } else {
                                                            "badge bad"
                                                        };
                                                        let file = format!("{}.rs", t.name);
                                                        view! {
                                                            <a class=class href=href title=t.id.clone()>
                                                                <div class="nav-item-top">
                                                                    <span class="nav-item-name">{t.name.clone()}</span>
                                                                    <span class=badge_class>{badge}</span>
                                                                </div>
                                                                <div class="nav-item-meta">{file}</div>
                                                            </a>
                                                        }
                                                    }).collect_view()}
                                                </div>
                                            </div>
                                        }
                                    }).collect_view()}
                                </nav>
                            </section>
                        </div>

                        <div class="sidebar-foot">
                            <span class="link-quiet">"tabs = src/chat | sessions | schema | registry"</span>
                        </div>
                    </aside>
                    <main class="main">
                        {match focus {
                            ShellFocus::Empty => view! {
                                <div class="empty">
                                    <h1>"Select an agent or tool"</h1>
                                    <p>"Sidebar lists live folders under agents/ and tools/. Agent tabs map to src modules."</p>
                                </div>
                            }.into_any(),
                            ShellFocus::Agent {
                                id,
                                tab,
                                settings_json,
                                cert_json,
                                system_body,
                                registry_json,
                                allowed_tools_json,
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
                            ShellFocus::Tool { category, name, detail_json } => view! {
                                <ToolPanel
                                    category=category
                                    name=name
                                    detail_json=detail_json
                                />
                            }.into_any(),
                        }}
                    </main>
                </div>
                <script type="module" src="/static/app.js"></script>
            </body>
        </html>
    }
}
