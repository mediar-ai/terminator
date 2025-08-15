use std::time::Instant;
use terminator_workflow_recorder::{
    ApplicationSwitchEvent, ApplicationSwitchMethod, EventMetadata, McpConverter, WorkflowEvent,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("🧪 Testing fast activate_element generation");
    println!("============================================");

    // Create an MCP converter
    let converter = McpConverter::new();

    // Create a test application switch event (like switching to Chrome)
    let app_switch_event = ApplicationSwitchEvent {
        from_application: Some("Cursor".to_string()),
        to_application: "Google Chrome".to_string(),
        from_process_id: Some(1234),
        to_process_id: 5678,
        switch_method: ApplicationSwitchMethod::AltTab,
        dwell_time_ms: Some(15000), // 15 seconds in previous app
        switch_count: Some(1),
        metadata: EventMetadata::with_timestamp(),
    };

    let workflow_event = WorkflowEvent::ApplicationSwitch(app_switch_event);

    // Measure conversion time
    let start = Instant::now();
    let result = converter.convert_event(&workflow_event, None).await?;
    let conversion_duration = start.elapsed();

    println!("✅ Conversion completed in {:?}", conversion_duration);
    println!();

    // Display the generated sequence
    println!("📋 Generated MCP sequence:");
    for (i, step) in result.primary_sequence.iter().enumerate() {
        println!("  Step {}: {}", i + 1, step.tool_name);
        println!("    Description: {}", step.description);
        println!(
            "    Arguments: {}",
            serde_json::to_string_pretty(&step.arguments)?
        );
        if let Some(timeout) = step.timeout_ms {
            println!("    Timeout: {}ms", timeout);
        }
        if let Some(delay) = step.delay_ms {
            println!("    Delay: {}ms", delay);
        }
        println!();
    }

    // Display conversion notes
    println!("📝 Conversion notes:");
    for note in result.conversion_notes {
        println!("  • {}", note);
    }
    println!();

    // Test fallback selector generation for various apps
    println!("🔍 Testing fallback selector generation:");
    let test_apps = vec![
        "Google Chrome",
        "Firefox",
        "Microsoft Edge",
        "Notepad",
        "Calculator",
        "Cursor",
        "Visual Studio Code",
        "File Explorer",
        "Command Prompt",
        "PowerShell",
        "Unknown App",
        "My Custom Application",
    ];

    for app in test_apps {
        let fallback = converter.generate_stable_fallback_selector(app);
        match fallback {
            Some(selector) => println!("  {} → {}", app, selector),
            None => println!("  {} → (no fallback)", app),
        }
    }

    println!();
    println!("🎯 Key optimizations applied:");
    println!("  • include_tree: false (skips expensive UI tree building)");
    println!("  • timeout_ms: 800 (vs default 3000ms)");
    println!("  • retries: 0 (no retry loops)");
    println!("  • delay_ms: 150 (vs previous 1000ms)");
    println!("  • Stable fallback selectors for common applications");

    Ok(())
}
