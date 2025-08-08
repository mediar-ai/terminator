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
async fn test_i94_website_search_click() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    info!("🎯 Testing Multiple Recorded MCP Selectors");
    info!("==========================================");
    info!("Executing multiple selectors from our workflow recording:");
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

    // Array of selectors to test
    let selectors = [
        ("select text", "role:Pane|name:contains:I-94/I-95 Website >> role:tabitem|name:click to expand navigation options"),
        ("Search Text", "role:Pane|name:contains:I-94/I-95 Website >> role:text|name:Search"),
        ("Search in Pane (Alternative)", "role:Pane|name:contains:I-94/I-95 Website >> role:text|name:Search"),
    ];

    let mut success_count = 0;
    let total_count = selectors.len();

    for (i, (description, selector)) in selectors.iter().enumerate() {
        info!("🧪 Test {}/{}: {}", i + 1, total_count, description);
        info!("   Selector: '{}'", selector);

        let result = service
            .call_tool(CallToolRequestParam {
                name: "click_element".into(),
                arguments: Some(object!({
                    "selector": selector,
                    "timeout_ms": 3000
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
                        if text.to_lowercase().contains("success")
                            || text.to_lowercase().contains("clicked")
                        {
                            info!("   ✅ SUCCESS! Element clicked successfully");
                            success_count += 1;

                            // Try to parse the result for more details
                            if let Ok(result_data) = serde_json::from_str::<serde_json::Value>(text)
                            {
                                if let Some(element) = result_data.get("element") {
                                    if let Some(element_name) = element.get("name") {
                                        info!("   🎯 Element: {}", element_name);
                                    }
                                }
                            }
                        } else {
                            info!("   ❌ FAILED: {}", text);

                            // Check if it's the multiple elements issue
                            if text.contains("resolved to") && text.contains("elements") {
                                if let Some(start) = text.find("resolved to ") {
                                    if let Some(end) = text[start + 12..].find(" elements") {
                                        let count = &text[start + 12..start + 12 + end];
                                        info!("   📊 Found {} elements", count);
                                    }
                                }
                            }
                        }
                    }
                } else {
                    info!("   ❌ FAILED: No response content");
                }
            }
            Err(e) => {
                info!("   ❌ ERROR: {:?}", e);
            }
        }

        info!("");

        // Small delay between tests to avoid overwhelming the UI
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    // Final summary
    info!("🏁 Test Results Summary:");
    info!("   ✅ Successful: {}/{}", success_count, total_count);
    info!(
        "   ❌ Failed: {}/{}",
        total_count - success_count,
        total_count
    );

    if success_count > 0 {
        info!("🎉 At least one selector worked! The MCP converter fixes are functional.");
    } else {
        info!("❌ No selectors worked. Check if the I-94 website is open and has the expected elements.");
    }

    info!("");
    info!("🔍 Analysis:");
    info!("   If this test failed, it means either:");
    info!("   1. No I-94/I-95 Website panes are currently open");
    info!("   2. The panes don't contain 'Search' text elements");
    info!("   3. Our fix to click first element needs more work");
    info!("   4. The selector syntax needs adjustment");
    info!("");
    info!("💡 Next steps:");
    info!("   - Open an I-94 website in a browser");
    info!("   - Ensure the page contains searchable elements");
    info!("   - Re-run this test");

    service.cancel().await?;
    Ok(())
}
