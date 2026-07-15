//! llm-provider: one OpenAI-compatible API for local llama.cpp/ollama + Grok + Claude.
//!
//! Loopback traffic is trusted (local tools on this Mac); remote traffic needs an
//! OAuth-minted key. Providers are pluggable behind the `Provider` trait — adding one
//! is a new `providers/<x>.rs` module plus one arm in `registry::from_config`.

pub mod app;
pub mod config;
pub mod identity;
pub mod openai;
pub mod providers;
pub mod registry;
pub mod server;
pub mod token;

pub const CLAUDE_OAUTH_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
pub const CLAUDE_CODE_SYSTEM: &str = "You are Claude Code, Anthropic's official CLI for Claude.";

pub use app::AppState;
pub use config::Config;

/// Build shared state (HTTP client + provider registry) from a loaded config.
pub fn build_app(cfg: Config) -> AppState {
    AppState::new(cfg)
}

/// Serve until the process is killed. Uses `into_make_service_with_connect_info` so the
/// loopback-bypass middleware can see the peer socket address.
pub async fn serve(app: AppState, listener: tokio::net::TcpListener) -> anyhow::Result<()> {
    let router = server::router(app);
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}
