//! Terminal MCP Server entry point.

use std::sync::Arc;

use anyhow::Result;
use rmcp::{transport::stdio, ServiceExt};
use terminal::Server;
use tracing_subscriber::{self, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    // Logging to stderr only (stdout is reserved for MCP protocol)
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive(tracing::Level::DEBUG.into()),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("Starting Terminal MCP Server");

    // Create server
    let server = Server::new();
    let server_for_shutdown = server.clone();

    // Set up graceful shutdown
    let shutdown = Arc::new(tokio::sync::Notify::new());
    let shutdown_clone = shutdown.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        tracing::info!("Received shutdown signal");
        shutdown_clone.notify_one();
    });

    // Run the server
    let service = server.serve(stdio()).await?;

    tokio::select! {
        result = service.waiting() => {
            result?;
        }
        _ = shutdown.notified() => {
            tracing::info!("Shutting down, cleaning up sessions");
            server_for_shutdown.shutdown().await;
        }
    }

    tracing::info!("Terminal MCP Server stopped");
    Ok(())
}
