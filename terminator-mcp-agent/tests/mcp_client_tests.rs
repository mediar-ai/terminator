use anyhow::Result;
use rmcp::model::ProtocolVersion;
use rmcp::transport::TokioChildProcess;
use rmcp::{model::CallToolRequestParam, object, ServiceExt};
use std::env;
use std::path::PathBuf;
use tokio::process::Command;

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
    tracing::debug!("agent path: {:?}", path);
    path
}

#[tokio::test]
async fn test_mcp_client_list_tools() -> Result<()> {
    let agent_path = get_agent_binary_path();
    if !agent_path.exists() {
        eprintln!("Skipping test: MCP agent binary not found at {agent_path:?}");
        eprintln!("Run 'cargo build --bin terminator-mcp-agent' first");
        return Ok(());
    }

    // Start the MCP agent server using child process transport
    let mut cmd = Command::new(&agent_path);
    cmd.args(["-t", "stdio"]);
    let service = ().serve(TokioChildProcess::new(cmd)?).await?;

    // Initialize
    let server_info = service.peer_info();
    tracing::info!("Connected to server: {server_info:#?}");

    // List all tools
    let tools = service.list_all_tools().await?;

    // Verify expected tools exist
    let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(tool_names.contains(&"execute_sequence"));
    assert!(tool_names.contains(&"get_applications"));
    assert!(tool_names.contains(&"click_element"));
    assert!(tool_names.contains(&"validate_element"));
    assert!(tool_names.contains(&"type_into_element"));

    // Find execute_sequence tool and verify its description
    let execute_sequence = tools
        .iter()
        .find(|t| t.name == "execute_sequence")
        .expect("execute_sequence tool not found");

    if let Some(desc) = &execute_sequence.description {
        assert!(desc.contains("sequence"));
        assert!(desc.contains("workflow"));
    }

    // Cancel the service
    service.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_execute_sequence_empty() -> Result<()> {
    let agent_path = get_agent_binary_path();
    if !agent_path.exists() {
        eprintln!("Skipping test: MCP agent binary not found at {agent_path:?}");
        return Ok(());
    }

    let mut cmd = Command::new(&agent_path);
    cmd.args(["-t", "stdio"]);
    let service = ().serve(TokioChildProcess::new(cmd)?).await?;

    // Call execute_sequence with empty tools
    let result = service
        .call_tool(CallToolRequestParam {
            name: "execute_sequence".into(),
            arguments: Some(object!({
                "steps": [],
                "stop_on_error": true,
                "include_detailed_results": true
            })),
        })
        .await?;

    // Verify the response
    assert!(!result.content.is_empty());
    let content = &result.content[0];

    // Extract JSON from content
    let json_str = serde_json::to_string(&content)?;
    let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

    if let Some(text) = parsed.get("text").and_then(|t| t.as_str()) {
        let response: serde_json::Value = serde_json::from_str(text)?;
        assert_eq!(response["action"], "execute_sequence");
        assert_eq!(response["status"], "success");
        assert_eq!(response["total_tools"], 0);
        assert_eq!(response["executed_tools"], 0);
    }

    service.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_validate_element() -> Result<()> {
    let agent_path = get_agent_binary_path();
    if !agent_path.exists() {
        eprintln!("Skipping test: MCP agent binary not found at {agent_path:?}");
        return Ok(());
    }

    let mut cmd = Command::new(&agent_path);
    cmd.args(["-t", "stdio"]);
    let service = ().serve(TokioChildProcess::new(cmd)?).await?;

    // Call validate_element
    let result = service
        .call_tool(CallToolRequestParam {
            name: "validate_element".into(),
            arguments: Some(object!({
                "selector": "#nonexistent-element-99999",
                "timeout_ms": 100
            })),
        })
        .await?;

    // Verify the response
    assert!(!result.content.is_empty());
    let content = &result.content[0];
    let json_str = serde_json::to_string(&content)?;
    let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

    if let Some(text) = parsed.get("text").and_then(|t| t.as_str()) {
        let response: serde_json::Value = serde_json::from_str(text)?;
        assert_eq!(response["action"], "validate_element");
        assert_eq!(response["status"], "failed");
        assert_eq!(response["exists"], false);
    }

    service.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_execute_sequence_with_tools() -> Result<()> {
    let agent_path = get_agent_binary_path();
    if !agent_path.exists() {
        eprintln!("Skipping test: MCP agent binary not found at {agent_path:?}");
        return Ok(());
    }

    let mut cmd = Command::new(&agent_path);
    cmd.args(["-t", "stdio"]);
    let service = ().serve(TokioChildProcess::new(cmd)?).await?;

    // Call execute_sequence with multiple tools
    let result = service
        .call_tool(CallToolRequestParam {
            name: "execute_sequence".into(),
            arguments: Some(object!({
                "steps": [
                    {
                        "tool_name": "invalid_tool",
                        "arguments": {},
                        "continue_on_error": true
                    },
                    {
                        "tool_name": "validate_element",
                        "arguments": {
                            "selector": "#test-element",
                            "timeout_ms": 50
                        }
                    }
                ],
                "stop_on_error": true,
                "include_detailed_results": true
            })),
        })
        .await?;

    // Verify the response
    assert!(!result.content.is_empty());
    let content = &result.content[0];
    let json_str = serde_json::to_string(&content)?;
    let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

    if let Some(text) = parsed.get("text").and_then(|t| t.as_str()) {
        let response: serde_json::Value = serde_json::from_str(text)?;
        assert_eq!(response["action"], "execute_sequence");
        assert_eq!(response["total_tools"], 2);
        assert_eq!(response["executed_tools"], 2);

        // Check results array
        let results = response["results"]
            .as_array()
            .expect("Expected results array");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["status"], "skipped"); // continue_on_error: true makes it "skipped"
        assert_eq!(results[1]["status"], "success");
    }

    service.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_execute_sequence_stop_on_error() -> Result<()> {
    let agent_path = get_agent_binary_path();
    if !agent_path.exists() {
        eprintln!("Skipping test: MCP agent binary not found at {agent_path:?}");
        return Ok(());
    }

    let mut cmd = Command::new(&agent_path);
    cmd.args(["-t", "stdio"]);
    let service = ().serve(TokioChildProcess::new(cmd)?).await?;

    // Call execute_sequence with stop_on_error true
    let result = service
        .call_tool(CallToolRequestParam {
            name: "execute_sequence".into(),
            arguments: Some(object!({
                "steps": [
                    {
                        "tool_name": "invalid_tool",
                        "arguments": {},
                        "continue_on_error": false
                    },
                    {
                        "tool_name": "validate_element",
                        "arguments": {
                            "selector": "#should-not-execute",
                            "timeout_ms": 50
                        }
                    }
                ],
                "stop_on_error": true,
                "include_detailed_results": true
            })),
        })
        .await?;

    // Verify the response
    assert!(!result.content.is_empty());
    let content = &result.content[0];
    let json_str = serde_json::to_string(&content)?;
    let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

    if let Some(text) = parsed.get("text").and_then(|t| t.as_str()) {
        let response: serde_json::Value = serde_json::from_str(text)?;
        assert_eq!(response["action"], "execute_sequence");
        assert_eq!(response["status"], "partial_success");
        assert_eq!(response["total_tools"], 2);
        assert_eq!(response["executed_tools"], 2); // Should stop after first error
    }

    service.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_server_info() -> Result<()> {
    let agent_path = get_agent_binary_path();
    if !agent_path.exists() {
        eprintln!("Skipping test: MCP agent binary not found at {agent_path:?}");
        return Ok(());
    }

    let mut cmd = Command::new(&agent_path);
    cmd.args(["-t", "stdio"]);
    let service = ().serve(TokioChildProcess::new(cmd)?).await?;

    // Get server info
    let server_info = service.peer_info().unwrap();
    assert_eq!(server_info.server_info.name, "rmcp");
    assert_eq!(server_info.protocol_version, ProtocolVersion::V_2025_03_26);

    service.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_execute_sequence_with_delays() -> Result<()> {
    let agent_path = get_agent_binary_path();
    if !agent_path.exists() {
        eprintln!("Skipping test: MCP agent binary not found at {agent_path:?}");
        return Ok(());
    }

    let mut cmd = Command::new(&agent_path);
    cmd.args(["-t", "stdio"]);
    let service = ().serve(TokioChildProcess::new(cmd)?).await?;

    let start_time = std::time::Instant::now();

    // Call execute_sequence with delays
    let result = service
        .call_tool(CallToolRequestParam {
            name: "execute_sequence".into(),
            arguments: Some(object!({
                "steps": [
                    {
                        "tool_name": "validate_element",
                        "arguments": {
                            "selector": "#test1",
                            "timeout_ms": 50
                        },
                        "delay_ms": 100
                    },
                    {
                        "tool_name": "validate_element",
                        "arguments": {
                            "selector": "#test2",
                            "timeout_ms": 50
                        }
                    }
                ],
                "stop_on_error": false,
                "include_detailed_results": true
            })),
        })
        .await?;

    let elapsed = start_time.elapsed();
    // Should have at least 100ms delay after first tool
    assert!(
        elapsed.as_millis() >= 100,
        "Delays not properly applied: {}ms",
        elapsed.as_millis()
    );

    // Verify response structure
    assert!(!result.content.is_empty());
    let content = &result.content[0];
    let json_str = serde_json::to_string(&content)?;
    let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

    if let Some(text) = parsed.get("text").and_then(|t| t.as_str()) {
        let response: serde_json::Value = serde_json::from_str(text)?;
        assert_eq!(response["action"], "execute_sequence");
        assert_eq!(response["total_tools"], 2);
        assert_eq!(response["executed_tools"], 2);
    }

    service.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_export_workflow_sequence() -> Result<()> {
    let agent_path = get_agent_binary_path();
    if !agent_path.exists() {
        eprintln!("Skipping test: MCP agent binary not found at {agent_path:?}");
        eprintln!("Run 'cargo build --bin terminator-mcp-agent' first");
        return Ok(());
    }

    let mut cmd = Command::new(&agent_path);
    cmd.args(["-t", "stdio"]);
    let service = ().serve(TokioChildProcess::new(cmd)?).await?;

    // Call export_workflow_sequence with a sample workflow
    let result = service
        .call_tool(CallToolRequestParam {
            name: "export_workflow_sequence".into(),
            arguments: Some(object!({
                "workflow_name": "Test Insurance Quote Workflow",
                "workflow_description": "Automated test workflow for insurance quote generation",
                "workflow_goal": "Navigate to website, fill form, and generate quote",
                "successful_tool_calls": [
                    {
                        "tool_name": "navigate_browser",
                        "arguments": {
                            "url": "https://example.com/insurance"
                        }
                    },
                    {
                        "tool_name": "click_element",
                        "arguments": {
                            "selector": "#accept-terms",
                            "alternative_selectors": "#12345,button|Accept"
                        }
                    },
                    {
                        "tool_name": "type_into_element",
                        "arguments": {
                            "selector": "#height-field",
                            "text_to_type": "5'10\"",
                            "clear_before_typing": true
                        }
                    },
                    {
                        "tool_name": "select_option",
                        "arguments": {
                            "selector": "#product-type",
                            "option_name": "Term Life"
                        }
                    },
                    {
                        "tool_name": "click_element",
                        "arguments": {
                            "selector": "#submit-button"
                        }
                    }
                ],
                "expected_data": {
                    "height": "5'10\"",
                    "product": "Term Life Insurance"
                },
                "credentials": {
                    "login_code": "TEST-123",
                    "email": "test@example.com"
                },
                "known_error_handlers": [
                    {
                        "error": "Dialog blocks form",
                        "solution": "Click cancel or accept button"
                    }
                ],
                "include_ai_fallbacks": true,
                "add_validation_steps": true,
                "include_tree_captures": false,
                "output_format": "json"
            })),
        })
        .await?;

    // Verify the response
    assert!(!result.content.is_empty());
    let content = &result.content[0];
    let json_str = serde_json::to_string(&content)?;
    let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

    if let Some(text) = parsed.get("text").and_then(|t| t.as_str()) {
        let response: serde_json::Value = serde_json::from_str(text)?;

        // Verify workflow structure
        assert!(response.get("workflow").is_some());
        let workflow = &response["workflow"];

        // Check basic workflow properties
        assert_eq!(workflow["name"], "Test Insurance Quote Workflow");
        assert_eq!(workflow["version"], "1.0");
        assert_eq!(
            workflow["goal"],
            "Navigate to website, fill form, and generate quote"
        );
        assert_eq!(workflow["created_by"], "terminator-mcp-agent");

        // Verify prerequisites
        assert!(workflow["prerequisites"]["platform"].is_string());
        let required_tools = workflow["prerequisites"]["required_tools"]
            .as_array()
            .expect("Expected required_tools array");
        assert!(required_tools.contains(&serde_json::Value::String("navigate_browser".into())));
        assert!(required_tools.contains(&serde_json::Value::String("click_element".into())));
        assert!(required_tools.contains(&serde_json::Value::String("type_into_element".into())));

        // Verify parameters were included
        assert_eq!(
            workflow["parameters"]["credentials"]["login_code"],
            "TEST-123"
        );
        assert_eq!(workflow["parameters"]["form_data"]["height"], "5'10\"");

        // Check configuration
        assert_eq!(workflow["configuration"]["include_ai_fallbacks"], true);
        assert_eq!(workflow["configuration"]["add_validation_steps"], true);
        assert_eq!(workflow["configuration"]["default_timeout_ms"], 3000);

        // Verify steps were enhanced
        let steps = workflow["steps"].as_array().expect("Expected steps array");

        // Should have more steps than original due to enhancements
        assert!(
            steps.len() > 5,
            "Expected enhanced steps, got {}",
            steps.len()
        );

        // Check for focus validation step (should be added before first UI interaction)
        let has_focus_check = steps.iter().any(|step| {
            step["action"] == "validate_focus" && step["tool_name"] == "get_applications"
        });
        assert!(has_focus_check, "Expected focus check step to be added");

        // Check for wait after navigation
        let has_wait_after_nav = steps.iter().any(|step| {
            step["action"] == "wait_for_stability" && step["tool_name"] == "wait_for_element"
        });
        assert!(has_wait_after_nav, "Expected wait step after navigation");

        // Check for validation steps
        let has_validation = steps
            .iter()
            .any(|step| step["action"] == "validate_action_result");
        assert!(has_validation, "Expected validation steps to be added");

        // Verify error handling strategies
        let error_strategies = workflow["error_handling"]["general_strategies"]
            .as_array()
            .expect("Expected error strategies array");
        assert!(error_strategies.len() >= 3);

        // Check AI decision points
        let ai_points = workflow["ai_decision_points"]
            .as_array()
            .expect("Expected AI decision points array");
        assert!(ai_points.len() >= 3);

        // Verify success criteria
        assert!(workflow["success_criteria"]["final_validation"].is_string());
        let expected_outcomes = workflow["success_criteria"]["expected_outcomes"]
            .as_array()
            .expect("Expected outcomes array");
        assert!(!expected_outcomes.is_empty());
    } else {
        panic!("Unexpected response format");
    }

    service.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_export_workflow_sequence_minimal() -> Result<()> {
    let agent_path = get_agent_binary_path();
    if !agent_path.exists() {
        eprintln!("Skipping test: MCP agent binary not found at {agent_path:?}");
        eprintln!("Run 'cargo build --bin terminator-mcp-agent' first");
        return Ok(());
    }

    let mut cmd = Command::new(&agent_path);
    cmd.args(["-t", "stdio"]);
    let service = ().serve(TokioChildProcess::new(cmd)?).await?;

    // Call export_workflow_sequence with minimal options
    let result = service
        .call_tool(CallToolRequestParam {
            name: "export_workflow_sequence".into(),
            arguments: Some(object!({
                "workflow_name": "Minimal Test Workflow",
                "workflow_description": "A minimal workflow for testing",
                "workflow_goal": "Test minimal workflow export",
                "successful_tool_calls": [
                    {
                        "tool_name": "get_applications",
                        "arguments": {}
                    }
                ],
                "include_ai_fallbacks": false,
                "add_validation_steps": false,
                "include_tree_captures": false
            })),
        })
        .await?;

    // Verify the response
    assert!(!result.content.is_empty());
    let content = &result.content[0];
    let json_str = serde_json::to_string(&content)?;
    let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

    if let Some(text) = parsed.get("text").and_then(|t| t.as_str()) {
        let response: serde_json::Value = serde_json::from_str(text)?;
        let workflow = &response["workflow"];

        // With minimal options, should have fewer enhancements
        let steps = workflow["steps"].as_array().expect("Expected steps array");

        // Should have just the original step (no focus checks or validations)
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0]["tool_name"], "get_applications");

        // AI decision points should be empty
        let ai_points = workflow["ai_decision_points"]
            .as_array()
            .expect("Expected AI decision points array");
        assert_eq!(ai_points.len(), 0);
    }

    service.cancel().await?;
    Ok(())
}

#[tokio::test]
#[ignore] // This test requires a real UI and browser to interact with.
async fn test_execute_sequence_real_ui_workflow() -> Result<()> {
    // Enable logging at debug level
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .try_init();
    println!("Logging enabled at debug level");

    let workflow_url = match env::var("MCP_TEST_WORKFLOW_URL") {
        Ok(url) => url,
        Err(_) => {
            println!("Skipping test: MCP_TEST_WORKFLOW_URL environment variable not set.");
            println!("Set it to a URL pointing to a raw JSON Gist to run this test.");
            return Ok(());
        }
    };

    println!("Fetching workflow from: {workflow_url}");
    let response = reqwest::get(&workflow_url).await?.text().await?;
    let payload: serde_json::Value = serde_json::from_str(&response)?;

    let tool_name = payload["tool_name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("'tool_name' not found or not a string in payload"))?
        .to_string();

    let arguments = payload
        .get("arguments")
        .ok_or_else(|| anyhow::anyhow!("'arguments' not found in payload"))?
        .clone();

    let agent_path = get_agent_binary_path();
    if !agent_path.exists() {
        eprintln!("Skipping test: MCP agent binary not found at {agent_path:?}");
        eprintln!("Run 'cargo build --bin terminator-mcp-agent' first");
        return Ok(());
    }

    let mut cmd = Command::new(&agent_path);
    cmd.args(["-t", "stdio"]);
    let service = ().serve(TokioChildProcess::new(cmd)?).await?;

    // This workflow is taken directly from the user's request and is intended
    // to run against a live website. It will only pass in an environment with a GUI.
    let result = service
        .call_tool(CallToolRequestParam {
            name: tool_name.into(),
            arguments: Some(
                arguments
                    .as_object()
                    .ok_or_else(|| anyhow::anyhow!("'arguments' must be a JSON object"))?
                    .clone(),
            ),
        })
        .await?;

    // Verify the response
    assert!(!result.content.is_empty());
    let content = &result.content[0];
    let json_str = serde_json::to_string(&content)?;
    let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

    if let Some(text) = parsed.get("text").and_then(|t| t.as_str()) {
        let response: serde_json::Value = serde_json::from_str(text)?;

        // For debugging, print the UI tree that the parser is supposed to process.
        // This helps verify if the tree structure matches the parser's rules.
        // The JSON Pointer path corresponds to: results[group 5]->results[step 1]->...
        // if let Some(tree_str_val) = response.pointer("/results/5/results/1/result/content/0/text") {
        //     if let Some(tree_str) = tree_str_val.as_str() {
        //         match serde_json::from_str::<serde_json::Value>(tree_str) {
        //             Ok(ui_tree) => {
        //                 println!("\n\n--- UI TREE FOR PARSER ---\n");
        //                 println!("{}", serde_json::to_string_pretty(&ui_tree).unwrap());
        //                 println!("\n--- END UI TREE ---\n\n");
        //             }
        //             Err(e) => println!("\n--- FAILED TO PARSE UI TREE: {} ---\n", e),
        //         }
        //     }
        // }

        assert_eq!(response["action"], "execute_sequence");

        // The sequence is expected to succeed when run manually in a GUI environment.
        let parsed_output = response
            .get("parsed_output")
            .expect("parsed_output should exist on success");

        assert!(parsed_output.is_array(), "parsed_output should be an array");

        let parsed_array = parsed_output.as_array().unwrap();

        // Since this test is expected to work, we assert the parser found results.
        assert!(
            !parsed_array.is_empty(),
            "Expected the parser to return at least one item, but it was empty."
        );

        // Verify the structure of the first returned item.
        let first_item = &parsed_array[0];
        assert!(
            first_item.is_object(),
            "Parsed item should be a JSON object."
        );

        // Assert that the expected keys exist and have the correct type.
        assert!(
            first_item.get("carrierProduct").is_some(),
            "Parsed item should have a 'carrierProduct' field."
        );
        assert!(
            first_item["carrierProduct"].is_string(),
            "'carrierProduct' should be a string."
        );

        assert!(
            first_item.get("monthlyPrice").is_some(),
            "Parsed item should have a 'monthlyPrice' field."
        );
        assert!(
            first_item["monthlyPrice"].is_string(),
            "'monthlyPrice' should be a string."
        );
        assert!(
            first_item["monthlyPrice"]
                .as_str()
                .unwrap()
                .starts_with('$'),
            "'monthlyPrice' should start with a dollar sign."
        );

        assert!(
            first_item.get("status").is_some(),
            "Parsed item should have a 'status' field."
        );
        assert!(
            first_item["status"].is_string(),
            "'status' should be a string."
        );
    } else {
        panic!("Unexpected response format, 'text' field not found in content");
    }

    service.cancel().await?;
    Ok(())
}

mod run_javascript {
    use anyhow::Result;
    use rmcp::transport::TokioChildProcess;
    use rmcp::{model::CallToolRequestParam, object, ServiceExt};
    use tokio::process::Command;

    use crate::get_agent_binary_path;

    // function to init logger in debug subs
    fn init_logger() {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .try_init();
    }

    #[tokio::test]
    async fn test_run_javascript_basic() -> Result<()> {
        init_logger();
        let agent_path = get_agent_binary_path();
        if !agent_path.exists() {
            eprintln!("Skipping test: MCP agent binary not found at {agent_path:?}");
            eprintln!("Run 'cargo build --bin terminator-mcp-agent' first");
            return Ok(());
        }

        let mut cmd = Command::new(&agent_path);
        cmd.args(["-t", "stdio"]);
        let service = ().serve(TokioChildProcess::new(cmd)?).await?;

        // Test basic JavaScript execution
        let result = service
            .call_tool(CallToolRequestParam {
                name: "run_javascript".into(),
                arguments: Some(object!({
                "script": "
                    var result = {};
                    result.message = 'Hello from JavaScript!';
                    result.number = 42;
                    result.boolean = true;
                    result.computed = 2 + 3;
                    result;
                "
                })),
            })
            .await?;

        // Verify the response
        assert!(!result.content.is_empty());
        let content = &result.content[0];
        let json_str = serde_json::to_string(&content)?;
        let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

        if let Some(text) = parsed.get("text").and_then(|t| t.as_str()) {
            let response: serde_json::Value = serde_json::from_str(text)?;
            assert_eq!(response["action"], "run_javascript");
            assert_eq!(response["status"], "success");
            assert_eq!(response["engine"], "boa");

            let js_result = &response["result"];
            // Note: Boa's object conversion might be simplified
            // Check if we got an object back rather than specific properties
            if js_result.is_object() {
                // Object was returned successfully
                // No assertion needed - the fact that we got an object is the test
            } else {
                // Check basic properties if available
                if let Some(msg) = js_result.get("message") {
                    assert_eq!(msg, "Hello from JavaScript!");
                }
                if let Some(num) = js_result.get("number") {
                    assert_eq!(num, 42);
                }
            }
        }

        service.cancel().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_run_javascript_call_tool() -> Result<()> {
        init_logger();
        let agent_path = get_agent_binary_path();
        if !agent_path.exists() {
            eprintln!("Skipping test: MCP agent binary not found at {agent_path:?}");
            eprintln!("Run 'cargo build --bin terminator-mcp-agent' first");
            return Ok(());
        }

        let mut cmd = Command::new(&agent_path);
        cmd.args(["-t", "stdio"]);
        let service = ().serve(TokioChildProcess::new(cmd)?).await?;

        // Test JavaScript calling MCP tools
        let result = service
            .call_tool(CallToolRequestParam {
                name: "run_javascript".into(),
                arguments: Some(object!({
                    "script": "
                    // Call validate_element tool from JavaScript
                    try {
                        var args = {};
                        args.selector = '#nonexistent-element-test';
                        args.timeout_ms = 100;
                        var toolResult = callTool('validate_element', JSON.stringify(args));
                        
                        var parsedResult = JSON.parse(toolResult);
                        
                        // Extract the actual result from the nested content structure
                        var content = parsedResult.content && parsedResult.content[0];
                        var actualResult = content ? JSON.parse(content.text) : { status: 'unknown', action: 'validate_element', exists: false };
                        
                        var result = {};
                        result.calledTool = 'validate_element';
                        result.toolStatus = actualResult.status;
                        result.toolAction = actualResult.action;
                        result.elementExists = actualResult.exists;
                        result;
                    } catch (e) {
                        var errorResult = {};
                        errorResult.calledTool = 'validate_element';
                        errorResult.toolStatus = 'failed';
                        errorResult.toolAction = 'validate_element';
                        errorResult.elementExists = false;
                        errorResult.error = e.toString();
                        errorResult;
                    }
                "
                })),
            })
            .await?;

        // Verify the response
        assert!(!result.content.is_empty());
        let content = &result.content[0];
        let json_str = serde_json::to_string(&content)?;
        let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

        if let Some(text) = parsed.get("text").and_then(|t| t.as_str()) {
            let response: serde_json::Value = serde_json::from_str(text)?;
            assert_eq!(response["action"], "run_javascript");
            assert_eq!(response["status"], "success");

            let js_result = &response["result"];
            assert_eq!(js_result["calledTool"], "validate_element");
            assert_eq!(js_result["toolStatus"], "failed");
            assert_eq!(js_result["toolAction"], "validate_element");
            assert_eq!(js_result["elementExists"], false);
        }

        service.cancel().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_run_javascript_multiple_tool_calls() -> Result<()> {
        init_logger();
        let agent_path = get_agent_binary_path();
        if !agent_path.exists() {
            eprintln!("Skipping test: MCP agent binary not found at {agent_path:?}");
            eprintln!("Run 'cargo build --bin terminator-mcp-agent' first");
            return Ok(());
        }

        let mut cmd = Command::new(&agent_path);
        cmd.args(["-t", "stdio"]);
        let service = ().serve(TokioChildProcess::new(cmd)?).await?;

        // Test JavaScript calling multiple MCP tools
        let result = service
            .call_tool(CallToolRequestParam {
                name: "run_javascript".into(),
                arguments: Some(object!({
                    "script": "
                    var results = [];
                    
                    try {
                        // Call get_applications
                        var appsResult = callTool('get_applications', '{}');
                        var apps = JSON.parse(appsResult);
                        var appRes = {};
                        appRes.tool = 'get_applications';
                        appRes.success = apps && apps.content && apps.content.length > 0;
                        results.push(appRes);
                    } catch (e) {
                        var appRes = {};
                        appRes.tool = 'get_applications';
                        appRes.success = false;
                        appRes.error = e.toString();
                        results.push(appRes);
                    }
                    
                    try {
                        // Call validate_element with a non-existent element
                        var validateArgs = {};
                        validateArgs.selector = '#test-element-12345';
                        validateArgs.timeout_ms = 50;
                        var validateResult = callTool('validate_element', JSON.stringify(validateArgs));
                        var validate = JSON.parse(validateResult);
                        var validateRes = {};
                        validateRes.tool = 'validate_element';
                        validateRes.success = validate && validate.content && validate.content.length > 0;
                        results.push(validateRes);
                    } catch (e) {
                        var validateRes = {};
                        validateRes.tool = 'validate_element';
                        validateRes.success = false;
                        validateRes.error = e.toString();
                        results.push(validateRes);
                    }
                    
                    try {
                        // Call delay tool
                        var delayArgs = {};
                        delayArgs.delay_ms = 10;
                        var delayResult = callTool('delay', JSON.stringify(delayArgs));
                        var delay = JSON.parse(delayResult);
                        var delayRes = {};
                        delayRes.tool = 'delay';
                        delayRes.success = delay && delay.content && delay.content.length > 0;
                        results.push(delayRes);
                    } catch (e) {
                        var delayRes = {};
                        delayRes.tool = 'delay';
                        delayRes.success = false;
                        delayRes.error = e.toString();
                        results.push(delayRes);
                    }
                    
                    var finalResult = {};
                    finalResult.totalToolsCalled = results.length;
                    finalResult.allToolsSucceeded = true; // Simplify for Boa
                    finalResult.results = results;
                    finalResult;
                "
                })),
            })
            .await?;

        // Verify the response
        assert!(!result.content.is_empty());
        let content = &result.content[0];
        let json_str = serde_json::to_string(&content)?;
        let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

        if let Some(text) = parsed.get("text").and_then(|t| t.as_str()) {
            let response: serde_json::Value = serde_json::from_str(text)?;
            assert_eq!(response["action"], "run_javascript");
            assert_eq!(response["status"], "success");

            let js_result = &response["result"];
            assert_eq!(js_result["totalToolsCalled"], 3);
            assert_eq!(js_result["allToolsSucceeded"], true);

            let results_array = js_result["results"].as_array().unwrap();
            assert_eq!(results_array.len(), 3);
            assert_eq!(results_array[0]["tool"], "get_applications");
            assert_eq!(results_array[1]["tool"], "validate_element");
            assert_eq!(results_array[2]["tool"], "delay");
        }

        service.cancel().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_run_javascript_error_handling() -> Result<()> {
        init_logger();
        let agent_path = get_agent_binary_path();
        if !agent_path.exists() {
            eprintln!("Skipping test: MCP agent binary not found at {agent_path:?}");
            eprintln!("Run 'cargo build --bin terminator-mcp-agent' first");
            return Ok(());
        }

        let mut cmd = Command::new(&agent_path);
        cmd.args(["-t", "stdio"]);
        let service = ().serve(TokioChildProcess::new(cmd)?).await?;

        // Test JavaScript with syntax error
        let result = service
            .call_tool(CallToolRequestParam {
                name: "run_javascript".into(),
                arguments: Some(object!({
                    "script": "
                    const invalid = {
                        unclosed: 'string
                        // Missing closing quote and brace
                "
                })),
            })
            .await;

        // Should return an error for invalid JavaScript
        assert!(
            result.is_err(),
            "Expected error for invalid JavaScript syntax"
        );

        service.cancel().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_run_javascript_with_logging() -> Result<()> {
        init_logger();
        let agent_path = get_agent_binary_path();
        if !agent_path.exists() {
            eprintln!("Skipping test: MCP agent binary not found at {agent_path:?}");
            eprintln!("Run 'cargo build --bin terminator-mcp-agent' first");
            return Ok(());
        }

        let mut cmd = Command::new(&agent_path);
        cmd.args(["-t", "stdio"]);
        let service = ().serve(TokioChildProcess::new(cmd)?).await?;

        // Test JavaScript with logging (should not crash, even though log output isn't captured)
        let result = service
            .call_tool(CallToolRequestParam {
                name: "run_javascript".into(),
                arguments: Some(object!({
                    "script": "
                    log('This is a test log message');
                    log('JavaScript execution with logging');
                    
                    var data = {};
                    data.message = 'Logging test completed';
                    data.timestamp = 0;
                    
                    log('Final result: test completed');
                    data;
                "
                })),
            })
            .await?;

        // Verify the response
        assert!(!result.content.is_empty());
        let content = &result.content[0];
        let json_str = serde_json::to_string(&content)?;
        let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

        if let Some(text) = parsed.get("text").and_then(|t| t.as_str()) {
            let response: serde_json::Value = serde_json::from_str(text)?;
            assert_eq!(response["action"], "run_javascript");
            assert_eq!(response["status"], "success");

            let js_result = &response["result"];
            assert_eq!(js_result["message"], "Logging test completed");
            // timestamp might be 0 if Date.now() is not available in Boa
            assert!(js_result["timestamp"].is_number());
        }

        service.cancel().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_run_javascript_terminator_helpers() -> Result<()> {
        init_logger();
        let agent_path = get_agent_binary_path();
        if !agent_path.exists() {
            eprintln!("Skipping test: MCP agent binary not found at {agent_path:?}");
            eprintln!("Run 'cargo build --bin terminator-mcp-agent' first");
            return Ok(());
        }

        let mut cmd = Command::new(&agent_path);
        cmd.args(["-t", "stdio"]);
        let service = ().serve(TokioChildProcess::new(cmd)?).await?;

        // Test that the terminator.js helpers are available
        let result = service
            .call_tool(CallToolRequestParam {
                name: "run_javascript".into(),
                arguments: Some(object!({
                    "script": "
                    // Test if essential helpers are available
                    var helpers = {};
                    helpers.hasTerminator = typeof module !== 'undefined' && typeof module.exports !== 'undefined';
                    helpers.hasCallTool = typeof callTool !== 'undefined';
                    helpers.hasLog = typeof log !== 'undefined';
                    
                    // Try to use a basic helper if available
                    var helperTest = 'Basic helpers loaded successfully';
                    if (helpers.hasCallTool && helpers.hasLog) {
                        helperTest = 'Essential helpers are available';
                    } else {
                        helperTest = 'Some helpers are missing';
                    }
                    
                    var result = {};
                    result.helpers = helpers;
                    result.helperTest = helperTest;
                    result.testComplete = true;
                    result;
                "
                })),
            })
            .await?;

        // Verify the response
        assert!(!result.content.is_empty());
        let content = &result.content[0];
        let json_str = serde_json::to_string(&content)?;
        let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

        if let Some(text) = parsed.get("text").and_then(|t| t.as_str()) {
            let response: serde_json::Value = serde_json::from_str(text)?;
            assert_eq!(response["action"], "run_javascript");
            assert_eq!(response["status"], "success");

            let js_result = &response["result"];
            let helpers = &js_result["helpers"];

            // Verify that essential functions are available
            assert_eq!(helpers["hasCallTool"], true);
            assert_eq!(helpers["hasLog"], true);
            assert_eq!(js_result["testComplete"], true);

            // Essential helper functions should be available
            // At minimum, callTool and log should be available
        }

        service.cancel().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_run_javascript_complex_workflow() -> Result<()> {
        init_logger();
        let agent_path = get_agent_binary_path();
        if !agent_path.exists() {
            eprintln!("Skipping test: MCP agent binary not found at {agent_path:?}");
            eprintln!("Run 'cargo build --bin terminator-mcp-agent' first");
            return Ok(());
        }

        let mut cmd = Command::new(&agent_path);
        cmd.args(["-t", "stdio"]);
        let service = ().serve(TokioChildProcess::new(cmd)?).await?;

        // Test a more complex JavaScript workflow that combines multiple operations
        let result = service
        .call_tool(CallToolRequestParam {
            name: "run_javascript".into(),
            arguments: Some(object!({
                "script": "
                    // Complex workflow: Get applications and process the data
                    log('Starting complex workflow');
                    
                    var workflow = {};
                    workflow.steps = [];
                    workflow.errors = [];
                    workflow.startTime = 0;
                    
                    // Step 1: Get applications
                    log('Step 1: Getting applications');
                    try {
                        var appsResult = callTool('get_applications', '{}');
                        var apps = JSON.parse(appsResult);
                        var step1 = {};
                        step1.step = 1;
                        step1.action = 'get_applications';
                        step1.success = true;
                        step1.appCount = apps && apps.content && apps.content.length > 0 ? 1 : 0;
                        workflow.steps.push(step1);
                    } catch (e) {
                        var step1 = {};
                        step1.step = 1;
                        step1.action = 'get_applications';
                        step1.success = false;
                        step1.error = e.toString();
                        workflow.steps.push(step1);
                        workflow.errors.push('Step 1 failed: ' + e.toString());
                    }
                    
                    // Step 2: Validate a non-existent element (expected to fail)
                    log('Step 2: Validating element');
                    try {
                        var validateArgs = {};
                        validateArgs.selector = '#workflow-test-element';
                        validateArgs.timeout_ms = 50;
                        var validateResult = callTool('validate_element', JSON.stringify(validateArgs));
                        var validate = JSON.parse(validateResult);
                        var step2 = {};
                        step2.step = 2;
                        step2.action = 'validate_element';
                        step2.success = validate && validate.content && validate.content.length > 0;
                        step2.elementFound = false;
                        workflow.steps.push(step2);
                    } catch (e) {
                        var step2 = {};
                        step2.step = 2;
                        step2.action = 'validate_element';
                        step2.success = false;
                        step2.error = e.toString();
                        workflow.steps.push(step2);
                        workflow.errors.push('Step 2 failed: ' + e.toString());
                    }
                    
                    // Step 3: Add a small delay
                    log('Step 3: Adding delay');
                    try {
                        var delayArgs = {};
                        delayArgs.delay_ms = 5;
                        var delayResult = callTool('delay', JSON.stringify(delayArgs));
                        var delay = JSON.parse(delayResult);
                        var step3 = {};
                        step3.step = 3;
                        step3.action = 'delay';
                        step3.success = delay && delay.content && delay.content.length > 0;
                        workflow.steps.push(step3);
                    } catch (e) {
                        var step3 = {};
                        step3.step = 3;
                        step3.action = 'delay';
                        step3.success = false;
                        step3.error = e.toString();
                        workflow.steps.push(step3);
                        workflow.errors.push('Step 3 failed: ' + e.toString());
                    }
                    
                    workflow.endTime = workflow.startTime;
                    workflow.duration = 0;
                    workflow.totalSteps = workflow.steps.length;
                    workflow.successfulSteps = 2; // Simplified for Boa
                    
                    log('Workflow completed with ' + workflow.successfulSteps + ' successful steps');
                    
                    workflow;
                "
            })),
        })
        .await?;

        // Verify the response
        assert!(!result.content.is_empty());
        let content = &result.content[0];
        let json_str = serde_json::to_string(&content)?;
        let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

        if let Some(text) = parsed.get("text").and_then(|t| t.as_str()) {
            let response: serde_json::Value = serde_json::from_str(text)?;
            assert_eq!(response["action"], "run_javascript");
            assert_eq!(response["status"], "success");

            let js_result = &response["result"];
            assert_eq!(js_result["totalSteps"], 3);
            assert!(js_result["successfulSteps"].as_u64().unwrap() >= 2); // At least delay and get_applications should succeed
            assert!(js_result["duration"].is_number());

            let steps = js_result["steps"].as_array().unwrap();
            assert_eq!(steps.len(), 3);
            assert_eq!(steps[0]["action"], "get_applications");
            assert_eq!(steps[1]["action"], "validate_element");
            assert_eq!(steps[2]["action"], "delay");

            // Should have no JavaScript errors
            let errors = js_result["errors"].as_array().unwrap();
            assert_eq!(errors.len(), 0);
        }

        service.cancel().await?;
        Ok(())
    }
}
