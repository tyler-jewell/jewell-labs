//! Credential store backed by keys.json. We issue OAuth-style credentials — a short-lived
//! **access token** (presented as Bearer on every request) paired with a longer-lived
//! **refresh token** that rotates on each use — so no issued token lives forever.
//!
//! keys.json is a map keyed by `sha256(access_token)`:
//!   { "<access_sha>": { email, created, access_expires_at, refresh_sha, refresh_expires_at } }
//!
//! The records are cached in memory and only re-read when the file's mtime changes (so a
//! cross-process `mint`/`refresh` is still seen without re-parsing on every request). Writes
//! are serialized and atomic (tmp + rename), re-reading the file first so concurrent /
//! cross-process mutations never clobber each other.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};
use std::time::SystemTime;

use axum::http::HeaderMap;
use rand::distr::Alphanumeric;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::token::now;

pub fn sha256_hex(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

/// One issued credential. Legacy bare entries (`{email, created}`) deserialize with the new
/// numeric fields defaulting to 0, i.e. already-expired — so they verify as invalid without a
/// special migration path.
#[derive(Clone, Serialize, Deserialize)]
struct Record {
    email: String,
    #[serde(default)]
    created: String,
    #[serde(default)]
    access_expires_at: f64, // unix seconds
    #[serde(default)]
    refresh_sha: String,
    #[serde(default)]
    refresh_expires_at: f64, // unix seconds
}

/// A freshly minted or refreshed credential pair returned to the caller.
pub struct Bundle {
    pub access_token: String,
    pub refresh_token: String,
    pub access_expires_at: f64, // unix seconds
}

fn read_records(path: &Path) -> HashMap<String, Record> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<HashMap<String, Record>>(&s).ok())
        .unwrap_or_default()
}

fn write_records(path: &Path, records: &HashMap<String, Record>) -> anyhow::Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(records)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Drop records whose refresh token has expired (they can no longer be renewed).
fn prune_expired(records: &mut HashMap<String, Record>, now_secs: f64) {
    records.retain(|_, r| r.refresh_expires_at > now_secs);
}

fn rand_suffix() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(43)
        .map(char::from)
        .collect()
}

fn mtime_of(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok().and_then(|m| m.modified().ok())
}

struct Cached {
    mtime: Option<SystemTime>,
    records: HashMap<String, Record>,
}

pub struct KeyStore {
    path: PathBuf,
    access_ttl: u64,
    refresh_ttl: u64,
    cache: RwLock<Cached>,
    write_lock: Mutex<()>,
}

impl KeyStore {
    pub fn new(path: PathBuf, access_ttl_secs: u64, refresh_ttl_secs: u64) -> Self {
        let cached = Cached {
            mtime: mtime_of(&path),
            records: read_records(&path),
        };
        KeyStore {
            path,
            access_ttl: access_ttl_secs,
            refresh_ttl: refresh_ttl_secs,
            cache: RwLock::new(cached),
            write_lock: Mutex::new(()),
        }
    }

    /// Reload the records only if the file changed on disk (cheap stat, no parse on hit).
    fn reload(&self) {
        let disk = mtime_of(&self.path);
        if self.cache.read().unwrap().mtime == disk {
            return;
        }
        let records = read_records(&self.path);
        let mut c = self.cache.write().unwrap();
        c.mtime = disk;
        c.records = records;
    }

    fn store_cache(&self, records: HashMap<String, Record>) {
        let mut c = self.cache.write().unwrap();
        c.mtime = mtime_of(&self.path);
        c.records = records;
    }

    /// True if the request carries a valid, unexpired access token (Bearer or x-api-key).
    pub fn verify(&self, headers: &HeaderMap) -> bool {
        let bearer = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(str::trim);
        let key = bearer.or_else(|| headers.get("x-api-key").and_then(|v| v.to_str().ok()));
        match key {
            Some(k) if !k.is_empty() => {
                self.reload();
                let c = self.cache.read().unwrap();
                match c.records.get(&sha256_hex(k)) {
                    Some(rec) => now() < rec.access_expires_at,
                    None => false,
                }
            }
            _ => false,
        }
    }

    /// Mint a fresh access+refresh pair for `email`, persist atomically, update the cache.
    /// Serialized; re-reads the file first so a concurrent mint/refresh is not overwritten.
    pub fn mint(&self, email: &str) -> anyhow::Result<Bundle> {
        let _g = self.write_lock.lock().unwrap();
        let mut records = read_records(&self.path);
        let bundle = self.insert_pair(&mut records, email);
        prune_expired(&mut records, now());
        write_records(&self.path, &records)?;
        self.store_cache(records);
        Ok(bundle)
    }

