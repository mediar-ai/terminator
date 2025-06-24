//! Basic example of using Terminator Observability

use std::time::Duration;
use std::sync::Arc;
use terminator_observability::prelude::*;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize observability with default configuration
    let observability = TerminatorObservability::builder()
        .with_service_name("calculator-automation")
        .with_sampling_ratio(1.0) // Sample all traces
        .with_stdout_exporter(true) // Enable console output for demo
        .build()?;

    // Create an observable desktop instance
    let mut desktop = observability.create_desktop()?;
    
    // Start a new session
    let session = desktop.start_session("calculate_sum");
    session.add_metadata("calculation", "7 + 3");
    session.add_metadata("expected_result", 10);
    
    println!("Starting calculator automation...");
    
    // Open calculator application
    let calculator = desktop.open_application("Calculator").await?;
    println!("✓ Calculator opened");
    
    // Wait a bit for the application to fully load
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Perform calculation: 7 + 3 = 10
    let button_7 = desktop
        .locator("name:Seven")
        .first(Some(Duration::from_secs(5)))
        .await?;
    button_7.click().await?;
    println!("✓ Clicked 7");
    
    let button_plus = desktop
        .locator("name:Plus")
        .first(Some(Duration::from_secs(5)))
        .await?;
    button_plus.click().await?;
    println!("✓ Clicked +");
    
    let button_3 = desktop
        .locator("name:Three")
        .first(Some(Duration::from_secs(5)))
        .await?;
    button_3.click().await?;
    println!("✓ Clicked 3");
    
    let button_equals = desktop
        .locator("name:Equals")
        .first(Some(Duration::from_secs(5)))
        .await?;
    button_equals.click().await?;
    println!("✓ Clicked =");
    
    // Complete the session and get the report
    let report = Arc::try_unwrap(session)
        .ok()
        .expect("Session still has multiple references")
        .complete();
    
    // Print results
    println!("\n=== Session Report ===");
    println!("Task: {}", report.task_name);
    println!("Duration: {:?}", report.duration);
    println!("Actions performed: {}", report.action_count);
    println!("Errors: {}", report.error_count);
    println!("Success rate: {:.1}%", report.success_rate * 100.0);
    
    // Print trace summary
    println!("\n=== Trace Summary ===");
    for span in &report.trace.spans {
        println!(
            "  {} - {:?} ({})",
            span.operation,
            span.duration,
            match &span.status {
                terminator_observability::trace::SpanStatus::Ok => "✓",
                terminator_observability::trace::SpanStatus::Error { .. } => "✗",
                terminator_observability::trace::SpanStatus::Unset => "?",
            }
        );
    }
    
    // Get metrics snapshot
    let metrics = observability.metrics().snapshot();
    
    println!("\n=== Metrics ===");
    // Print some key metrics
    for (key, value) in metrics.counters.iter() {
        println!("  Counter {}: {}", key, value);
    }
    
    for (key, histogram) in metrics.histograms.iter() {
        println!(
            "  Histogram {}: mean={:.2}ms, min={:.2}ms, max={:.2}ms",
            key,
            histogram.mean(),
            histogram.min,
            histogram.max
        );
    }
    
    // Shutdown observability to flush all data
    observability.shutdown().await?;
    
    println!("\n✓ Observability demo completed successfully!");
    
    Ok(())
}