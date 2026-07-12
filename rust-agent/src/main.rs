//! Jewell Labs agent console binary entrypoint.

use rust_agent::server::{build_router, default_state};
use rust_agent::{builtin_tools, sessions_dir};
use std::net::SocketAddr;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rust_agent=info,tower_http=info".into()),
        )
        .init();

    let sessions = sessions_dir();
    std::fs::create_dir_all(&sessions)?;

    let state = default_state();
    info!(
        crate_root = %state.crate_root.display(),
        agents = %state.agents_dir.display(),
        sessions = %state.sessions_dir.display(),
        tools = builtin_tools().len(),
        "starting rust-agent"
    );

    let app = build_router(state);
    let port: u16 = std::env::var("RUST_AGENT_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    info!("listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
