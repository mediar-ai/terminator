use anyhow::Result;
use rmcp::{
    transport::TokioChildProcess, 
    ServiceExt, model::*
};
use serde_json::json;
use std::env;
use tokio::process::Command;
use tracing::{info, error};

/// REAL MCP Client: Actually uses terminator MCP tools for desktop automation
/// 
/// This example demonstrates actual MCP protocol usage:
/// 1. Calls get_applications via MCP
/// 2. Uses click_element via MCP  
/// 3. Calls capture_screen via MCP
/// 4. Uses type_into_element via MCP
/// 5. Real desktop automation through MCP protocol
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("mcp_client_example=info,rmcp=debug")
        .init();

    info!("🚀 Starting REAL MCP Client - Using Terminator Tools");

    // Build the path to the terminator-mcp-agent binary
    let agent_path = env::current_dir()?
        .join("target")
        .join("release")
        .join("terminator-mcp-agent");

    info!("Looking for terminator-mcp-agent at: {}", agent_path.display());

    if !agent_path.exists() {
        error!("❌ terminator-mcp-agent not found. Please build it first with:");
        error!("   cargo build --release --bin terminator-mcp-agent");
        return Ok(());
    }

    // Create command to spawn the MCP agent
    let mut cmd = Command::new(&agent_path);
    cmd.stdin(std::process::Stdio::piped())
       .stdout(std::process::Stdio::piped())
       .stderr(std::process::Stdio::piped());

    info!("🔧 Spawning terminator-mcp-agent process...");

    // Create transport using the correct rmcp API
    let transport = TokioChildProcess::new(&mut cmd)?;
    info!("✅ MCP transport created successfully");
    
    // Use the correct rmcp pattern - serve with empty handler
    let client_peer = ().serve(transport).await?;
    info!("🔌 MCP client connection established successfully!");

    // Now actually USE the MCP tools through the client_peer!
    info!("🎯 Calling REAL terminator MCP tools...");
    
    // Demo 1: List available tools first
    demo_list_tools(&client_peer).await?;
    
    // Demo 2: Get applications using MCP
    demo_get_applications(&client_peer).await?;
    
    // Demo 3: Try screen capture using MCP
    demo_screen_capture(&client_peer).await?;
    
    // Demo 4: Try launching calculator and automating it using MCP
    demo_calculator_automation(&client_peer).await?;
    
    // Demo 5: Test clipboard operations using MCP
    demo_clipboard_operations(&client_peer).await?;
    
    info!("🏁 REAL MCP Client completed - Actually used terminator tools!");
    Ok(())
}

async fn demo_list_tools(client_peer: &rmcp::service::Peer<rmcp::service::RoleClient>) -> Result<()> {
    info!("📋 Demo 0: Listing available tools via MCP");
    
    match client_peer.list_tools(ListToolsParams {}).await {
        Ok(result) => {
            info!("✅ MCP list_tools SUCCESS - Found {} tools:", result.tools.len());
            for tool in &result.tools {
                info!("  🔧 Tool: {} - {}", tool.name, tool.description.as_deref().unwrap_or("No description"));
            }
        },
        Err(e) => {
            info!("⚠️ MCP list_tools failed: {}", e);
        }
    }
    
    Ok(())
}

async fn demo_get_applications(client_peer: &rmcp::service::Peer<rmcp::service::RoleClient>) -> Result<()> {
    info!("📱 Demo 1: Getting applications via MCP get_applications tool");
    
    let params = json!({});
    
    match client_peer.call_tool("get_applications", params).await {
        Ok(result) => {
            info!("✅ MCP get_applications SUCCESS:");
            for content in &result.content {
                match content {
                    Content::Text { text } => {
                        info!("� Text result: {}", text);
                    },
                    Content::Image { .. } => {
                        info!("🖼️ Image result received");
                    },
                    Content::Resource { .. } => {
                        info!("📦 Resource result received");
                    }
                }
            }
        },
        Err(e) => {
            info!("⚠️ MCP get_applications failed (expected in headless): {}", e);
        }
    }
    
    Ok(())
}

