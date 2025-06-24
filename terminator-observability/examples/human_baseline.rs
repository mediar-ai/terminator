//! Example showing how to compare automation performance with human baselines

use std::time::Duration;
use std::sync::Arc;
use terminator_observability::prelude::*;
use terminator_observability::session::BaselineAction;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Create or load a human baseline
    let baseline = create_form_filling_baseline();
    
    // Save baseline for future use
    baseline.save("form_filling.baseline")?;
    println!("✓ Human baseline saved to form_filling.baseline");
    
    // Initialize observability
    let observability = TerminatorObservability::builder()
        .with_service_name("form-automation")
        .with_sampling_ratio(1.0)
        .build()?;

    // Create observable desktop
    let mut desktop = observability.create_desktop()?;
    
    // Start session with baseline comparison
    let session = desktop.start_session("form_filling");
    
    // Note: In a real implementation, we'd modify start_session to accept baseline
    // For now, we'll work with the session as-is
    session.add_metadata("baseline_duration_ms", baseline.average_duration.as_millis());
    
    // Simulate form filling automation
    println!("\n🤖 Starting automated form filling...");
    
    // Simulate opening application
    let app = desktop.open_application("Calculator").await?;
    println!("✓ Application opened");
    
    // Simulate form field interactions
    // In real scenario, these would be actual form fields
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let field1 = desktop
        .locator("name:Memory")
        .first(Some(Duration::from_secs(3)))
        .await?;
    field1.click().await?;
    println!("✓ Clicked first field");
    
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    let field2 = desktop
        .locator("name:History")
        .first(Some(Duration::from_secs(3)))
        .await?;
    field2.click().await?;
    println!("✓ Clicked second field");
    
    // Complete the session
    let report = Arc::try_unwrap(session)
        .ok()
        .expect("Session still has multiple references")
        .complete();
    
    // Display comparison results
    println!("\n📊 === Performance Comparison ===");
    println!("Task: {}", report.task_name);
    println!("Agent time: {:?}", report.duration);
    println!("Human average: {:?}", baseline.average_duration);
    println!("Efficiency ratio: {:.2}x", report.efficiency_ratio);
    
    if report.is_faster_than_human() {
        println!(
            "✅ Agent is {:.0}% FASTER than human!",
            report.improvement_percentage()
        );
    } else {
        println!(
            "❌ Agent is {:.0}% slower than human",
            -report.improvement_percentage()
        );
    }
    
    println!("\n📈 === Detailed Metrics ===");
    println!("Actions performed: {}", report.action_count);
    println!("Errors encountered: {}", report.error_count);
    println!("Success rate: {:.1}%", report.success_rate * 100.0);
    println!("Accuracy score: {:.1}%", report.accuracy_score * 100.0);
    
    // Compare individual action timings
    println!("\n⏱️  === Action Timing Comparison ===");
    println!("{:<30} {:>15} {:>15}", "Action", "Human (ms)", "Agent (ms)");
    println!("{:-<62}", "");
    
    // Show baseline actions
    for action in &baseline.actions {
        println!(
            "{:<30} {:>15}",
            action.name,
            action.duration.as_millis()
        );
    }
    
    // Export trace for further analysis
    let trace_json = report.trace.to_json()?;
    std::fs::write("form_filling_trace.json", trace_json)?;
    println!("\n✓ Trace exported to form_filling_trace.json");
    
    // Shutdown
    observability.shutdown().await?;
    
    Ok(())
}

/// Create a sample human baseline for form filling
fn create_form_filling_baseline() -> HumanBaseline {
    let mut baseline = HumanBaseline::new(
        "form_filling".to_string(),
        Duration::from_millis(15000), // 15 seconds average for human
    );
    
    // Add individual action timings based on user studies
    baseline.actions.push(BaselineAction {
        name: "locate_name_field".to_string(),
        duration: Duration::from_millis(3000),
        action_type: "visual_search".to_string(),
    });
    
    baseline.actions.push(BaselineAction {
        name: "click_name_field".to_string(),
        duration: Duration::from_millis(500),
        action_type: "mouse_click".to_string(),
    });
    
    baseline.actions.push(BaselineAction {
        name: "type_name".to_string(),
        duration: Duration::from_millis(4000),
        action_type: "typing".to_string(),
    });
    
    baseline.actions.push(BaselineAction {
        name: "locate_email_field".to_string(),
        duration: Duration::from_millis(2000),
        action_type: "visual_search".to_string(),
    });
    
    baseline.actions.push(BaselineAction {
        name: "click_email_field".to_string(),
        duration: Duration::from_millis(500),
        action_type: "mouse_click".to_string(),
    });
    
    baseline.actions.push(BaselineAction {
        name: "type_email".to_string(),
        duration: Duration::from_millis(5000),
        action_type: "typing".to_string(),
    });
    
    baseline.sample_count = 10; // Based on 10 human samples
    
    baseline
}