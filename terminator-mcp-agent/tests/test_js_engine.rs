use serde_json::json;
use terminator_mcp_agent::scripting_engine;

#[tokio::test]
async fn test_javascript_engine_basic() {
    // Test basic JavaScript execution with the new 'run' parameter
    let script = "return {success: true, value: 42};".to_string();
    let result = scripting_engine::execute_javascript_with_nodejs(script)
        .await
        .expect("JavaScript execution should succeed");

    assert_eq!(result["success"], true);
    assert_eq!(result["value"], 42);
}

#[tokio::test]
async fn test_javascript_engine_with_async() {
    // Test async JavaScript execution
    let script = r#"
        await sleep(100);
        return {delayed: true, timestamp: Date.now()};
    "#
    .to_string();

    let result = scripting_engine::execute_javascript_with_nodejs(script)
        .await
        .expect("Async JavaScript execution should succeed");

    assert_eq!(result["delayed"], true);
    assert!(result["timestamp"].is_number());
}

#[tokio::test]
async fn test_javascript_engine_with_desktop_api() {
    // Test that desktop API is available (may fail if no UI elements present)
    let script = r#"
        // Just check that desktop object exists
        return {
            hasDesktop: typeof desktop !== 'undefined',
            hasLocator: typeof desktop?.locator === 'function'
        };
    "#
    .to_string();

    let result = scripting_engine::execute_javascript_with_nodejs(script)
        .await
        .expect("Desktop API check should succeed");

    assert_eq!(result["hasDesktop"], true);
    assert_eq!(result["hasLocator"], true);
}