async fn demo_screen_capture(client_peer: &rmcp::service::Peer<rmcp::service::RoleClient>) -> Result<()> {
    info!("📸 Demo 2: Screen capture via MCP capture_screen tool");
    
    let params = json!({});
    
    match client_peer.call_tool("capture_screen", params).await {
        Ok(result) => {
            info!("✅ MCP capture_screen SUCCESS:");
            for content in &result.content {
                match content {
                    Content::Text { text } => {
                        info!("📄 Text result: {}", text);
                    },
                    Content::Image { .. } => {
                        info!("🖼️ Image result received (screenshot captured!)");
                    },
                    Content::Resource { .. } => {
                        info!("📦 Resource result received");
                    }
                }
            }
        },
        Err(e) => {
            info!("⚠️ MCP capture_screen failed (expected in headless): {}", e);
        }
    }
    
    Ok(())
}

async fn demo_calculator_automation(client_peer: &rmcp::service::Peer<rmcp::service::RoleClient>) -> Result<()> {
    info!("🧮 Demo 3: Calculator automation via MCP tools");
    
    // First, try to open calculator using MCP
    info!("  🚀 Opening calculator via MCP open_application");
    let open_params = json!({
        "application_name": "gnome-calculator"
    });
    
    match client_peer.call_tool("open_application", open_params).await {
        Ok(result) => {
            info!("✅ MCP open_application SUCCESS:");
            for content in &result.content {
                if let Content::Text { text } = content {
                    info!("📄 Result: {}", text);
                }
            }
            
            // Wait for app to start
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            
            // Try to click on calculator buttons using MCP
            info!("  🔢 Clicking calculator buttons via MCP click_element");
            let click_params = json!({
                "selector_chain": ["name:Calculator"],
                "timeout_ms": 5000
            });
            
            match client_peer.call_tool("click_element", click_params).await {
                Ok(result) => {
                    info!("✅ MCP click_element SUCCESS:");
                    for content in &result.content {
                        if let Content::Text { text } = content {
                            info!("📄 Click result: {}", text);
                        }
                    }
                },
                Err(e) => {
                    info!("⚠️ MCP click_element failed: {}", e);
                }
            }
            
            // Try typing into calculator using MCP
            info!("  ⌨️ Typing into calculator via MCP type_into_element");
            let type_params = json!({
                "selector_chain": ["name:Calculator"],
                "text_to_type": "2+2=",
                "timeout_ms": 5000
            });
            
            match client_peer.call_tool("type_into_element", type_params).await {
                Ok(result) => {
                    info!("✅ MCP type_into_element SUCCESS:");
                    for content in &result.content {
                        if let Content::Text { text } = content {
                            info!("📄 Type result: {}", text);
                        }
                    }
                },
                Err(e) => {
                    info!("⚠️ MCP type_into_element failed: {}", e);
                }
            }
        },
        Err(e) => {
            info!("⚠️ MCP open_application failed: {}", e);
        }
    }
    
    Ok(())
}

async fn demo_clipboard_operations(client_peer: &rmcp::service::Peer<rmcp::service::RoleClient>) -> Result<()> {
    info!("📋 Demo 4: Clipboard operations via MCP tools");
    
    // Set clipboard using MCP
    info!("  📝 Setting clipboard via MCP set_clipboard");
    let set_params = json!({
        "text": "Hello from MCP terminator automation!"
    });
    
    match client_peer.call_tool("set_clipboard", set_params).await {
        Ok(result) => {
            info!("✅ MCP set_clipboard SUCCESS:");
            for content in &result.content {
                if let Content::Text { text } = content {
                    info!("📄 Set result: {}", text);
                }
            }
            
            // Get clipboard using MCP
            info!("  📖 Getting clipboard via MCP get_clipboard");
            let get_params = json!({});
            
            match client_peer.call_tool("get_clipboard", get_params).await {
                Ok(result) => {
                    info!("✅ MCP get_clipboard SUCCESS:");
                    for content in &result.content {
                        if let Content::Text { text } = content {
                            info!("📄 Clipboard content: {}", text);
                        }
                    }
                },
                Err(e) => {
                    info!("⚠️ MCP get_clipboard failed: {}", e);
                }
            }
        },
        Err(e) => {
            info!("⚠️ MCP set_clipboard failed: {}", e);
        }
    }
    
    Ok(())
}