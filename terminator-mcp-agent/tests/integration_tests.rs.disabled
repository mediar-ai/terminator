use rmcp::handler::server::tool::Parameters;
use serde_json::json;
use terminator_mcp_agent::server::DesktopWrapper;
use terminator_mcp_agent::utils::{ExecuteSequenceArgs, ToolCall, ValidateElementArgs};

/// Helper function to extract JSON from tool response
fn extract_json_from_content(content: &[rmcp::model::Content]) -> Option<serde_json::Value> {
    content.first().and_then(|c| {
        // Serialize the content to JSON to extract the text field
        let content_json = serde_json::to_value(c).ok()?;
        content_json
            .get("text")
            .and_then(|t| t.as_str())
            .and_then(|text| serde_json::from_str(text).ok())
    })
}

#[tokio::test]
async fn test_execute_sequence_direct() {
    let desktop = match DesktopWrapper::new().await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Skipping test due to Desktop initialization failure: {}", e);
            return;
        }
    };

    // Test execute_sequence with empty tools array
    let args = ExecuteSequenceArgs {
        tools_json: serde_json::to_string(&Vec::<ToolCall>::new()).unwrap(),
        stop_on_error: Some(true),
        include_detailed_results: Some(true),
    };

    let result = desktop.execute_sequence(Parameters(args)).await;
    assert!(result.is_ok(), "execute_sequence failed");

    let call_result = result.unwrap();
    assert!(!call_result.content.is_empty(), "No content returned");

    // Verify the response
    let parsed = extract_json_from_content(&call_result.content)
        .expect("Failed to extract JSON from response");

    assert_eq!(parsed["action"], "execute_sequence");
    assert_eq!(parsed["status"], "success");
    assert_eq!(parsed["total_tools"], 0);
    assert_eq!(parsed["executed_tools"], 0);
}

#[tokio::test]
async fn test_validate_element_direct() {
    let desktop = match DesktopWrapper::new().await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Skipping test due to Desktop initialization failure: {}", e);
            return;
        }
    };

    // Test validate_element with a non-existent element
    let args = ValidateElementArgs {
        selector: "#99999999999".to_string(),
        alternative_selectors: None,
        timeout_ms: Some(100),
        include_tree: Some(false),
    };

    let result = desktop.validate_element(Parameters(args)).await;
    assert!(
        result.is_ok(),
        "validate_element should handle not found gracefully"
    );

    let call_result = result.unwrap();
    let parsed = extract_json_from_content(&call_result.content)
        .expect("Failed to extract JSON from response");

    assert_eq!(parsed["action"], "validate_element");
    assert_eq!(parsed["status"], "failed");
    assert_eq!(parsed["exists"], false);
}

#[tokio::test]
async fn test_execute_sequence_with_invalid_tool() {
    let desktop = match DesktopWrapper::new().await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Skipping test due to Desktop initialization failure: {}", e);
            return;
        }
    };

    // Test execute_sequence with an invalid tool
    let args = ExecuteSequenceArgs {
        tools_json: serde_json::to_string(&vec![ToolCall {
            tool_name: "non_existent_tool".to_string(),
            arguments: json!({}),
            continue_on_error: None,
            delay_ms: None,
        }])
        .unwrap(),
        stop_on_error: Some(true),
        include_detailed_results: Some(true),
    };

    let result = desktop.execute_sequence(Parameters(args)).await;
    assert!(
        result.is_ok(),
        "execute_sequence should handle invalid tools gracefully"
    );

    let call_result = result.unwrap();
    let parsed = extract_json_from_content(&call_result.content)
        .expect("Failed to extract JSON from response");

    assert_eq!(parsed["action"], "execute_sequence");
    assert_eq!(parsed["status"], "partial_success");
    assert_eq!(parsed["total_tools"], 1);
    assert_eq!(parsed["executed_tools"], 1);

    // Check that the error was captured
    let results = parsed["results"]
        .as_array()
        .expect("Expected results array");
    let first_result = &results[0];
    assert_eq!(first_result["status"], "error");
    assert!(first_result["error"]
        .as_str()
        .unwrap()
        .contains("Unknown tool"));
}

#[tokio::test]
async fn test_complex_sequence_direct() {
    let desktop = match DesktopWrapper::new().await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Skipping test due to Desktop initialization failure: {}", e);
            return;
        }
    };

    // Test a more complex sequence
    let args = ExecuteSequenceArgs {
        tools_json: serde_json::to_string(&vec![
            ToolCall {
                tool_name: "invalid_tool".to_string(),
                arguments: json!({}),
                continue_on_error: Some(true),
                delay_ms: None,
            },
            ToolCall {
                tool_name: "validate_element".to_string(),
                arguments: json!({
                    "selector": "#test-element",
                    "timeout_ms": 50
                }),
                continue_on_error: None,
                delay_ms: None,
            },
        ])
        .unwrap(),
        stop_on_error: Some(true),
        include_detailed_results: Some(true),
    };

    let result = desktop.execute_sequence(Parameters(args)).await;
    assert!(result.is_ok(), "execute_sequence failed");

    let call_result = result.unwrap();
    let parsed = extract_json_from_content(&call_result.content)
        .expect("Failed to extract JSON from response");

    // Both tools should have executed
    assert_eq!(parsed["total_tools"], 2);
    assert_eq!(parsed["executed_tools"], 2);

    let results = parsed["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);

    // First tool should have failed
    assert_eq!(results[0]["status"], "error");
    // Second tool should have succeeded
    assert_eq!(results[1]["status"], "success");
}

#[tokio::test]
async fn test_execute_sequence_delays() {
    let desktop = match DesktopWrapper::new().await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Skipping test due to Desktop initialization failure: {}", e);
            return;
        }
    };

    // Test that delays are respected
    let start_time = std::time::Instant::now();

    let args = ExecuteSequenceArgs {
        tools_json: serde_json::to_string(&vec![
            ToolCall {
                tool_name: "validate_element".to_string(),
                arguments: json!({
                    "selector": "#test-delay-1",
                    "timeout_ms": 50
                }),
                continue_on_error: None,
                delay_ms: Some(100),
            },
            ToolCall {
                tool_name: "validate_element".to_string(),
                arguments: json!({
                    "selector": "#test-delay-2",
                    "timeout_ms": 50
                }),
                continue_on_error: None,
                delay_ms: None,
            },
        ])
        .unwrap(),
        stop_on_error: Some(true),
        include_detailed_results: Some(false),
    };

    let result = desktop.execute_sequence(Parameters(args)).await;
    assert!(result.is_ok());

    let elapsed = start_time.elapsed();
    // Should have at least 100ms delay after first tool
    assert!(
        elapsed.as_millis() >= 100,
        "Delays not properly applied: {}ms",
        elapsed.as_millis()
    );
}
