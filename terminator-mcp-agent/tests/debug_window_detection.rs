use anyhow::Result;
use rmcp::transport::TokioChildProcess;
use rmcp::{model::CallToolRequestParam, object, ServiceExt};
use std::env;
use std::path::PathBuf;
use tokio::process::Command;
use tracing::{info, warn};

/// Helper to get the path to the MCP agent binary
fn get_agent_binary_path() -> PathBuf {
    let mut path = env::current_exe().unwrap();
    path.pop(); // Remove the test binary name
    path.pop(); // Remove 'deps'
    path.pop(); // Remove 'debug' or 'release'
    path.push("release"); // Use release build
    path.push("terminator-mcp-agent");
    #[cfg(target_os = "windows")]
    path.set_extension("exe");
    path
}

#[tokio::test]
async fn debug_window_detection() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    info!("🔍 Debugging Window Detection");
    info!("============================");
    info!("Let's see what windows are actually available and why our selector isn't working");
    info!("");

    let agent_path = get_agent_binary_path();
    if !agent_path.exists() {
        warn!("❌ MCP agent binary not found at {agent_path:?}");
        return Ok(());
    }

    // Start the MCP agent server
    let mut cmd = Command::new(&agent_path);
    cmd.args(["-t", "stdio"]);
    let service = ().serve(TokioChildProcess::new(cmd)?).await?;

    info!("✅ Connected to MCP agent");
    info!("");

    // First, let's see all running applications
    info!("📋 Step 1: Getting all running applications...");
    let apps_result = service
        .call_tool(CallToolRequestParam {
            name: "get_applications".into(),
            arguments: None,
        })
        .await?;

    if !apps_result.content.is_empty() {
        let content = &apps_result.content[0];
        let json_str = serde_json::to_string(&content)?;
        let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

        if let Some(text) = parsed.get("text").and_then(|t| t.as_str()) {
            info!("📱 Available applications:");
            // Look for Chrome/Browser apps
            for line in text.lines() {
                if line.contains("Chrome")
                    || line.contains("Firefox")
                    || line.contains("Edge")
                    || line.contains("I-94")
                {
                    info!("   🌐 {}", line);
                }
            }
        }
    }

    info!("");
    info!("🔍 Step 2: Testing different selector variations...");

    let test_selectors = vec![
        "role:Window|name:contains:I-94/I-95 Website >> role:text|name:Search",
        "role:Window|name:contains:I-94",
        "role:Window|name:contains:Chrome",
        "role:Window|name:contains:Google Chrome",
        "text:Search",
    ];

    for (i, selector) in test_selectors.iter().enumerate() {
        info!("🧪 Test {}: '{}'", i + 1, selector);

        let result = service
            .call_tool(CallToolRequestParam {
                name: "validate_element".into(),
                arguments: Some(object!({
                    "selector": selector,
                    "timeout_ms": 2000
                })),
            })
            .await;

        match result {
            Ok(response) => {
                if !response.content.is_empty() {
                    let content = &response.content[0];
                    let json_str = serde_json::to_string(&content)?;
                    let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

                    if let Some(text) = parsed.get("text").and_then(|t| t.as_str()) {
                        if text.contains("success") || text.contains("found") {
                            info!("   ✅ FOUND: {}", text);
                        } else {
                            info!("   ❌ NOT FOUND: {}", text);
                        }
                    }
                }
            }
            Err(e) => {
                info!("   ❌ ERROR: {:?}", e);
            }
        }
    }

    info!("");
    info!("🎯 Step 3: Let's get the window tree for Chrome to see the actual structure...");

    // Try to get window tree for Chrome
    let tree_result = service
        .call_tool(CallToolRequestParam {
            name: "get_window_tree".into(),
            arguments: Some(object!({
                "application_name": "chrome",
                "include_tree": true
            })),
        })
        .await;

    match tree_result {
        Ok(response) => {
            if !response.content.is_empty() {
                let content = &response.content[0];
                let json_str = serde_json::to_string(&content)?;
                let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

                if let Some(text) = parsed.get("text").and_then(|t| t.as_str()) {
                    info!("🌳 Chrome window tree (first 1000 chars):");
                    let preview = if text.len() > 1000 {
                        &text[..1000]
                    } else {
                        text
                    };
                    info!("{}", preview);
                    if text.len() > 1000 {
                        info!("... (truncated)");
                    }
                }
            }
        }
        Err(e) => {
            info!("❌ Failed to get Chrome window tree: {:?}", e);
        }
    }

    service.cancel().await?;
    Ok(())
}
