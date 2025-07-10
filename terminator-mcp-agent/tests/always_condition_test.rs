use rmcp::handler::server::tool::Parameters;
use serde_json::json;
use terminator_mcp_agent::server::DesktopWrapper;
use terminator_mcp_agent::utils::ExecuteSequenceArgs;

#[tokio::test]
async fn test_always_condition_execution() {
    // Create a test workflow where the second step fails but the third step has always=true
    let workflow_args = ExecuteSequenceArgs {
        steps: vec![
            // Step 1: Should succeed
            serde_json::from_value(json!({
                "tool_name": "delay",
                "arguments": {
                    "delay_ms": 10
                }
            }))
            .unwrap(),
            // Step 2: Will fail (invalid selector)
            serde_json::from_value(json!({
                "tool_name": "click_element",
                "arguments": {
                    "selector": "invalid|selector|that|will|fail",
                    "timeout_ms": 100
                }
            }))
            .unwrap(),
            // Step 3: Should run despite step 2 failing because always=true
            serde_json::from_value(json!({
                "tool_name": "delay",
                "arguments": {
                    "delay_ms": 10
                },
                "always": true
            }))
            .unwrap(),
            // Step 4: Should NOT run because it doesn't have always=true
            serde_json::from_value(json!({
                "tool_name": "delay",
                "arguments": {
                    "delay_ms": 10
                }
            }))
            .unwrap(),
        ],
        variables: None,
        inputs: None,
        selectors: None,
        stop_on_error: Some(true),
        include_detailed_results: Some(true),
        output_parser: None,
    };

    // Execute the workflow
    let desktop_wrapper = DesktopWrapper::new()
        .await
        .expect("Failed to create desktop wrapper");
    let result = desktop_wrapper
        .execute_sequence(Parameters(workflow_args))
        .await;

    // Verify the result
    assert!(
        result.is_ok(),
        "Workflow execution should complete successfully"
    );

    let content = &result.unwrap().content;
    assert_eq!(content.len(), 1, "Should have one content item");

    // Parse the result JSON
    let result_json: serde_json::Value = match &content[0] {
        rmcp::model::Content::Text { text, .. } => serde_json::from_str(text).unwrap(),
        _ => panic!("Expected text content"),
    };

    // Verify that we executed exactly 3 tools (step 4 should not run)
    let executed_tools = result_json["executed_tools"].as_u64().unwrap();
    assert_eq!(executed_tools, 3, "Should have executed exactly 3 tools");

    // Verify the results array
    let results = result_json["results"].as_array().unwrap();
    assert_eq!(results.len(), 3, "Should have 3 results");

    // Step 1 should succeed
    assert_eq!(results[0]["status"], "success", "Step 1 should succeed");

    // Step 2 should fail
    assert_eq!(results[1]["status"], "error", "Step 2 should fail");

    // Step 3 should succeed (ran because of always=true)
    assert_eq!(
        results[2]["status"], "success",
        "Step 3 should succeed due to always=true"
    );

    // Step 4 should not be in results (didn't run)
    // The status should indicate completion with errors since step 2 failed
    let final_status = result_json["status"].as_str().unwrap();
    assert_eq!(
        final_status, "partial_success",
        "Final status should be partial_success"
    );
}

#[tokio::test]
async fn test_always_condition_in_groups() {
    // Test that always condition works within groups
    let workflow_args = ExecuteSequenceArgs {
        steps: vec![
            // Group with mixed always conditions
            serde_json::from_value(json!({
                "group_name": "test_group",
                "skippable": false,
                "steps": [
                    {
                        "tool_name": "delay",
                        "arguments": { "delay_ms": 10 }
                    },
                    {
                        "tool_name": "click_element",
                        "arguments": {
                            "selector": "invalid|selector",
                            "timeout_ms": 100
                        }
                    },
                    {
                        "tool_name": "delay",
                        "arguments": { "delay_ms": 10 },
                        "always": true
                    }
                ]
            }))
            .unwrap(),
        ],
        variables: None,
        inputs: None,
        selectors: None,
        stop_on_error: Some(true),
        include_detailed_results: Some(true),
        output_parser: None,
    };

    let desktop_wrapper = DesktopWrapper::new()
        .await
        .expect("Failed to create desktop wrapper");
    let result = desktop_wrapper
        .execute_sequence(Parameters(workflow_args))
        .await;

    assert!(result.is_ok(), "Workflow execution should complete");

    let content = &result.unwrap().content;
    let result_json: serde_json::Value = match &content[0] {
        rmcp::model::Content::Text { text, .. } => serde_json::from_str(text).unwrap(),
        _ => panic!("Expected text content"),
    };

    let results = result_json["results"].as_array().unwrap();
    let group_results = results[0]["results"].as_array().unwrap();

    // All 3 tools in the group should have been executed
    assert_eq!(
        group_results.len(),
        3,
        "All 3 tools in group should execute"
    );
    assert_eq!(
        group_results[2]["status"], "success",
        "Third tool with always=true should succeed"
    );
}
