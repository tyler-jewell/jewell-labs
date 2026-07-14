//! Leptos SSR UI — agents-first console + evals dashboard.

mod evals_panel;
mod panels;
mod shell;
mod views;

pub use shell::AppShell;
pub use views::{AgentRowView, CategoryAgents, ShellFocus};
