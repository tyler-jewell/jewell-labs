//! End-to-end OAuth + inference test. Boots the provider in-process on an ephemeral port,
//! mints REAL temp Google identity tokens via gcloud (allowed = tyler; denied = an
//! impersonated non-allowlisted service account), and drives the full flow.
//!
//! Runnable any time on this Mac:  cargo test --test oauth_integration -- --nocapture
//! Skips gracefully (does not fail) if gcloud isn't available/authed.
//!
//! The remote-without-key -> 401 path is covered by the pure unit tests in
//! `identity::tests` (is_authorized), since an in-process client is always loopback.

use serde_json::{json, Value};

fn gcloud_token(args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("gcloud").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let t = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!t.is_empty()).then_some(t)
}

async fn wait_healthy(http: &reqwest::Client, base: &str) {
    for _ in 0..50 {
        if let Ok(r) = http.get(format!("{base}/healthz")).send().await {
            if r.status().is_success() {
                return;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("server did not become healthy");
}

#[tokio::test]
async fn oauth_allowed_denied_and_inference() {
    // Isolate keys.json into a temp dir so the test never touches the real key store.
    let dir = std::env::temp_dir().join(format!("llmprov-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::env::set_var("LLM_PROVIDER_DIR", &dir);

    let mut cfg = llm_provider::Config::default();
    cfg.finalize();
    let app = llm_provider::build_app(cfg);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { llm_provider::serve(app, listener).await.unwrap() });

    let base = format!("http://{addr}");
    let http = reqwest::Client::new();
    wait_healthy(&http, &base).await;

    // 1. Loopback bypass: /v1/models with NO key -> 200 and includes providers.
    let r = http.get(format!("{base}/v1/models")).send().await.unwrap();
    assert_eq!(r.status(), 200, "loopback should bypass auth");
    let models: Value = r.json().await.unwrap();
    assert!(
        models["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["owned_by"] == "anthropic"),
        "claude models should be listed"
    );

    // 1b. Exposed mode + full credential lifecycle: a second instance with trust_loopback=false
    //     requires a key even on loopback (the safe posture behind a reverse tunnel). Exercise
    //     mint -> use -> refresh(rotate) -> old-token-invalid -> expiry, proving no token is permanent.
    {
        let mut exposed = llm_provider::Config::default();
        exposed.auth.trust_loopback = false;
        exposed.finalize();
        let app2 = llm_provider::build_app(exposed);
        let l2 = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr2 = l2.local_addr().unwrap();
        tokio::spawn(async move { llm_provider::serve(app2, l2).await.unwrap() });
        let base2 = format!("http://{addr2}");
        wait_healthy(&http, &base2).await;

        // mint an access+refresh bundle directly against the shared key store.
        let b1 = llm_provider::identity::KeyStore::new(llm_provider::config::keys_file(), 3600, 86400)
            .mint("local-test@example.com")
            .unwrap();
        assert!(b1.access_token.starts_with("llmgw-"));
        assert!(b1.refresh_token.starts_with("llmgwr-"));

        // keyless -> 401; valid access -> 200.
        let no_key = http.get(format!("{base2}/v1/models")).send().await.unwrap();
        assert_eq!(no_key.status(), 401, "trust_loopback=false must reject keyless loopback");
        let ok = http.get(format!("{base2}/v1/models")).bearer_auth(&b1.access_token).send().await.unwrap();
        assert_eq!(ok.status(), 200, "a valid access token must pass");

        // refresh rotates: a new bundle works; the old access + old refresh are invalidated.
        let rr = http.post(format!("{base2}/auth/refresh"))
            .json(&json!({"refresh_token": b1.refresh_token})).send().await.unwrap();
        assert_eq!(rr.status(), 200, "refresh must succeed");
        let b2: Value = rr.json().await.unwrap();
        let access2 = b2["access_token"].as_str().unwrap().to_string();
        assert!(b2["refresh_token"].as_str().unwrap().starts_with("llmgwr-"));
        assert!(b2["expires_at"].as_f64().is_some());

        let old = http.get(format!("{base2}/v1/models")).bearer_auth(&b1.access_token).send().await.unwrap();
        assert_eq!(old.status(), 401, "old access must be invalidated after rotation");
        let reuse = http.post(format!("{base2}/auth/refresh"))
            .json(&json!({"refresh_token": b1.refresh_token})).send().await.unwrap();
        assert_eq!(reuse.status(), 401, "a used refresh token must not be reusable");
        let newok = http.get(format!("{base2}/v1/models")).bearer_auth(&access2).send().await.unwrap();
        assert_eq!(newok.status(), 200, "new access token must pass");
        let bogus = http.post(format!("{base2}/auth/refresh"))
            .json(&json!({"refresh_token": "llmgwr-bogus"})).send().await.unwrap();
        assert_eq!(bogus.status(), 401, "bogus refresh must be rejected");

        // expiry: a bundle minted with access_ttl=0 is already expired -> 401.
        let expd = llm_provider::identity::KeyStore::new(llm_provider::config::keys_file(), 0, 86400)
            .mint("exp@example.com").unwrap();
        let er = http.get(format!("{base2}/v1/models")).bearer_auth(&expd.access_token).send().await.unwrap();
        assert_eq!(er.status(), 401, "expired access token must be rejected");
    }

    // 2. Denied path: non-allowlisted impersonated SA token -> 403 email_not_allowed.
    let Some(deny) = gcloud_token(&[
        "auth",
        "print-identity-token",
        "--impersonate-service-account=test-notallowed@dev-flag-seeker.iam.gserviceaccount.com",
        "--audiences=llm-gateway",
        "--include-email",
    ]) else {
        eprintln!("SKIP: gcloud not available / not authed; skipping OAuth assertions");
        std::fs::remove_dir_all(&dir).ok();
        return;
    };
    let r = http
        .post(format!("{base}/auth/google"))
        .json(&json!({"id_token": deny}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403, "non-allowlisted token must be denied");
    let b: Value = r.json().await.unwrap();
    assert_eq!(b["error"]["code"], "email_not_allowed");
    assert_eq!(b["contact"], "tyler.p.jewell@gmail.com");
    assert!(b["login_url"].as_str().unwrap().ends_with("/login"));

    // 3. Allowed path: tyler's identity token -> 200 + minted access+refresh bundle.
    let allow = gcloud_token(&["auth", "print-identity-token"]).expect("allowed token");
    let r = http
        .post(format!("{base}/auth/google"))
        .json(&json!({"id_token": allow}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "allowlisted token must mint a bundle");
    let b: Value = r.json().await.unwrap();
    let key = b["access_token"].as_str().unwrap().to_string();
    assert!(key.starts_with("llmgw-"));
    assert!(b["refresh_token"].as_str().unwrap().starts_with("llmgwr-"));
    assert!(b["expires_at"].as_f64().is_some());
    assert_eq!(b["email"], "tyler.p.jewell@gmail.com");

    // 4. Use the minted key to run grok inference through the gateway.
    let r = http
        .post(format!("{base}/v1/chat/completions"))
        .bearer_auth(&key)
        .json(&json!({"model": "grok-4.20-0309-non-reasoning", "max_tokens": 12,
                      "messages": [{"role": "user", "content": "Reply with exactly: itest ok"}]}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "grok inference should succeed with a valid key");
    let b: Value = r.json().await.unwrap();
    let content = b["choices"][0]["message"]["content"].as_str().unwrap_or("");
    assert!(
        content.to_lowercase().contains("itest ok"),
        "unexpected grok reply: {content}"
    );

    // 5. Best-effort local inference (skips if no local backend is up).
    let r = http
        .post(format!("{base}/v1/chat/completions"))
        .bearer_auth(&key)
        .json(&json!({"model": "llama3.2:1b", "max_tokens": 12,
                      "messages": [{"role": "user", "content": "Say hello."}]}))
        .send()
        .await
        .unwrap();
    if r.status() == 200 {
        let b: Value = r.json().await.unwrap();
        assert!(b["choices"][0]["message"]["content"].as_str().is_some());
    } else {
        eprintln!("NOTE: local model unreachable ({}), skipping local assertion", r.status());
    }

    std::fs::remove_dir_all(&dir).ok();
}
