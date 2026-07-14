//! View-model types for the agent console shell.

#[derive(Clone, Debug, Default)]
pub struct AgentRowView {
    pub id: String,
    pub category: String,
    pub stem: String,
    pub name: String,
    pub description: String,
    pub role: String,
    pub default_model: String,
    pub tools: Vec<String>,
    pub cert_ok: bool,
    pub selected: bool,
    /// Operational presence: `idle` or `busy`.
    pub presence: String,
}

#[derive(Clone, Debug)]
pub struct CategoryAgents {
    pub category: String,
    pub agents: Vec<AgentRowView>,
}

/// What the main panel is showing.
#[derive(Clone, Debug)]
pub enum ShellFocus {
    Empty,
    Agent {
        id: String,
        tab: String,
        settings_json: String,
        cert_json: String,
        system_body: String,
        registry_json: String,
        allowed_tools_json: String,
    },
    /// Global evals dashboard (not per-agent).
    Evals,
}
