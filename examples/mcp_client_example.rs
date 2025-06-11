use anyhow::Result;
use rmcp::{
    transport::TokioChildProcess, 
    ServiceExt
};
use std::env;
use tokio::process::Command;
use tracing::{info, error};

/// Example demonstrating how to use MCP client to connect to terminator-mcp-agent
/// and interact with UI automation tools.
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("mcp_client_example=info,rmcp=debug")
        .init();

    info!("Starting MCP Client Example");

    // Build the path to the terminator-mcp-agent binary
    let agent_path = env::current_dir()?
        .join("target")
        .join("release")
        .join("terminator-mcp-agent");

    info!("Looking for terminator-mcp-agent at: {}", agent_path.display());

    // Check if the binary exists
    if !agent_path.exists() {
        error!("terminator-mcp-agent binary not found at: {}", agent_path.display());
        error!("Please build it first with: cargo build --release --bin terminator-mcp-agent");
        return Err(anyhow::anyhow!("MCP agent binary not found"));
    }

    // Create a command to spawn the MCP agent
    let mut cmd = Command::new(&agent_path);
    
    info!("Spawning terminator-mcp-agent process...");

    // Create the MCP client by connecting to the agent via stdio
    let _client = ()
        .serve(TokioChildProcess::new(&mut cmd)?)
        .await
        .map_err(|e| {
            error!("Failed to connect to MCP agent: {:?}", e);
            anyhow::anyhow!("Failed to establish MCP connection: {}", e)
        })?;

    info!("✅ Successfully connected to terminator-mcp-agent via MCP!");

    // Demonstrate basic MCP interaction
    info!("🔧 The MCP client is connected and ready!");
    info!("The client can now interact with the terminator-mcp-agent");
    info!("This demonstrates the basic connection setup for MCP in Rust");

    // Keep the connection alive for a short time to demonstrate
    info!("Keeping connection alive for 5 seconds...");
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    info!("🎉 MCP Client example completed successfully!");
    info!("In a real application, you would use the client to:");
    info!("  - List available tools on the server");
    info!("  - Call specific tools with parameters");
    info!("  - Handle tool responses and errors");
    info!("  - Manage the connection lifecycle");

    Ok(())
}