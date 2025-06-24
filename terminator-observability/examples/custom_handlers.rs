//! Example demonstrating custom event handlers and telemetry processors

use std::time::Duration;
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use terminator_observability::prelude::*;
use anyhow::Result;

/// Custom metrics tracker that counts specific operations
struct OperationTracker {
    click_count: AtomicU64,
    type_count: AtomicU64,
    error_count: AtomicU64,
    total_typing_chars: AtomicU64,
}

impl OperationTracker {
    fn new() -> Self {
        Self {
            click_count: AtomicU64::new(0),
            type_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            total_typing_chars: AtomicU64::new(0),
        }
    }
    
    fn increment_clicks(&self) {
        self.click_count.fetch_add(1, Ordering::Relaxed);
    }
    
    fn increment_typing(&self, chars: u64) {
        self.type_count.fetch_add(1, Ordering::Relaxed);
        self.total_typing_chars.fetch_add(chars, Ordering::Relaxed);
    }
    
    fn increment_errors(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }
    
    fn report(&self) {
        println!("\n📊 Custom Metrics Report:");
        println!("  Total clicks: {}", self.click_count.load(Ordering::Relaxed));
        println!("  Total typing operations: {}", self.type_count.load(Ordering::Relaxed));
        println!("  Total characters typed: {}", self.total_typing_chars.load(Ordering::Relaxed));
        println!("  Total errors: {}", self.error_count.load(Ordering::Relaxed));
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("🔧 Terminator Observability - Custom Handlers Example\n");
    
    // Create custom tracker
    let tracker = Arc::new(OperationTracker::new());
    let tracker_clone = tracker.clone();
    
    // Initialize observability with custom configuration
    let observability = TerminatorObservability::builder()
        .with_service_name("custom-handlers-demo")
        .with_sampling_ratio(1.0)
        .with_stdout_exporter(true)
        .build()?;
    
    // Create observable desktop
    let mut desktop = observability.create_desktop()?;
    
    // Start monitoring session
    let session = desktop.start_session("demonstrate_custom_handlers");
    session.add_metadata("demo_type", "custom_metrics");
    
    // Spawn a task to monitor metrics in real-time
    let metrics_collector = observability.metrics().clone();
    let monitor_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(2));
        
