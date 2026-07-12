//! View-model types for the agent console shell.

use crate::tool_fs::ToolFileEntry;

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
}

#[derive(Clone, Debug)]
pub struct CategoryAgents {
    pub category: String,
    pub agents: Vec<AgentRowView>,
}

#[derive(Clone, Debug)]
pub struct CategoryTools {
    pub category: String,
    pub tools: Vec<ToolFileEntry>,
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
    Tool {
        category: String,
        name: String,
        detail_json: String,
    },
}
