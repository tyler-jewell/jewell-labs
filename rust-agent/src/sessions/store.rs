//! SessionStore: disk-backed session CRUD and search.

use super::types::{
    ChatSession, MessageHit, SessionError, SessionMessage, SessionSummary,
};
use crate::agents::parse_agent_ref;
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SessionStore {
    pub root: PathBuf,
}

impl SessionStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn ensure(&self) -> Result<(), SessionError> {
        fs::create_dir_all(&self.root)?;
        Ok(())
    }

    fn agent_dir(&self, agent_id: &str) -> Result<PathBuf, SessionError> {
        let (cat, name) = parse_agent_ref(agent_id)?;
        Ok(self.root.join(cat).join(name))
    }

    fn session_path(&self, agent_id: &str, id: &str) -> Result<PathBuf, SessionError> {
        validate_session_id(id)?;
        Ok(self.agent_dir(agent_id)?.join(format!("{id}.json")))
    }

    pub fn upsert(&self, mut session: ChatSession) -> Result<ChatSession, SessionError> {
        parse_agent_ref(&session.agent_stem)?;
        if session.id.is_empty() {
            session.id = Uuid::new_v4().to_string();
        } else {
            validate_session_id(&session.id)?;
        }
        session.updated = Utc::now();
        if session.created.timestamp() == 0 {
            session.created = session.updated;
        }
        let dir = self.agent_dir(&session.agent_stem)?;
        fs::create_dir_all(&dir)?;
        let path = self.session_path(&session.agent_stem, &session.id)?;
        let text = serde_json::to_string_pretty(&session)?;
        fs::write(path, text)?;
        Ok(session)
    }

    pub fn get(&self, agent_id: &str, id: &str) -> Result<ChatSession, SessionError> {
        let path = self.session_path(agent_id, id)?;
        if !path.is_file() {
            return Err(SessionError::NotFound(id.to_string()));
        }
        let text = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&text)?)
    }

    pub fn list(&self, agent_id: Option<&str>) -> Result<Vec<SessionSummary>, SessionError> {
        self.ensure()?;
        let mut out = Vec::new();

        if let Some(agent_id) = agent_id {
            parse_agent_ref(agent_id)?;
            let dir = self.agent_dir(agent_id)?;
            if dir.is_dir() {
                collect_sessions(&dir, agent_id, &mut out)?;
            }
        } else if self.root.is_dir() {
            for cat_ent in fs::read_dir(&self.root)? {
                let cat_ent = cat_ent?;
                if !cat_ent.file_type()?.is_dir() {
                    continue;
                }
                let Some(cat) = cat_ent.file_name().to_str().map(|s| s.to_string()) else {
                    continue;
                };
                for name_ent in fs::read_dir(cat_ent.path())? {
                    let name_ent = name_ent?;
                    if !name_ent.file_type()?.is_dir() {
                        continue;
                    }
                    let Some(name) = name_ent.file_name().to_str().map(|s| s.to_string()) else {
                        continue;
                    };
                    let id = format!("{cat}/{name}");
                    if parse_agent_ref(&id).is_err() {
                        continue;
                    }
                    collect_sessions(&name_ent.path(), &id, &mut out)?;
                }
            }
        }

        out.sort_by(|a, b| b.updated.cmp(&a.updated));
        Ok(out)
    }

    pub fn append_messages(
        &self,
        agent_id: &str,
        id: &str,
        messages: Vec<SessionMessage>,
    ) -> Result<ChatSession, SessionError> {
        let mut session = match self.get(agent_id, id) {
            Ok(s) => s,
            Err(SessionError::NotFound(_)) => ChatSession {
                id: id.to_string(),
                agent_stem: agent_id.to_string(),
                created: Utc::now(),
                updated: Utc::now(),
                title: None,
                messages: Vec::new(),
            },
            Err(e) => return Err(e),
        };
        for mut m in messages {
            if m.ts.is_none() {
                m.ts = Some(Utc::now());
            }
            session.messages.push(m);
        }
        self.upsert(session)
    }

    pub fn search(
        &self,
        query: &str,
        agent_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MessageHit>, SessionError> {
        let q = query.trim().to_ascii_lowercase();
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let limit = limit.clamp(1, 200);
        let summaries = self.list(agent_id)?;
        let mut hits = Vec::new();

        for sum in summaries {
            if hits.len() >= limit {
                break;
            }
            let Ok(session) = self.get(&sum.agent_stem, &sum.id) else {
                continue;
            };
            for (idx, msg) in session.messages.iter().enumerate() {
                if hits.len() >= limit {
                    break;
                }
                if msg.content.to_ascii_lowercase().contains(&q) {
                    hits.push(MessageHit {
                        session_id: session.id.clone(),
                        agent_stem: session.agent_stem.clone(),
                        message_index: idx,
                        role: msg.role.clone(),
                        snippet: snippet(&msg.content, &q, 160),
                        updated: session.updated,
                    });
                }
            }
        }
        Ok(hits)
    }

    pub fn count_all(&self) -> Result<(usize, usize), SessionError> {
        let list = self.list(None)?;
        let msgs: usize = list.iter().map(|s| s.message_count).sum();
        Ok((list.len(), msgs))
    }
}

fn collect_sessions(
    dir: &Path,
    agent_id: &str,
    out: &mut Vec<SessionSummary>,
) -> Result<(), SessionError> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let session: ChatSession = match serde_json::from_str(&text) {
            Ok(s) => s,
            Err(_) => continue,
        };
        out.push(SessionSummary {
            id: session.id,
            agent_stem: agent_id.to_string(),
            created: session.created,
            updated: session.updated,
            title: session.title,
            message_count: session.messages.len(),
            path: path.display().to_string(),
        });
    }
    Ok(())
}

fn validate_session_id(id: &str) -> Result<(), SessionError> {
    if id.is_empty()
        || id.contains('/')
        || id.contains('\\')
        || id.contains("..")
        || id.contains('\0')
    {
        return Err(SessionError::InvalidId(id.to_string()));
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(SessionError::InvalidId(id.to_string()));
    }
    Ok(())
}

fn snippet(content: &str, query_lc: &str, max: usize) -> String {
    let lower = content.to_ascii_lowercase();
    let pos = lower.find(query_lc).unwrap_or(0);
    let start = pos.saturating_sub(40);
    let end = (pos + query_lc.len() + 80).min(content.len());
    let mut s = content[start..end].to_string();
    if start > 0 {
        s.insert_str(0, "…");
    }
    if end < content.len() {
        s.push('…');
    }
    if s.len() > max {
        s.truncate(max);
        s.push('…');
    }
    s
}
