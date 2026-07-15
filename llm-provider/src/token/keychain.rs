//! Claude Code OAuth token from the macOS Keychain. Reads the `Claude Code-credentials`
//! entry, refreshes at console.anthropic.com when near expiry, and writes the new token
//! back so Claude Code itself reuses it.

use serde_json::{json, Value};
use tokio::process::Command;
use tokio::sync::Mutex;

use super::{is_stale, now, Cache};
use crate::CLAUDE_OAUTH_CLIENT_ID;

async fn keychain_read() -> anyhow::Result<(Value, String)> {
    let secret = Command::new("security")
        .args(["find-generic-password", "-s", "Claude Code-credentials", "-w"])
        .output()
        .await?;
    anyhow::ensure!(
        secret.status.success(),
        "Keychain read failed (is Claude Code signed in?)"
    );
    let creds: Value = serde_json::from_slice(&secret.stdout)?;
    let attrs = Command::new("security")
        .args(["find-generic-password", "-s", "Claude Code-credentials"])
        .output()
        .await?;
    let text = String::from_utf8_lossy(&attrs.stdout).to_string();
    let account = text
        .split("\"acct\"<blob>=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or("")
        .to_string();
    Ok((creds, account))
}

async fn keychain_write(creds: &Value, account: &str) -> anyhow::Result<()> {
    let out = Command::new("security")
        .args([
            "add-generic-password",
            "-U",
            "-s",
            "Claude Code-credentials",
            "-a",
            account,
            "-w",
            &serde_json::to_string(creds)?,
        ])
        .output()
        .await?;
    anyhow::ensure!(out.status.success(), "Keychain write failed");
    Ok(())
}

pub async fn claude_token(http: &reqwest::Client, cache: &Mutex<Cache>) -> anyhow::Result<String> {
    let mut cache = cache.lock().await;
    if let Some(tok) = &cache.tok {
        if !is_stale(cache.exp, 60.0) {
            return Ok(tok.clone());
        }
    }
    let (mut creds, account) = keychain_read().await?;
    let oauth = &creds["claudeAiOauth"];
    let mut access = oauth["accessToken"].as_str().unwrap_or_default().to_string();
    // expiresAt is epoch millis; a missing/zero expiry stays 0.0 => is_stale => forced refresh.
    let mut exp = oauth["expiresAt"].as_f64().unwrap_or(0.0) / 1000.0;

    if is_stale(exp, 60.0) {
        let r = http
            .post("https://console.anthropic.com/v1/oauth/token")
            .json(&json!({
                "grant_type": "refresh_token",
                "refresh_token": oauth["refreshToken"],
                "client_id": CLAUDE_OAUTH_CLIENT_ID,
            }))
            .send()
            .await?;
        anyhow::ensure!(
            r.status().is_success(),
            "Claude token refresh failed: {}",
            r.text().await?
        );
        let d: Value = r.json().await?;
        access = d["access_token"].as_str().unwrap_or_default().to_string();
        exp = now() + d["expires_in"].as_f64().unwrap_or(3600.0);
        let o = &mut creds["claudeAiOauth"];
        o["accessToken"] = json!(access);
        if let Some(rt) = d["refresh_token"].as_str() {
            o["refreshToken"] = json!(rt);
        }
        o["expiresAt"] = json!((exp * 1000.0) as i64);
        keychain_write(&creds, &account).await?;
    }
    cache.tok = Some(access.clone());
    cache.exp = exp;
    Ok(access)
}
