//! In-memory API-key store backed by keys.json. Keys are SHA256-hashed; the hash set is
//! cached and only re-read when the file's mtime changes (so a cross-process `mint` is still
//! seen, without re-parsing the file on every request). Mints are serialized and written
//! atomically (tmp + rename), and re-read the file first so concurrent/cross-process mints
//! never clobber each other.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};
use std::time::SystemTime;

use axum::http::HeaderMap;
use rand::distr::Alphanumeric;
use rand::Rng;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub fn sha256_hex(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

fn read_hashes(path: &Path) -> HashSet<String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.as_object().map(|o| o.keys().cloned().collect()))
        .unwrap_or_default()
}

fn mtime_of(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok().and_then(|m| m.modified().ok())
}

struct Cached {
    mtime: Option<SystemTime>,
    hashes: HashSet<String>,
}

pub struct KeyStore {
    path: PathBuf,
    cache: RwLock<Cached>,
    mint_lock: Mutex<()>,
}

impl KeyStore {
    pub fn new(path: PathBuf) -> Self {
        let cached = Cached {
            mtime: mtime_of(&path),
            hashes: read_hashes(&path),
        };
        KeyStore {
            path,
            cache: RwLock::new(cached),
            mint_lock: Mutex::new(()),
        }
    }

    /// Reload the hash set only if the file changed on disk (cheap stat, no parse on hit).
    fn refresh(&self) {
        let disk = mtime_of(&self.path);
        if self.cache.read().unwrap().mtime == disk {
            return;
        }
        let hashes = read_hashes(&self.path);
        let mut c = self.cache.write().unwrap();
        c.mtime = disk;
        c.hashes = hashes;
    }

    /// True if the request carries a valid minted key (Bearer or x-api-key).
    pub fn verify(&self, headers: &HeaderMap) -> bool {
        let bearer = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(str::trim);
        let key = bearer.or_else(|| headers.get("x-api-key").and_then(|v| v.to_str().ok()));
        match key {
            Some(k) if !k.is_empty() => {
                self.refresh();
                self.cache.read().unwrap().hashes.contains(&sha256_hex(k))
            }
            _ => false,
        }
    }

    /// Mint a new key, persist atomically, and update the cache. Serialized across callers;
    /// re-reads the file first so a concurrent mint (or `llm-provider mint` in another
    /// process) is not overwritten.
    pub fn mint(&self, email: &str) -> anyhow::Result<String> {
        let _g = self.mint_lock.lock().unwrap();
        let suffix: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(43)
            .map(char::from)
            .collect();
        let key = format!("llmgw-{suffix}");
        let mut keys: Value = std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| json!({}));
        keys[sha256_hex(&key)] = json!({
            "email": email,
            "created": chrono::Utc::now().to_rfc3339(),
        });
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(&keys)?)?;
        std::fs::rename(&tmp, &self.path)?;
        let mut c = self.cache.write().unwrap();
        c.mtime = mtime_of(&self.path);
        c.hashes = keys
            .as_object()
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default();
        Ok(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mint_then_verify_roundtrips_and_rejects_bogus() {
        let path = std::env::temp_dir().join(format!("llmprov-keys-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let store = KeyStore::new(path.clone());

        let mut good = HeaderMap::new();
        let key = store.mint("tyler@example.com").unwrap();
        good.insert("authorization", format!("Bearer {key}").parse().unwrap());
        assert!(store.verify(&good), "minted key must verify");

        let mut bogus = HeaderMap::new();
        bogus.insert("authorization", "Bearer llmgw-nope".parse().unwrap());
        assert!(!store.verify(&bogus), "unknown key must be rejected");

        assert!(!store.verify(&HeaderMap::new()), "no key must be rejected");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn second_mint_preserves_first() {
        let path = std::env::temp_dir().join(format!("llmprov-keys2-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let store = KeyStore::new(path.clone());
        let k1 = store.mint("a@example.com").unwrap();
        let _k2 = store.mint("b@example.com").unwrap();
        let mut h = HeaderMap::new();
        h.insert("authorization", format!("Bearer {k1}").parse().unwrap());
        assert!(store.verify(&h), "first key must survive a second mint");
        std::fs::remove_file(&path).ok();
    }
}
