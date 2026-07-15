//! Google sign-in (browser `/login`+`/oauth/callback` and headless `/auth/google`) plus the
//! xAI device-code CLI. All paths verify a Google/xAI identity, check email_verified +
//! allowlist, and mint a gateway key.

use std::collections::HashMap;
use std::time::Instant;

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use rand::distr::Alphanumeric;
use rand::Rng;
use serde_json::{json, Value};

use super::denial_message;
use super::keystore::KeyStore;
use crate::app::AppState;
use crate::config::Config;

pub async fn index(State(app): State<AppState>) -> Html<String> {
    Html(format!(
        "<h2>llm-provider</h2><p>OpenAI-compatible base URL: <code>{}/v1</code></p>\
         <p><a href='/login'>Sign in with Google to get an API key</a></p>",
        app.cfg.public_url
    ))
}

/// POST /auth/google — headless login with a Google ID token (e.g. from
/// `gcloud auth print-identity-token`, valid 1 hour).
/// ponytail: any Google-issued audience is accepted; pin allowed `aud` client ids here if
/// the gateway is ever exposed beyond LAN/tailscale.
pub async fn google_token(State(app): State<AppState>, headers: HeaderMap, body: axum::body::Bytes) -> Response {
    let from_body = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|v| v["id_token"].as_str().map(String::from));
    let token = from_body.or_else(|| {
        headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(|s| s.trim().to_string())
    });
    let Some(token) = token else {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(json!({"error": {"message": "Provide a Google ID token as JSON {\"id_token\": ...} or Authorization: Bearer.", "type": "invalid_request_error"}})),
        )
            .into_response();
    };
    let resp = match app
        .http
        .get(format!("https://oauth2.googleapis.com/tokeninfo?id_token={token}"))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                axum::Json(json!({"error": {"message": format!("tokeninfo unreachable: {e}"), "type": "api_error"}})),
            )
                .into_response()
        }
    };
    if !resp.status().is_success() {
        return (
            StatusCode::UNAUTHORIZED,
            axum::Json(json!({"error": {"message": "Invalid or expired Google ID token.", "type": "invalid_request_error"}})),
        )
            .into_response();
    }
    let info: Value = resp.json().await.unwrap_or_default();
    let email = info["email"].as_str().unwrap_or("").to_lowercase();
    let verified = info["email_verified"] == json!(true) || info["email_verified"].as_str() == Some("true");
    if !verified || !app.cfg.is_allowed(&email) {
        return (
            StatusCode::FORBIDDEN,
            axum::Json(json!({"error": {"message": denial_message(&app.cfg, &email), "type": "permission_error", "code": "email_not_allowed"},
                              "contact": app.cfg.admin_contact(), "login_url": format!("{}/login", app.cfg.public_url)})),
        )
            .into_response();
    }
    match app.keys.mint(&email) {
        Ok(key) => axum::Json(json!({"api_key": key, "email": email, "base_url": format!("{}/v1", app.cfg.public_url)})).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(json!({"error": {"message": format!("mint failed: {e}"), "type": "api_error"}})),
        )
            .into_response(),
    }
}

pub async fn login(State(app): State<AppState>) -> Response {
    if app.cfg.auth.google_client_id.is_empty() || app.cfg.auth.google_client_secret.is_empty() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html("Google sign-in is not configured. Set auth.google_client_id/google_client_secret in config.toml (or use POST /auth/google with a gcloud identity token)."),
        )
            .into_response();
    }
    let state: String = rand::rng().sample_iter(&Alphanumeric).take(32).map(char::from).collect();
    {
        let mut states = app.oauth_states.lock().await;
        states.retain(|_, t| t.elapsed().as_secs() < 600);
        states.insert(state.clone(), Instant::now());
    }
    let url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope=openid%20email&state={}&prompt=select_account",
        urlenc(&app.cfg.auth.google_client_id),
        urlenc(&format!("{}/oauth/callback", app.cfg.public_url)),
        state,
    );
    Redirect::temporary(&url).into_response()
}

