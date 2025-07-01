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
    path.push("terminator-mcp-agent");
    #[cfg(target_os = "windows")]
    path.set_extension("exe");
    path
}

#[tokio::test]
async fn test_mcp_client_list_tools() -> Result<()> {
    let agent_path = get_agent_binary_path();
    if !agent_path.exists() {
        eprintln!(
            "Skipping test: MCP agent binary not found at {:?}",
            agent_path
        );
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
        eprintln!(
            "Skipping test: MCP agent binary not found at {:?}",
            agent_path
        );
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
                "tools_json": "[]",
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
        eprintln!(
            "Skipping test: MCP agent binary not found at {:?}",
            agent_path
        );
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
        eprintln!(
            "Skipping test: MCP agent binary not found at {:?}",
            agent_path
        );
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
                "tools_json": serde_json::to_string(&vec![
                    object!({
                        "tool_name": "invalid_tool",
                        "arguments": {},
                        "continue_on_error": true
                    }),
                    object!({
                        "tool_name": "validate_element",
                        "arguments": {
                            "selector": "#test-element",
                            "timeout_ms": 50
                        }
                    })
                ]).unwrap(),
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
        assert_eq!(results[0]["status"], "error");
        assert_eq!(results[1]["status"], "success");
    }

    service.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_execute_sequence_stop_on_error() -> Result<()> {
    let agent_path = get_agent_binary_path();
    if !agent_path.exists() {
        eprintln!(
            "Skipping test: MCP agent binary not found at {:?}",
            agent_path
        );
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
                "tools_json": serde_json::to_string(&vec![
                    object!({
                        "tool_name": "invalid_tool",
                        "arguments": {},
                        "continue_on_error": false
                    }),
                    object!({
                        "tool_name": "validate_element",
                        "arguments": {
                            "selector": "#should-not-execute",
                            "timeout_ms": 50
                        }
                    })
                ]).unwrap(),
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
        assert_eq!(response["executed_tools"], 1); // Should stop after first error
    }

    service.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_server_info() -> Result<()> {
    let agent_path = get_agent_binary_path();
    if !agent_path.exists() {
        eprintln!(
            "Skipping test: MCP agent binary not found at {:?}",
            agent_path
        );
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
        eprintln!(
            "Skipping test: MCP agent binary not found at {:?}",
            agent_path
        );
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
                "tools_json": serde_json::to_string(&vec![
                    object!({
                        "tool_name": "validate_element",
                        "arguments": {
                            "selector": "#test1",
                            "timeout_ms": 50
                        },
                        "delay_ms": 100
                    }),
                    object!({
                        "tool_name": "validate_element",
                        "arguments": {
                            "selector": "#test2",
                            "timeout_ms": 50
                        }
                    })
                ]).unwrap(),
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
        eprintln!(
            "Skipping test: MCP agent binary not found at {:?}",
            agent_path
        );
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
        eprintln!(
            "Skipping test: MCP agent binary not found at {:?}",
            agent_path
        );
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