    /// Exchange a refresh token for a new pair. The presented refresh token (and its access
    /// token) are invalidated — rotating refresh, exactly like Claude/grok. Errors if the
    /// refresh token is unknown or expired.
    pub fn refresh(&self, refresh_token: &str) -> anyhow::Result<Bundle> {
        let _g = self.write_lock.lock().unwrap();
        let now_secs = now();
        let rsha = sha256_hex(refresh_token);
        let mut records = read_records(&self.path);
        let found = records
            .iter()
            .find(|(_, r)| r.refresh_sha == rsha && now_secs < r.refresh_expires_at)
            .map(|(k, r)| (k.clone(), r.email.clone()));
        let Some((old_access_sha, email)) = found else {
            anyhow::bail!("invalid or expired refresh token");
        };
        records.remove(&old_access_sha); // invalidate the old access + refresh
        let bundle = self.insert_pair(&mut records, &email);
        prune_expired(&mut records, now_secs);
        write_records(&self.path, &records)?;
        self.store_cache(records);
        Ok(bundle)
    }

    /// Generate a new access+refresh pair, insert it into `records`, and return the bundle.
    fn insert_pair(&self, records: &mut HashMap<String, Record>, email: &str) -> Bundle {
        let now_secs = now();
        let access = format!("llmgw-{}", rand_suffix());
        let refresh = format!("llmgwr-{}", rand_suffix());
        let access_expires_at = now_secs + self.access_ttl as f64;
        records.insert(
            sha256_hex(&access),
            Record {
                email: email.to_string(),
                created: chrono::Utc::now().to_rfc3339(),
                access_expires_at,
                refresh_sha: sha256_hex(&refresh),
                refresh_expires_at: now_secs + self.refresh_ttl as f64,
            },
        );
        Bundle {
            access_token: access,
            refresh_token: refresh,
            access_expires_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn store(access_ttl: u64, refresh_ttl: u64) -> (KeyStore, PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "llmprov-keys-{}-{}.json",
            std::process::id(),
            rand::random::<u32>()
        ));
        let _ = std::fs::remove_file(&path);
        (KeyStore::new(path.clone(), access_ttl, refresh_ttl), path)
    }

    fn bearer(k: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert("authorization", format!("Bearer {k}").parse().unwrap());
        h
    }

    #[test]
    fn mint_then_verify_roundtrips_and_rejects_bogus() {
        let (s, path) = store(3600, 86400);
        let b = s.mint("tyler@example.com").unwrap();
        assert!(s.verify(&bearer(&b.access_token)), "minted access token must verify");
        assert!(!s.verify(&bearer("llmgw-nope")), "unknown key must be rejected");
        assert!(!s.verify(&HeaderMap::new()), "no key must be rejected");
        // the refresh token is NOT an access token
        assert!(!s.verify(&bearer(&b.refresh_token)), "refresh token must not verify as access");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn expired_access_token_is_rejected() {
        let (s, path) = store(3600, 86400);
        // hand-write an already-expired record so the test is deterministic (no sleeps).
        let access = "llmgw-expired";
        let rec = json!({ sha256_hex(access): {
            "email": "x@example.com", "created": "",
            "access_expires_at": now() - 10.0,
            "refresh_sha": "", "refresh_expires_at": now() - 10.0,
        }});
        std::fs::write(&path, serde_json::to_string(&rec).unwrap()).unwrap();
        assert!(!s.verify(&bearer(access)), "expired access token must be rejected");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn refresh_rotates_and_invalidates_old_tokens() {
        let (s, path) = store(3600, 86400);
        let first = s.mint("a@example.com").unwrap();
        let second = s.refresh(&first.refresh_token).unwrap();

        // new access verifies; old access no longer does (rotated out)
        assert!(s.verify(&bearer(&second.access_token)), "new access must verify");
        assert!(!s.verify(&bearer(&first.access_token)), "old access must be invalidated");
        // old refresh token can't be reused
        assert!(s.refresh(&first.refresh_token).is_err(), "old refresh must be rejected");
        // new refresh works and rotates again
        let third = s.refresh(&second.refresh_token).unwrap();
        assert!(s.verify(&bearer(&third.access_token)));
        assert!(!s.verify(&bearer(&second.access_token)));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn expired_refresh_token_is_rejected() {
        let (s, path) = store(3600, 86400);
        let access = "llmgw-x";
        let refresh = "llmgwr-old";
        let rec = json!({ sha256_hex(access): {
            "email": "x@example.com", "created": "",
            "access_expires_at": now() + 3600.0,
            "refresh_sha": sha256_hex(refresh), "refresh_expires_at": now() - 1.0,
        }});
        std::fs::write(&path, serde_json::to_string(&rec).unwrap()).unwrap();
        assert!(s.refresh(refresh).is_err(), "expired refresh must be rejected");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn second_mint_preserves_first() {
        let (s, path) = store(3600, 86400);
        let k1 = s.mint("a@example.com").unwrap();
        let _k2 = s.mint("b@example.com").unwrap();
        assert!(s.verify(&bearer(&k1.access_token)), "first key must survive a second mint");
        std::fs::remove_file(&path).ok();
    }
}
