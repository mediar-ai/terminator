use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use crate::utils::{init_logging, DesktopWrapper};

pub mod utils;
pub mod server;

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

