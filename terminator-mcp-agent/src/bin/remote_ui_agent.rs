// Remote UI Automation Agent
// This runs on the target VM to receive and execute UI automation commands

use anyhow::Result;
use axum::{
    extract::{Query, Json},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use clap::Parser;
use std::sync::Arc;
use terminator::Desktop;
use terminator_mcp_agent::{
    remote_server::{start_remote_server, RemoteServerState},
    utils::DesktopWrapper,
    cancellation::RequestManager,
};
use tokio::sync::Mutex;
use tracing::{info, error};

#[derive(Parser, Debug)]
#[clap(name = "remote-ui-agent")]
#[clap(about = "Remote UI Automation Agent for Windows", long_about = None)]
struct Args {
    /// Port to listen on
    #[clap(short, long, default_value = "8080")]
    port: u16,

    /// API key for authentication (optional)
    #[clap(short, long)]
    api_key: Option<String>,

    /// Enable verbose logging
    #[clap(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .init();

    info!("Starting Remote UI Automation Agent on port {}", args.port);

    // Set API key if provided
    if let Some(api_key) = args.api_key {
        std::env::set_var("REMOTE_API_KEY", api_key);
        info!("API key authentication enabled");
    }

    // Initialize desktop automation
    let desktop = Desktop::new(false, false)?;

    // Create DesktopWrapper with required fields
    let desktop_wrapper = DesktopWrapper {
        desktop: Arc::new(desktop),
        tool_router: rmcp::handler::server::tool::ToolRouter::new(),
        request_manager: Default::default(),
        recorder: Arc::new(Mutex::new(None)),
        active_highlights: Arc::new(Mutex::new(Vec::new())),
    };
    let desktop_arc = Arc::new(Mutex::new(desktop_wrapper));

    // Start the server
    info!("Starting HTTP server on 0.0.0.0:{}", args.port);

    match start_remote_server(desktop_arc, args.port).await {
        Ok(_) => {
            info!("Server stopped");
        }
        Err(e) => {
            error!("Server error: {}", e);
            return Err(e);
        }
    }

    Ok(())
}