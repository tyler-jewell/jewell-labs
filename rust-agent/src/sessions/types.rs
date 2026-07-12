//! Session data types.

use crate::agents::AgentsError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid agent id: {0}")]
    Stem(#[from] AgentsError),
    #[error("session not found: {0}")]
    NotFound(String),
    #[error("invalid session id: {0}")]
    InvalidId(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ts: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: String,
    /// Agent id `category/name`
    pub agent_stem: String,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub messages: Vec<SessionMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub agent_stem: String,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    pub title: Option<String>,
    pub message_count: usize,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHit {
    pub session_id: String,
    pub agent_stem: String,
    pub message_index: usize,
    pub role: String,
    pub snippet: String,
    pub updated: DateTime<Utc>,
}
