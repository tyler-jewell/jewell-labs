//! Multi-agent operational presence: idle vs busy, grouped by agent.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PresenceStatus {
    Idle,
    Busy,
}

impl PresenceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Busy => "busy",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PresenceRow {
    pub agent_id: String,
    pub status: PresenceStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Unix millis for last transition.
    pub updated_ms: i64,
}

/// Pure merge: sessions + live busy map → rows sorted by agent then status.
pub fn build_presence_rows(
    agent_ids: &[String],
    busy: &HashMap<String, Option<String>>,
    now_ms: i64,
) -> Vec<PresenceRow> {
    let mut rows = Vec::with_capacity(agent_ids.len());
    for id in agent_ids {
        if let Some(sid) = busy.get(id) {
            rows.push(PresenceRow {
                agent_id: id.clone(),
                status: PresenceStatus::Busy,
                session_id: sid.clone(),
                updated_ms: now_ms,
            });
        } else {
            rows.push(PresenceRow {
                agent_id: id.clone(),
                status: PresenceStatus::Idle,
                session_id: None,
                updated_ms: now_ms,
            });
        }
    }
    rows.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));
    rows
}

/// Group rows by category prefix (`core` from `core/orchestrator`).
pub fn group_by_project(rows: &[PresenceRow]) -> Vec<(String, Vec<PresenceRow>)> {
    let mut map: HashMap<String, Vec<PresenceRow>> = HashMap::new();
    for r in rows {
        let project = r
            .agent_id
            .split_once('/')
            .map(|(p, _)| p.to_string())
            .unwrap_or_else(|| "default".into());
        map.entry(project).or_default().push(r.clone());
    }
    let mut groups: Vec<_> = map.into_iter().collect();
    groups.sort_by(|a, b| a.0.cmp(&b.0));
    for (_, agents) in &mut groups {
        agents.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));
    }
    groups
}

#[derive(Clone, Default)]
pub struct PresenceBoard {
    inner: Arc<Mutex<HashMap<String, PresenceRow>>>,
}

impl PresenceBoard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_busy(&self, agent_id: &str, session_id: Option<&str>) {
        let mut g = self.inner.lock().expect("presence lock");
        g.insert(
            agent_id.to_string(),
            PresenceRow {
                agent_id: agent_id.to_string(),
                status: PresenceStatus::Busy,
                session_id: session_id.map(|s| s.to_string()),
                updated_ms: Utc::now().timestamp_millis(),
            },
        );
    }

    pub fn set_idle(&self, agent_id: &str) {
        let mut g = self.inner.lock().expect("presence lock");
        g.insert(
            agent_id.to_string(),
            PresenceRow {
                agent_id: agent_id.to_string(),
                status: PresenceStatus::Idle,
                session_id: None,
                updated_ms: Utc::now().timestamp_millis(),
            },
        );
    }

    pub fn snapshot(&self) -> Vec<PresenceRow> {
        let g = self.inner.lock().expect("presence lock");
        let mut rows: Vec<_> = g.values().cloned().collect();
        rows.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));
        rows
    }

    /// Overlay live board onto agent list (unknown agents default idle).
    pub fn for_agents(&self, agent_ids: &[String]) -> Vec<PresenceRow> {
        let g = self.inner.lock().expect("presence lock");
        let now = Utc::now().timestamp_millis();
        let busy: HashMap<String, Option<String>> = g
            .iter()
            .filter(|(_, r)| r.status == PresenceStatus::Busy)
            .map(|(k, r)| (k.clone(), r.session_id.clone()))
            .collect();
        drop(g);
        build_presence_rows(agent_ids, &busy, now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_marks_busy_and_idle() {
        let agents = vec!["core/a".into(), "core/b".into()];
        let mut busy = HashMap::new();
        busy.insert("core/a".into(), Some("s1".into()));
        let rows = build_presence_rows(&agents, &busy, 1000);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].status, PresenceStatus::Busy);
        assert_eq!(rows[0].session_id.as_deref(), Some("s1"));
        assert_eq!(rows[1].status, PresenceStatus::Idle);
    }

    #[test]
    fn group_by_project_path() {
        let rows = vec![
            PresenceRow {
                agent_id: "core/orch".into(),
                status: PresenceStatus::Idle,
                session_id: None,
                updated_ms: 0,
            },
            PresenceRow {
                agent_id: "tutoring/math".into(),
                status: PresenceStatus::Busy,
                session_id: None,
                updated_ms: 0,
            },
        ];
        let g = group_by_project(&rows);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].0, "core");
        assert_eq!(g[1].0, "tutoring");
    }

    #[test]
    fn board_busy_idle_roundtrip() {
        let b = PresenceBoard::new();
        b.set_busy("core/orchestrator", Some("sess"));
        let snap = b.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].status, PresenceStatus::Busy);
        b.set_idle("core/orchestrator");
        assert_eq!(b.snapshot()[0].status, PresenceStatus::Idle);
    }
}
