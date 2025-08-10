use terminator_workflow_recorder::{ClickEvent, EventMetadata, McpConverter, WorkflowEvent};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    info!("🧪 Testing Chrome-Specific MCP Converter Fix");
    info!("===========================================");

    let converter = McpConverter::new();

    // Create a synthetic Chrome click event to test our fix
    let chrome_metadata = EventMetadata::with_timestamp();

    let chrome_click = ClickEvent {
        element_text: "Search".to_string(),
        interaction_type: terminator_workflow_recorder::ButtonInteractionType::Click,
        element_role: "Text".to_string(),
        was_enabled: true,
        click_position: Some(terminator_workflow_recorder::Position { x: 100, y: 200 }),
        element_description: None,
        child_text_content: vec![],
        metadata: chrome_metadata,
    };

    // Test our Chrome fix
    info!("🔍 Testing Chrome application click conversion...");

    let workflow_event = WorkflowEvent::Click(chrome_click);
    let conversion = converter.convert_event(&workflow_event, None).await?;
    let mcp_sequence = conversion.primary_sequence;

    info!("📊 Generated MCP Sequence:");
    for (i, step) in mcp_sequence.iter().enumerate() {
        info!(
            "  Step {}: {} with selector: '{}'",
            i + 1,
            step.tool_name,
            step.arguments
                .get("selector")
                .unwrap_or(&serde_json::Value::String("N/A".to_string()))
        );

        // Check if our Chrome fix worked
        if let Some(selector) = step.arguments.get("selector").and_then(|s| s.as_str()) {
            if selector.contains("role:Pane") {
                info!("  ✅ SUCCESS! Chrome fix worked - using role:Pane");
            } else if selector.contains("role:Window") {
                info!("  ❌ FAILED! Still using role:Window for Chrome");
            }
        }
    }

    // Test a non-Chrome application for comparison
    info!("");
    info!("🔍 Testing non-Chrome application click conversion (for comparison)...");

    let notepad_metadata = EventMetadata::with_timestamp();

    let notepad_click = ClickEvent {
        element_text: "Text Editor".to_string(),
        interaction_type: terminator_workflow_recorder::ButtonInteractionType::Click,
        element_role: "Document".to_string(),
        was_enabled: true,
        click_position: Some(terminator_workflow_recorder::Position { x: 100, y: 200 }),
        element_description: None,
        child_text_content: vec![],
        metadata: notepad_metadata,
    };

    let notepad_event = WorkflowEvent::Click(notepad_click);
    let notepad_conversion = converter.convert_event(&notepad_event, None).await?;
    let notepad_sequence = notepad_conversion.primary_sequence;

    info!("📊 Generated MCP Sequence for Notepad:");
    for (i, step) in notepad_sequence.iter().enumerate() {
        info!(
            "  Step {}: {} with selector: '{}'",
            i + 1,
            step.tool_name,
            step.arguments
                .get("selector")
                .unwrap_or(&serde_json::Value::String("N/A".to_string()))
        );

        // This should still use Window for non-Chrome apps
        if let Some(selector) = step.arguments.get("selector").and_then(|s| s.as_str()) {
            if selector.contains("role:Window") {
                info!("  ✅ CORRECT! Non-Chrome app using role:Window as expected");
            } else if selector.contains("role:Pane") {
                info!("  ❌ UNEXPECTED! Non-Chrome app using role:Pane");
            }
        }
    }

    info!("");
    info!("🏁 Chrome Fix Test Complete!");

    Ok(())
}
