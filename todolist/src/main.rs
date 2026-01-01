use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use todolist::Server;
use tracing_subscriber::{self, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::DEBUG.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let service = Server::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
