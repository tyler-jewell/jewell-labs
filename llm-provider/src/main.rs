//! CLI entrypoint: `mint <email>` and `login-xai` admin subcommands, else serve.

use llm_provider::config::{self, Config};
use llm_provider::identity::{self, oauth_google, KeyStore};
use llm_provider::{build_app, serve};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    // `expect` drives interactive commands via a PTY; it needs no config (and must work even
    // if config.toml is broken), so dispatch it before loading anything.
    if args.get(1).map(String::as_str) == Some("expect") {
        std::process::exit(llm_provider::interactive::run(&args[2..]));
    }
    let cfg = Config::load();
    match args.get(1).map(String::as_str) {
        Some("mint") => {
            let email = args
                .get(2)
                .expect("usage: llm-provider mint <email>")
                .to_lowercase();
            if !cfg.is_allowed(&email) {
                eprintln!("{}", identity::denial_message(&cfg, &email));
                std::process::exit(1);
            }
            let store = KeyStore::new(
                config::keys_file(),
                cfg.auth.access_ttl_secs,
                cfg.auth.refresh_ttl_secs,
            );
            let b = store.mint(&email).expect("mint failed");
            println!(
                "{}",
                serde_json::json!({
                    "access_token": b.access_token,
                    "refresh_token": b.refresh_token,
                    "expires_at": b.access_expires_at,
                })
            );
        }
        Some("login-xai") => {
            if let Err(e) = oauth_google::login_xai(&cfg).await {
                eprintln!("login failed: {e}");
                std::process::exit(1);
            }
        }
        _ => {
            let port = cfg.port;
            let app = build_app(cfg);
            let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await.unwrap();
            println!("llm-provider listening on 0.0.0.0:{port}");
            serve(app, listener).await.unwrap();
        }
    }
}