        for i in 1..=3 {
            interval.tick().await;
            
            let snapshot = metrics_collector.snapshot();
            println!("\n📈 Metrics Snapshot #{}", i);
            
            // Process counters
            for (key, value) in snapshot.counters.iter() {
                if key.name().contains("click") {
                    tracker_clone.increment_clicks();
                }
                println!("  Counter {}: {}", key, value);
            }
            
            // Process histograms
            for (key, histogram) in snapshot.histograms.iter() {
                if histogram.count > 0 {
                    println!(
                        "  Histogram {}: count={}, mean={:.2}ms, p95={:.2}ms",
                        key,
                        histogram.count,
                        histogram.mean(),
                        histogram.percentile(95.0)
                    );
                }
            }
        }
    });
    
    // Perform some operations to generate telemetry
    println!("🚀 Starting automation operations...");
    
    // Operation 1: Open application
    match desktop.open_application("Calculator").await {
        Ok(_) => println!("✓ Calculator opened"),
        Err(e) => {
            println!("✗ Failed to open Calculator: {}", e);
            tracker.increment_errors();
        }
    }
    
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Operation 2: Click operations
    for i in 1..=3 {
        println!("\n🖱️  Click operation #{}", i);
        
        let button = desktop
            .locator(format!("name:{}", i))
            .first(Some(Duration::from_secs(2)))
            .await;
            
        match button {
            Ok(btn) => {
                match btn.click().await {
                    Ok(_) => {
                        println!("  ✓ Clicked button {}", i);
                        tracker.increment_clicks();
                    }
                    Err(e) => {
                        println!("  ✗ Click failed: {}", e);
                        tracker.increment_errors();
                    }
                }
            }
            Err(e) => {
                println!("  ✗ Button not found: {}", e);
                tracker.increment_errors();
            }
        }
        
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    
    // Operation 3: Type some text
    println!("\n⌨️  Typing operation");
    let text = "Hello, Observability!";
    
    // Simulate typing into a field
    let field = desktop
        .locator("role:Edit")
        .first(Some(Duration::from_secs(2)))
        .await;
        
    match field {
        Ok(f) => {
            match f.type_text(text).await {
                Ok(_) => {
                    println!("  ✓ Typed: '{}'", text);
                    tracker.increment_typing(text.len() as u64);
                }
                Err(e) => {
                    println!("  ✗ Typing failed: {}", e);
                    tracker.increment_errors();
                }
            }
        }
        Err(_) => {
            println!("  ℹ️  No text field found, simulating typing metrics");
            // Simulate typing metrics even if field not found
            session.add_metadata("simulated_typing", text);
            tracker.increment_typing(text.len() as u64);
        }
    }
    
    // Wait for monitoring task to complete
    tokio::time::sleep(Duration::from_secs(7)).await;
    monitor_task.abort();
    
    // Complete session and generate report
    let report = Arc::try_unwrap(session)
        .ok()
        .expect("Session still has multiple references")
        .complete();
    
    // Display session report
    println!("\n📋 === Session Report ===");
    println!("Duration: {:?}", report.duration);
    println!("Actions: {}", report.action_count);
    println!("Errors: {}", report.error_count);
    println!("Success Rate: {:.1}%", report.success_rate * 100.0);
    
    // Display custom metrics
    tracker.report();
    
    // Advanced: Analyze trace spans
    println!("\n🔍 === Span Analysis ===");
    analyze_spans(&report.trace);
    
    // Export data for external analysis
    export_observability_data(&report)?;
    
    // Shutdown
    observability.shutdown().await?;
    
    println!("\n✅ Custom handlers demo completed!");
    
    Ok(())
}

/// Analyze spans to extract insights
fn analyze_spans(trace: &terminator_observability::trace::Trace) {
    use terminator_observability::trace::SpanStatus;
    
    let total_spans = trace.spans.len();
    let error_spans = trace.spans.iter()
        .filter(|s| matches!(s.status, SpanStatus::Error { .. }))
        .count();
    
    let operation_times: std::collections::HashMap<String, Vec<Duration>> = 
        trace.spans.iter()
            .fold(std::collections::HashMap::new(), |mut acc, span| {
                acc.entry(span.operation.clone())
                    .or_insert_with(Vec::new)
                    .push(span.duration);
                acc
            });
    
    println!("Total spans: {}", total_spans);
    println!("Error spans: {}", error_spans);
    println!("\nOperation timing analysis:");
    
    for (operation, durations) in operation_times {
        let total: Duration = durations.iter().sum();
        let avg_ms = total.as_millis() / durations.len() as u128;
        
        println!("  {}: {} calls, avg {}ms", 
            operation, 
            durations.len(), 
            avg_ms
        );
    }
}

/// Export observability data for external tools
fn export_observability_data(report: &SessionReport) -> Result<()> {
    // Export trace as JSON
    let trace_json = report.trace.to_json()?;
    std::fs::write("custom_handlers_trace.json", trace_json)?;
    
    // Export metrics summary
    let metrics_summary = serde_json::json!({
        "session_id": report.session_id,
        "task_name": report.task_name,
        "duration_ms": report.duration.as_millis(),
        "action_count": report.action_count,
        "error_count": report.error_count,
        "success_rate": report.success_rate,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    
    std::fs::write(
        "custom_handlers_metrics.json", 
        serde_json::to_string_pretty(&metrics_summary)?
    )?;
    
    println!("\n💾 Exported data:");
    println!("  - Trace: custom_handlers_trace.json");
    println!("  - Metrics: custom_handlers_metrics.json");
    
    Ok(())
}