//! Leptos SSR UI — structure mirrors on-disk agents/ + tools/ and src/ domain tabs.

mod panels;
mod shell;
mod views;

pub use shell::AppShell;
pub use views::{AgentRowView, CategoryAgents, CategoryTools, ShellFocus};
