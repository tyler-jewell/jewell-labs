//! CLI entrypoint: `mint <email>` and `login-xai` admin subcommands, else serve.

use llm_provider::config::{self, Config};
use llm_provider::identity::{self, oauth_google, KeyStore};
use llm_provider::{build_app, serve};

#[tokio::main]
async fn main() {
    let cfg = Config::load();
    let args: Vec<String> = std::env::args().collect();
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
            let key = KeyStore::new(config::keys_file()).mint(&email).expect("mint failed");
            println!("{key}");
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
