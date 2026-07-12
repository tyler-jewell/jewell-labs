//! Server-side chat session storage for agent introspection.
//! Layout: `{root}/{category}/{name}/{session_id}.json`

mod store;
mod types;

pub use store::SessionStore;
pub use types::{
    ChatSession, MessageHit, SessionError, SessionMessage, SessionSummary,
};

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tempfile::tempdir;

    #[test]
    fn upsert_list_get_search_nested() {
        let dir = tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        let s = store
            .upsert(ChatSession {
                id: "sess-1".into(),
                agent_stem: "tutoring/math-tutor".into(),
                created: Utc::now(),
                updated: Utc::now(),
                title: Some("pi".into()),
                messages: vec![SessionMessage {
                    role: "user".into(),
                    content: "What is pi approximately?".into(),
                    ts: Some(Utc::now()),
                }],
            })
            .unwrap();
        assert_eq!(s.id, "sess-1");
        assert!(dir
            .path()
            .join("tutoring/math-tutor/sess-1.json")
            .is_file());

        let list = store.list(Some("tutoring/math-tutor")).unwrap();
        assert_eq!(list.len(), 1);

        let hits = store
            .search("pi", Some("tutoring/math-tutor"), 10)
            .unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn rejects_bad_agent_id() {
        let dir = tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        let err = store
            .upsert(ChatSession {
                id: "x".into(),
                agent_stem: "../evil".into(),
                created: Utc::now(),
                updated: Utc::now(),
                title: None,
                messages: vec![],
            })
            .unwrap_err();
        assert!(matches!(err, SessionError::Stem(_)));
    }
}
