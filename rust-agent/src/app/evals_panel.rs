//! Global Evals dashboard — pass/fail first, structured results (no JSON dump).

use leptos::prelude::*;

#[component]
pub(crate) fn EvalsPanel() -> impl IntoView {
    view! {
        <header class="agent-header">
            <div class="agent-title-row">
                <h1 class="agent-title">"Evals"</h1>
            </div>
        </header>
        <section class="panel evals-panel" id="evals-root" data-tab="evals">
            <div class="evals-controls">
                <button type="button" class="btn primary" id="btn-run-team">"Run team gate"</button>
                <button type="button" class="btn primary" id="btn-run-catalog">"Catalog sample"</button>
                <button type="button" class="btn ghost" id="btn-run-compare">"Legacy local tasks"</button>
                <div class="evals-agent-run">
                    <select id="eval-agent" aria-label="Agent">
                        <option value="core/orchestrator">"orchestrator"</option>
                        <option value="system/learner">"learner"</option>
                        <option value="system/agent-implementor">"agent-implementor"</option>
                        <option value="system/tool-implementor">"tool-implementor"</option>
                    </select>
                    <button type="button" class="btn ghost" id="btn-run-agent">"Run agent"</button>
                </div>
            </div>
            <p class="evals-hint muted">"Catalog sample: SWE-bench · Terminal-Bench · BFCL (filterable, seeded). Team gate stays structural CI."</p>
            <p class="evals-status" id="eval-status" hidden></p>
            <div class="eval-result" id="eval-result">
                <p class="muted eval-result-empty" id="eval-result-empty">"Select a run, run the team gate, or compare harnesses."</p>
            </div>
            <div class="evals-history">
                <h2>"Recent"</h2>
                <ul id="eval-run-list" class="eval-run-list"></ul>
            </div>
        </section>
    }
}
