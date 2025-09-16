// Example demonstrating OpenTelemetry integration
// Run with: cargo run --example test_telemetry --features telemetry

#[cfg(feature = "telemetry")]
use terminator_mcp_agent::telemetry::{StepSpan, WorkflowSpan};

#[cfg(feature = "telemetry")]
#[tokio::main]
async fn main() {
    // DO NOT disable - we want to send real telemetry
    // std::env::set_var("OTEL_SDK_DISABLED", "true");

    // Initialize telemetry
    if let Err(e) = terminator_mcp_agent::telemetry::init_telemetry() {
        eprintln!("Failed to initialize telemetry: {e}");
        return;
    }

    println!("OpenTelemetry integration test");
    println!("================================");

    // Simulate a workflow execution
    let mut workflow = WorkflowSpan::new("example_workflow");
    workflow.set_attribute("workflow_file", "examples/test.yml".to_string());
    workflow.set_attribute("total_steps", "3".to_string());

    println!("Starting workflow: example_workflow");

    // Step 1: Navigate
    {
        let mut step = StepSpan::new("navigate", Some("nav_001"));
        step.set_attribute("url", "https://example.com".to_string());
        println!("  Step 1: Navigating to https://example.com");
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        step.set_status(true, None);
        step.end();
        println!("  ✓ Navigation complete");
    }

    // Step 2: Type text
    {
        let mut step = StepSpan::new("type_text", Some("type_002"));
        step.set_attribute("selector", "#search".to_string());
        step.set_attribute("text", "OpenTelemetry test".to_string());
        println!("  Step 2: Typing text into #search");
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        step.set_status(true, None);
        step.end();
        println!("  ✓ Text typed successfully");
    }

    // Step 3: Click (fails)
    {
        let mut step = StepSpan::new("click", Some("click_003"));
        step.set_attribute("selector", "#nonexistent".to_string());
        println!("  Step 3: Clicking #nonexistent");
        tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
        let error = "Element not found: #nonexistent";
        step.set_status(false, Some(error));
        step.end();
        println!("  ✗ Click failed: {error}");
    }

    // Add workflow event
    workflow.add_event(
        "workflow_failed",
        vec![
            ("failed_at_step", "3".to_string()),
            ("error", "Element not found".to_string()),
        ],
    );

    // Mark workflow as failed
    workflow.set_status(false, "Workflow failed at step 3: Element not found");
    workflow.end();

    println!("\nWorkflow completed with errors");
    println!("Total duration: ~300ms");

    // Shutdown telemetry
    terminator_mcp_agent::telemetry::shutdown_telemetry();

    // Give time for traces to be exported
    println!("\nWaiting for traces to be exported...");
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    println!("\nTelemetry test completed successfully!");
    println!("Traces sent to: {}", std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").unwrap_or_else(|_| "http://localhost:4318".to_string()));
}

#[cfg(not(feature = "telemetry"))]
fn main() {
    println!("This example requires the 'telemetry' feature.");
    println!("Run with: cargo run --example test_telemetry --features telemetry");
}
