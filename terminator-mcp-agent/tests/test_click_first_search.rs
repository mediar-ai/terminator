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
async fn test_click_first_search() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    info!("🎯 Testing Click First Search Element in I-94 Page");
    info!("==================================================");
    info!("Goal: Click on the FIRST 'Search' element within a page containing I-94");
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

    // Strategy: Use run_javascript to find elements and click the first one
    info!("🧪 Attempting to click first search element using JavaScript approach");

    let js_result = service
        .call_tool(CallToolRequestParam {
            name: "run_javascript".into(),
            arguments: Some(object!({
                "script": r#"
                    // Find all 'Search' text elements within I-94 panes using scoped search
                    log('🔍 Looking for I-94 panes with Search elements...');
                    
                    try {
                        // First, find panes that contain I-94
                        const i94Panes = await desktop.locator('role:Pane|name:contains:I-94').all();
                        log(`Found ${i94Panes.length} I-94 panes`);
                        
                        if (i94Panes.length === 0) {
                            return { 
                                success: false, 
                                error: 'No I-94 panes found',
                                panes_found: 0
                            };
                        }
                        
                        // For each I-94 pane, find search elements within it
                        let totalSearchElements = 0;
                        let clickedElement = null;
                        
                        for (let i = 0; i < i94Panes.length; i++) {
                            const pane = i94Panes[i];
                            const paneName = await pane.name();
                            log(`Checking pane ${i + 1}: ${paneName}`);
                            
                            // Find search elements within this specific pane
                            // We'll use a more targeted search within the pane's scope
                            try {
                                const searchElements = await pane.locator('text:Search').all();
                                log(`  Found ${searchElements.length} search elements in this pane`);
                                totalSearchElements += searchElements.length;
                                
                                // Click the first search element we find
                                if (!clickedElement && searchElements.length > 0) {
                                    const firstSearch = searchElements[0];
                                    const elementName = await firstSearch.name();
                                    const elementText = await firstSearch.text();
                                    
                                    log(`🎯 Clicking first search element: name="${elementName}", text="${elementText}"`);
                                    await firstSearch.click();
                                    
                                    clickedElement = {
                                        name: elementName,
                                        text: elementText,
                                        pane: paneName,
                                        pane_index: i
                                    };
                                }
                            } catch (error) {
                                log(`  Error searching within pane ${i + 1}: ${error.message}`);
                            }
                        }
                        
                        return {
                            success: clickedElement !== null,
                            clicked_element: clickedElement,
                            total_search_elements: totalSearchElements,
                            i94_panes_found: i94Panes.length
                        };
                        
                    } catch (error) {
                        log(`❌ Error: ${error.message}`);
                        return { 
                            success: false, 
                            error: error.message 
                        };
                    }
                "#
            })),
        })
        .await?;

    if !js_result.content.is_empty() {
        let content = &js_result.content[0];
        let json_str = serde_json::to_string(&content)?;
        let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

        if let Some(text) = parsed.get("text").and_then(|t| t.as_str()) {
            // Try to parse the result as JSON
            if let Ok(result) = serde_json::from_str::<serde_json::Value>(text) {
                if let Some(success) = result.get("success").and_then(|s| s.as_bool()) {
                    if success {
                        info!("🎉 SUCCESS! Clicked the first search element");

                        if let Some(clicked) = result.get("clicked_element") {
                            if let Some(name) = clicked.get("name").and_then(|n| n.as_str()) {
                                info!("  🎯 Element name: {}", name);
                            }
                            if let Some(text) = clicked.get("text").and_then(|t| t.as_str()) {
                                info!("  📝 Element text: {}", text);
                            }
                            if let Some(pane) = clicked.get("pane").and_then(|p| p.as_str()) {
                                info!("  📋 I-94 Pane: {}", pane);
                            }
                        }

                        if let Some(total) =
                            result.get("total_search_elements").and_then(|t| t.as_u64())
                        {
                            info!("  📊 Total search elements found: {}", total);
                        }

                        if let Some(panes) = result.get("i94_panes_found").and_then(|p| p.as_u64())
                        {
                            info!("  📋 I-94 panes found: {}", panes);
                        }

                        service.cancel().await?;
                        return Ok(());
                    } else {
                        if let Some(error) = result.get("error").and_then(|e| e.as_str()) {
                            info!("❌ Failed: {}", error);
                        }

                        if let Some(panes) = result.get("panes_found").and_then(|p| p.as_u64()) {
                            info!("  📋 I-94 panes found: {}", panes);
                        }
                    }
                }
            } else {
                info!("📋 JavaScript output: {}", text);
            }
        }
    }

    info!("");
    info!("💡 Alternative: Try direct MCP click with index selector");
    info!("   (This might not work yet but worth testing)");

    // Try the direct MCP approach with an index (this might not be implemented)
    let direct_result = service
        .call_tool(CallToolRequestParam {
            name: "click_element".into(),
            arguments: Some(object!({
                "selector": "role:Pane|name:contains:I-94 >> text:Search",
                "timeout_ms": 3000,
                "click_first": true // This parameter might help if implemented
            })),
        })
        .await?;

    if !direct_result.content.is_empty() {
        let content = &direct_result.content[0];
        let json_str = serde_json::to_string(&content)?;
        let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

        if let Some(text) = parsed.get("text").and_then(|t| t.as_str()) {
            if text.to_lowercase().contains("success") {
                info!("🎉 SUCCESS with direct MCP click!");
            } else {
                info!("❌ Direct MCP click failed: {}", text);
            }
        }
    }

    service.cancel().await?;
    Ok(())
}