pub async fn callback(State(app): State<AppState>, Query(q): Query<HashMap<String, String>>) -> Response {
    let (code, state) = (q.get("code"), q.get("state"));
    let state_ok = match state {
        Some(s) => app.oauth_states.lock().await.remove(s).is_some(),
        None => false,
    };
    if q.contains_key("error") || code.is_none() || !state_ok {
        let err = q.get("error").cloned().unwrap_or_else(|| "bad state".into());
        return (
            StatusCode::BAD_REQUEST,
            Html(format!("Sign-in failed: {err}. <a href='/login'>Try again</a>.")),
        )
            .into_response();
    }
    match exchange(&app, code.unwrap()).await {
        Ok(email_info) => {
            let email = email_info["email"].as_str().unwrap_or("").to_lowercase();
            let verified = email_info["email_verified"].as_bool().unwrap_or(false)
                || email_info["email_verified"].as_str() == Some("true");
            if !verified || !app.cfg.is_allowed(&email) {
                return (StatusCode::FORBIDDEN, Html(denial_message(&app.cfg, &email))).into_response();
            }
            match app.keys.mint(&email) {
                Ok(key) => Html(format!(
                    "<h2>Welcome, {email}</h2><p>Your API key (shown once — store it now):</p>\
                     <pre style='font-size:1.1em;background:#eee;padding:1em'>{key}</pre>\
                     <p>Base URL: <code>{}/v1</code> &nbsp; (OpenAI-compatible)</p>",
                    app.cfg.public_url
                ))
                .into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("Key mint failed: {e}"))).into_response(),
            }
        }
        Err(e) => (StatusCode::BAD_GATEWAY, Html(format!("Google sign-in failed: {e}"))).into_response(),
    }
}

async fn exchange(app: &AppState, code: &str) -> anyhow::Result<Value> {
    let r = app
        .http
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("code", code),
            ("client_id", &app.cfg.auth.google_client_id),
            ("client_secret", &app.cfg.auth.google_client_secret),
            ("redirect_uri", &format!("{}/oauth/callback", app.cfg.public_url)),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await?;
    if !r.status().is_success() {
        anyhow::bail!("token exchange failed: {}", r.text().await?);
    }
    let tok: Value = r.json().await?;
    let access = tok["access_token"].as_str().ok_or_else(|| anyhow::anyhow!("no access_token"))?;
    let info: Value = app
        .http
        .get("https://openidconnect.googleapis.com/v1/userinfo")
        .bearer_auth(access)
        .send()
        .await?
        .json()
        .await?;
    Ok(info)
}

fn urlenc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// `llm-provider login-xai`: xAI OIDC device-code flow, reusing the grok CLI's registered
/// client. Prints an allowlist-checked gateway key on success.
pub async fn login_xai(cfg: &Config) -> anyhow::Result<()> {
    let http = reqwest::Client::new();
    let grok: Value = serde_json::from_str(&std::fs::read_to_string(crate::config::grok_auth_file())?)?;
    let entry = grok
        .as_object()
        .and_then(|o| o.values().next())
        .ok_or_else(|| anyhow::anyhow!("no grok auth entry; run `grok` once to register the OIDC client"))?;
    let issuer = entry["oidc_issuer"].as_str().unwrap_or("https://auth.x.ai").trim_end_matches('/');
    let client_id = entry["oidc_client_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("no oidc_client_id in grok auth.json"))?;

    let disc: Value = http
        .get(format!("{issuer}/.well-known/openid-configuration"))
        .send()
        .await?
        .json()
        .await?;
    let device_ep = disc["device_authorization_endpoint"].as_str().unwrap();
    let token_ep = disc["token_endpoint"].as_str().unwrap();
    let userinfo_ep = disc["userinfo_endpoint"].as_str().unwrap();

    let dev: Value = http
        .post(device_ep)
        .form(&[("client_id", client_id), ("scope", "openid profile email")])
        .send()
        .await?
        .json()
        .await?;
    let device_code = dev["device_code"].as_str().ok_or_else(|| anyhow::anyhow!("device flow rejected: {dev}"))?;
    let user_code = dev["user_code"].as_str().unwrap_or("");
    let verify_url = dev["verification_uri_complete"]
        .as_str()
        .or(dev["verification_uri"].as_str())
        .unwrap_or("");
    let interval = dev["interval"].as_u64().unwrap_or(5).max(1);
    let expires_in = dev["expires_in"].as_u64().unwrap_or(900);

    println!("USER_CODE: {user_code}");
    println!("OPEN_THIS_URL: {verify_url}");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(expires_in.min(600));
    let tok: Value = loop {
        tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        anyhow::ensure!(std::time::Instant::now() < deadline, "sign-in timed out");
        let r: Value = http
            .post(token_ep)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", device_code),
                ("client_id", client_id),
            ])
            .send()
            .await?
            .json()
            .await?;
        if r["access_token"].is_string() {
            break r;
        }
        match r["error"].as_str().unwrap_or("") {
            "authorization_pending" | "slow_down" => continue,
            e => anyhow::bail!("sign-in failed: {e}"),
        }
    };
    let access = tok["access_token"].as_str().unwrap();

    let info: Value = http.get(userinfo_ep).bearer_auth(access).send().await?.json().await?;
    let email = info["email"].as_str().unwrap_or("").to_lowercase();
    if !cfg.is_allowed(&email) {
        anyhow::bail!("{}", denial_message(cfg, &email));
    }
    let key = KeyStore::new(crate::config::keys_file()).mint(&email)?;
    println!("SIGNED_IN: {email}");
    println!("API_KEY: {key}");
    Ok(())
}
