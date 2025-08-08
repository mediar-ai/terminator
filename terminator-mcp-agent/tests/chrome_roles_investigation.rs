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
async fn investigate_chrome_roles() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    info!("🔍 Chrome UI Roles Investigation");
    info!("================================");

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

    // 1. Get all applications and find Chrome PIDs
    info!("📋 Step 1: Finding all Chrome processes...");
    let apps_result = service
        .call_tool(CallToolRequestParam {
            name: "get_applications".into(),
            arguments: Some(object!({})),
        })
        .await?;

    let mut chrome_pids = Vec::new();

    if !apps_result.content.is_empty() {
        let content = &apps_result.content[0];
        let json_str = serde_json::to_string(&content)?;
        let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

        if let Some(text) = parsed.get("text").and_then(|t| t.as_str()) {
            if let Ok(response) = serde_json::from_str::<serde_json::Value>(text) {
                if let Some(apps) = response.get("applications").and_then(|a| a.as_array()) {
                    for app in apps {
                        if let Some(name) = app.get("name").and_then(|n| n.as_str()) {
                            if name.to_lowercase().contains("chrome") {
                                if let Some(pid) = app.get("pid").and_then(|p| p.as_i64()) {
                                    let role = app
                                        .get("role")
                                        .and_then(|r| r.as_str())
                                        .unwrap_or("unknown");
                                    info!(
                                        "🌐 Chrome found: PID={}, Role={}, Name='{}'",
                                        pid, role, name
                                    );
                                    chrome_pids.push(pid);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Get window tree for each Chrome PID to see the full structure
    for pid in chrome_pids {
        info!("🏗️  Analyzing Chrome PID: {}", pid);

        let tree_result = service
            .call_tool(CallToolRequestParam {
                name: "get_window_tree".into(),
                arguments: Some(object!({
                    "pid": pid,
                    "include_tree": true
                })),
            })
            .await?;

        if !tree_result.content.is_empty() {
            let tree_content = &tree_result.content[0];
            let tree_json_str = serde_json::to_string(&tree_content)?;
            let tree_parsed: serde_json::Value = serde_json::from_str(&tree_json_str)?;

            if let Some(tree_text) = tree_parsed.get("text").and_then(|t| t.as_str()) {
                if let Ok(tree_data) = serde_json::from_str::<serde_json::Value>(tree_text) {
                    if let Some(windows) = tree_data.get("windows").and_then(|w| w.as_array()) {
                        info!("📊 Chrome PID {} has {} windows:", pid, windows.len());

                        for (i, window) in windows.iter().enumerate() {
                            let name = window
                                .get("name")
                                .and_then(|n| n.as_str())
                                .unwrap_or("(no name)");
                            let role = window
                                .get("role")
                                .and_then(|r| r.as_str())
                                .unwrap_or("unknown");
                            let id = window
                                .get("id")
                                .and_then(|i| i.as_str())
                                .unwrap_or("(no id)");

                            info!(
                                "  {}. Role: {:<10} | ID: {:<10} | Name: '{}'",
                                i + 1,
                                role,
                                id,
                                name
                            );

                            // Look specifically for "Window" role elements
                            if role == "Window" {
                                info!("    ✅ Found actual Window role! This might be our target.");

                                // Test this specific window
                                let window_selector = format!("#{}", id);
                                info!("    🎯 Testing selector: '{}'", window_selector);

                                let validate_result = service
                                    .call_tool(CallToolRequestParam {
                                        name: "validate_element".into(),
                                        arguments: Some(object!({
                                            "selector": window_selector,
                                            "timeout_ms": 500
                                        })),
                                    })
                                    .await?;

                                if !validate_result.content.is_empty() {
                                    let validate_content = &validate_result.content[0];
                                    let validate_json_str =
                                        serde_json::to_string(&validate_content)?;
                                    let validate_parsed: serde_json::Value =
                                        serde_json::from_str(&validate_json_str)?;

                                    if let Some(validate_text) =
                                        validate_parsed.get("text").and_then(|t| t.as_str())
                                    {
                                        if validate_text.contains("found")
                                            || validate_text.contains("success")
                                        {
                                            info!(
                                                "    ✅ Window selector WORKS: {}",
                                                window_selector
                                            );
                                        } else {
                                            info!(
                                                "    ❌ Window selector failed: {}",
                                                window_selector
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Test the pattern we should use in our MCP converter
    info!("🎯 Step 3: Testing the pattern we should generate...");

    let test_selectors = vec![
        "name:contains:I-94/I-95 Website",           // No role restriction
        "role:Pane|name:contains:I-94/I-95 Website", // Use Pane instead of Window
        "name:contains:Google Chrome",               // Generic Chrome window
    ];

    for selector in test_selectors {
        info!("🔍 Testing selector: '{}'", selector);
        let validate_result = service
            .call_tool(CallToolRequestParam {
                name: "validate_element".into(),
                arguments: Some(object!({
                    "selector": selector,
                    "timeout_ms": 1000
                })),
            })
            .await?;

        if !validate_result.content.is_empty() {
            let validate_content = &validate_result.content[0];
            let validate_json_str = serde_json::to_string(&validate_content)?;
            let validate_parsed: serde_json::Value = serde_json::from_str(&validate_json_str)?;

            if let Some(validate_text) = validate_parsed.get("text").and_then(|t| t.as_str()) {
                if validate_text.contains("found") || validate_text.contains("success") {
                    info!("✅ WORKS: {}", selector);
                } else {
                    info!("❌ FAILS: {}", selector);
                }
            }
        }
    }

    service.cancel().await?;
    Ok(())
}
