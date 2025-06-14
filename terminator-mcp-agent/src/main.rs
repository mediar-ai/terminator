use crate::utils::{init_logging, DesktopWrapper};
use anyhow::Result;
use rmcp::{transport::stdio, ServiceExt};

pub mod server;
pub mod utils;

#[tokio::main]
async fn main() -> Result<()> {
    init_logging()?;

    tracing::info!("Initializing Terminator MCP server...");

    let service = DesktopWrapper::new()
        .await?
        .serve(stdio())
        .await
        .inspect_err(|e| {
            tracing::error!("Serving error: {:?}", e);
        })?;

    service.waiting().await?;
    Ok(())
}
