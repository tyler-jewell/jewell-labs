//! grok CLI OIDC token from ~/.grok/auth.json, refreshed via the CLI's own OIDC issuer.
//! `XAI_API_KEY` short-circuits everything.

use std::path::Path;

use serde_json::{json, Value};
use tokio::sync::Mutex;

use super::{is_stale, now};

fn parse_iso(s: &str) -> f64 {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|t| t.timestamp() as f64)
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f")
                .map(|t| t.and_utc().timestamp() as f64)
        })
        .unwrap_or(0.0)
}

pub async fn grok_token(
    http: &reqwest::Client,
    lock: &Mutex<()>,
    auth_file: &Path,
) -> anyhow::Result<String> {
    if let Ok(k) = std::env::var("XAI_API_KEY") {
        return Ok(k);
    }
    let _guard = lock.lock().await; // serialize refreshes
    let mut data: Value = serde_json::from_str(&std::fs::read_to_string(auth_file)?)?;
    let slot = data
        .as_object()
        .and_then(|o| o.keys().next().cloned())
        .ok_or_else(|| anyhow::anyhow!("empty grok auth.json"))?;
    let entry = &data[&slot];
    // a missing/unparseable expires_at parses to 0.0 => is_stale => forced refresh.
    let exp = parse_iso(entry["expires_at"].as_str().unwrap_or(""));
    if !is_stale(exp, 120.0) {
        return Ok(entry["key"].as_str().unwrap_or_default().to_string());
    }
    let issuer = entry["oidc_issuer"]
        .as_str()
        .unwrap_or("https://auth.x.ai")
        .trim_end_matches('/');
    let disc: Value = http
        .get(format!("{issuer}/.well-known/openid-configuration"))
        .send()
        .await?
        .json()
        .await?;
    let token_endpoint = disc["token_endpoint"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("no token_endpoint"))?;
    let r = http
        .post(token_endpoint)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", entry["refresh_token"].as_str().unwrap_or("")),
            ("client_id", entry["oidc_client_id"].as_str().unwrap_or("")),
        ])
        .send()
        .await?;
    anyhow::ensure!(
        r.status().is_success(),
        "grok token refresh failed: {}",
        r.text().await?
    );
    let d: Value = r.json().await?;
    let key = d["access_token"].as_str().unwrap_or_default().to_string();
    let e = &mut data[&slot];
    e["key"] = json!(key);
    if let Some(rt) = d["refresh_token"].as_str() {
        e["refresh_token"] = json!(rt);
    }
    let new_exp = now() + d["expires_in"].as_f64().unwrap_or(3600.0);
    e["expires_at"] = json!(chrono::DateTime::from_timestamp(new_exp as i64, 0)
        .unwrap()
        .to_rfc3339());
    let tmp = auth_file.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string(&data)?)?;
    std::fs::rename(&tmp, auth_file)?;
    Ok(key)
}
