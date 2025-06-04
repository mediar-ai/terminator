use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use terminator::platforms::windows::WindowsEngine;
use terminator_workflow_recorder::{WorkflowRecorder, WorkflowRecorderConfig};
use tokio::signal::ctrl_c;
use tokio_stream::StreamExt;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use terminator::platforms::AccessibilityEngine;
use serde::{Deserialize, Serialize};

/// Performance metrics for tree fetching operations
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TreeFetchMetrics {
    method: String,
    duration: Duration,
    element_count: usize,
    max_depth: usize,
    success: bool,
    error_msg: Option<String>,
    timestamp: std::time::SystemTime,
    app_name: String,
}

/// Aggregated statistics for tree fetching performance
#[derive(Debug, Default)]
struct PerformanceStats {
    total_fetches: usize,
    successful_fetches: usize,
    total_duration: Duration,
    min_duration: Option<Duration>,
    max_duration: Option<Duration>,
    total_elements: usize,
    method_counts: HashMap<String, usize>,
    method_stats: HashMap<String, MethodStats>,
    all_metrics: Vec<TreeFetchMetrics>,
}

#[derive(Debug, Default, Clone)]
struct MethodStats {
    count: usize,
    total_elements: usize,
    total_duration: Duration,
    min_duration: Duration,
    max_duration: Duration,
}

impl PerformanceStats {
    fn add_metric(&mut self, metric: &TreeFetchMetrics) {
        self.all_metrics.push(metric.clone());
        self.total_fetches += 1;
        
        if metric.success {
            self.successful_fetches += 1;
            self.total_duration += metric.duration;
            self.total_elements += metric.element_count;
            
            // Update min/max durations
            match self.min_duration {
                None => self.min_duration = Some(metric.duration),
                Some(min) if metric.duration < min => self.min_duration = Some(metric.duration),
                _ => {}
            }
            
            match self.max_duration {
                None => self.max_duration = Some(metric.duration),
                Some(max) if metric.duration > max => self.max_duration = Some(metric.duration),
                _ => {}
            }
            
            // Update method-specific stats
            *self.method_counts.entry(metric.method.clone()).or_insert(0) += 1;
            
            let method_stat = self.method_stats.entry(metric.method.clone()).or_insert(MethodStats {
                min_duration: metric.duration,
                max_duration: metric.duration,
                ..Default::default()
            });
            
            method_stat.count += 1;
            method_stat.total_elements += metric.element_count;
            method_stat.total_duration += metric.duration;
            
            if metric.duration < method_stat.min_duration {
                method_stat.min_duration = metric.duration;
            }
            if metric.duration > method_stat.max_duration {
                method_stat.max_duration = metric.duration;
            }
        }
    }
    
    fn average_duration(&self) -> Duration {
        if self.successful_fetches > 0 {
            self.total_duration / self.successful_fetches as u32
        } else {
            Duration::ZERO
        }
    }
    
    fn elements_per_second(&self) -> f64 {
        if self.total_duration.as_secs_f64() > 0.0 {
            self.total_elements as f64 / self.total_duration.as_secs_f64()
        } else {
            0.0
        }
    }
    
    fn print_detailed_summary(&self) {
        println!("\n");
        println!("🎯 ========================= PERFORMANCE SUMMARY =========================");
        println!("📊 Total Fetches: {} (Success: {}, Failure: {})", 
                 self.total_fetches, self.successful_fetches, self.total_fetches - self.successful_fetches);
        
        if self.successful_fetches > 0 {
            println!("⏱️  Overall Timing: Avg {:?}, Min {:?}, Max {:?}", 
                     self.average_duration(),
                     self.min_duration.unwrap_or(Duration::ZERO),
                     self.max_duration.unwrap_or(Duration::ZERO));
            println!("⚡ Overall Speed: {:.1} elements/second", self.elements_per_second());
            println!("🔢 Total Elements: {}", self.total_elements);
            
            println!("\n📈 METHOD COMPARISON:");
            println!("┌─────────────────┬───────┬──────────────┬──────────────┬──────────────┬──────────────┐");
            println!("│ Method          │ Count │ Avg Duration │ Avg Elem/Sec │ Min Duration │ Max Duration │");
            println!("├─────────────────┼───────┼──────────────┼──────────────┼──────────────┼──────────────┤");
            
            let mut sorted_methods: Vec<_> = self.method_stats.iter().collect();
            sorted_methods.sort_by(|a, b| {
                let a_speed = a.1.total_elements as f64 / a.1.total_duration.as_secs_f64();
                let b_speed = b.1.total_elements as f64 / b.1.total_duration.as_secs_f64();
                b_speed.partial_cmp(&a_speed).unwrap_or(std::cmp::Ordering::Equal)
            });
            
            for (method, stats) in sorted_methods {
                let avg_duration = stats.total_duration / stats.count as u32;
                let speed = stats.total_elements as f64 / stats.total_duration.as_secs_f64();
                println!("│ {:15} │ {:5} │ {:>10.2?} │ {:>10.1} │ {:>10.2?} │ {:>10.2?} │",
                         method, stats.count, avg_duration, speed, 
                         stats.min_duration, stats.max_duration);
            }
            println!("└─────────────────┴───────┴──────────────┴──────────────┴──────────────┴──────────────┘");
            
            // Show recent performance trend
            if self.all_metrics.len() >= 5 {
                println!("\n📈 RECENT PERFORMANCE (Last 5 fetches):");
                let recent: Vec<_> = self.all_metrics.iter().rev().take(5).collect();
                for (i, metric) in recent.iter().enumerate() {
                    let speed = if metric.success {
                        metric.element_count as f64 / metric.duration.as_secs_f64()
                    } else { 0.0 };
                    
                    let status = if metric.success { "✅" } else { "❌" };
                    println!("  {} {}: {} - {} elements in {:?} ({:.1} elem/s) [{}]",
                             i + 1, status, metric.method.to_uppercase(), 
                             metric.element_count, metric.duration, speed, metric.app_name);
                }
            }
        }
        println!("═══════════════════════════════════════════════════════════════════════");
    }
}

/// Fetches UI tree using different methods and measures performance
async fn fetch_tree_with_performance(
    engine: &WindowsEngine,
    app_name: &str,
    method: &str,
) -> TreeFetchMetrics {
    let start_time = Instant::now();
    let timestamp = std::time::SystemTime::now();
    
    println!("🔄 Fetching tree for '{}' using {} method...", app_name, method.to_uppercase());
    
    let result = match method {
        "cached" => {
            match engine.get_window_tree_cached(app_name) {
                Ok(tree) => Ok((count_tree_elements(&tree), calculate_tree_depth(&tree))),
                Err(e) => Err(e),
            }
        }
        "fast_depth_4" => {
            match engine.get_window_tree_fast(app_name, 4) {
                Ok(tree) => Ok((count_tree_elements(&tree), calculate_tree_depth(&tree))),
                Err(e) => Err(e),
            }
        }
        "fast_depth_6" => {
            match engine.get_window_tree_fast(app_name, 6) {
                Ok(tree) => Ok((count_tree_elements(&tree), calculate_tree_depth(&tree))),
                Err(e) => Err(e),
            }
        }
        "fast_depth_8" => {
            match engine.get_window_tree_fast(app_name, 8) {
                Ok(tree) => Ok((count_tree_elements(&tree), calculate_tree_depth(&tree))),
                Err(e) => Err(e),
            }
        }
        "baseline" => {
            match engine.get_window_tree_by_title(app_name) {
                Ok(tree) => Ok((count_tree_elements(&tree), calculate_tree_depth(&tree))),
                Err(e) => Err(e),
            }
        }
        _ => {
            let error_msg = format!("Unknown method: {}", method);
            return TreeFetchMetrics {
                method: method.to_string(),
                duration: start_time.elapsed(),
                element_count: 0,
                max_depth: 0,
                success: false,
                error_msg: Some(error_msg),
                timestamp,
                app_name: app_name.to_string(),
            };
        }
    };
    
    let duration = start_time.elapsed();
    
    match result {
        Ok((element_count, max_depth)) => {
            let speed = element_count as f64 / duration.as_secs_f64();
            
            println!("   ✅ SUCCESS: {} elements, depth {}, {:?} (🚀 {:.1} elem/s)", 
                     element_count, max_depth, duration, speed);
            
            TreeFetchMetrics {
                method: method.to_string(),
                duration,
                element_count,
                max_depth,
                success: true,
                error_msg: None,
                timestamp,
                app_name: app_name.to_string(),
            }
        }
        Err(e) => {
            let error_msg = e.to_string();
            println!("   ❌ FAILED: {:?} - {}", duration, error_msg);
            
            TreeFetchMetrics {
                method: method.to_string(),
                duration,
                element_count: 0,
                max_depth: 0,
                success: false,
                error_msg: Some(error_msg),
                timestamp,
                app_name: app_name.to_string(),
            }
        }
    }
}

/// Helper to count elements in a UI tree
fn count_tree_elements(node: &terminator::UINode) -> usize {
    1 + node.children.iter().map(count_tree_elements).sum::<usize>()
}

/// Helper to calculate tree depth
fn calculate_tree_depth(node: &terminator::UINode) -> usize {
    if node.children.is_empty() {
        1
    } else {
        1 + node.children.iter().map(calculate_tree_depth).max().unwrap_or(0)
    }
}

/// Gets the application name from the current focused window
async fn get_current_app_name(engine: &WindowsEngine) -> String {
    match engine.get_current_application().await {
        Ok(app) => {
            match app.attributes().name {
                Some(name) => {
                    // Clean up the name for better tree fetching
                    if name.contains(" - ") {
                        name.split(" - ").last().unwrap_or(&name).to_string()
                    } else {
                        name
                    }
                }
                None => "Unknown App".to_string(),
            }
        }
        Err(_) => "Unknown App".to_string(),
    }
}

/// Helper to check if mouse event is a click
fn is_mouse_click(mouse_event: &terminator_workflow_recorder::MouseEvent) -> bool {
    match mouse_event.event_type {
        terminator_workflow_recorder::MouseEventType::Click |
        terminator_workflow_recorder::MouseEventType::RightClick => true,
        _ => false,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 ================= CACHE-OPTIMIZED TREE PERFORMANCE TESTER =================");
    
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    
    info!("🔧 Initializing Windows engine with cache optimization...");
    let engine = Arc::new(WindowsEngine::new(false, false)?);
    
    // Enable background cache warmer for even better performance
    info!("🔥 Enabling background cache warmer...");
    engine.enable_background_cache_warmer(true, Some(10), Some(5))?;
    
    info!("📝 Setting up lightweight workflow recorder...");
    let config = WorkflowRecorderConfig {
        // Focus on key events that matter for UI tree analysis
        record_mouse: true,
        record_keyboard: true,
        record_window: true,
        capture_ui_elements: true, // Disable to reduce overhead since we're fetching trees manually
        
        // Minimize other recording overhead
        record_clipboard: true,
        record_text_selection: true,
        record_drag_drop: true,
        record_hotkeys: true,
        record_ui_focus_changes: true, // Keep this to detect app changes
        record_ui_structure_changes: true,
        record_ui_property_changes: true,
        
        // Performance tuning
        mouse_move_throttle_ms: 200, // Reduce mouse event spam
        
        ..Default::default()
    };
    
    let mut recorder = WorkflowRecorder::new("Cache Performance Test".to_string(), config);
    let mut event_stream = recorder.event_stream();
    recorder.start().await?;
    
    // Create shutdown signal
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::broadcast::channel(1);
    let shutdown_tx_clone = shutdown_tx.clone();
    
    // Handle Ctrl+C gracefully
    tokio::spawn(async move {
        ctrl_c().await.expect("Failed to listen for ctrl+c");
        println!("\n🛑 Ctrl+C detected - shutting down gracefully...");
        let _ = shutdown_tx_clone.send(());
    });
    
    println!("🎯 Cache Performance Tester Started!");
    println!("================================================================================");
    println!("🚀 This demo fetches UI trees using optimized Windows cache requests");
    println!("📊 Performance metrics are collected for each tree fetch operation");
    println!("⚡ The cache warmer runs in background to pre-populate caches");
    println!("");
    println!("🎮 Interact with different applications to trigger tree fetching:");
    println!("   • Mouse clicks will fetch tree with CACHED method");
    println!("   • Key presses will cycle through FAST methods (depth 4, 6, 8)");
    println!("   • Focus changes will use BASELINE method for comparison");
    println!("");
    println!("📈 Watch the real-time performance improvements!");
    println!("🛑 Press Ctrl+C to stop and see detailed performance summary");
    println!("================================================================================");
    
    let mut performance_stats = PerformanceStats::default();
    let mut last_app_name = String::new();
    
    // Cycle through different methods to compare performance
    let methods = ["fast_depth_4", "fast_depth_6", "fast_depth_8", "cached"];
    let mut method_index = 0;
    
    // Clone Arc for the async task
    let engine_clone = Arc::clone(&engine);
    let mut shutdown_rx_clone = shutdown_rx.resubscribe();
    
    let event_processing_task = tokio::spawn(async move {
        let mut local_stats = PerformanceStats::default();
        
        loop {
            tokio::select! {
                _ = shutdown_rx_clone.recv() => {
                    println!("🛑 Shutdown signal received in event processor");
                    break;
                }
                event_opt = event_stream.next() => {
                    match event_opt {
                        Some(event) => {
                            // Get current application name
                            let app_name = get_current_app_name(&engine_clone).await;
                            let mut should_fetch = false;
                            let mut selected_method = "cached";
                            
                            match &event {
                                terminator_workflow_recorder::WorkflowEvent::Mouse(mouse_event) => {
                                    if is_mouse_click(mouse_event) {
                                        should_fetch = true;
                                        selected_method = "cached";
                                        println!("\n🖱️  Mouse click detected - fetching tree with CACHED method");
                                    }
                                }
                                terminator_workflow_recorder::WorkflowEvent::Keyboard(kb_event) => {
                                    if kb_event.is_key_down && kb_event.character.is_some() {
                                        should_fetch = true;
                                        selected_method = "cached";
                                        println!("\n⌨️  Key press detected - fetching tree with {} method", selected_method.to_uppercase());
                                    }
                                }
                                terminator_workflow_recorder::WorkflowEvent::UiFocusChanged(_) => {
                                    // Only fetch if app actually changed
                                    if app_name != last_app_name && !app_name.contains("Unknown") {
                                        should_fetch = true;
                                        selected_method = "baseline";
                                        println!("\n🎯 App focus changed to '{}' - fetching tree with BASELINE method", app_name);
                                        last_app_name = app_name.clone();
                                    }
                                }
                                terminator_workflow_recorder::WorkflowEvent::Clipboard(_) => {
                                    should_fetch = true;
                                    selected_method = "cached";
                                    println!("\n📋 Clipboard content changed - fetching tree with CACHED method");
                                }
                                _ => {}
                            }
                            
                            if should_fetch && !app_name.contains("Unknown") {
                                // Fetch tree and measure performance
                                let metric = fetch_tree_with_performance(&engine_clone, &app_name, selected_method).await;
                                
                                local_stats.add_metric(&metric);
                                
                                // Print quick progress every 3 successful fetches
                                if local_stats.successful_fetches % 3 == 0 && local_stats.successful_fetches > 0 {
                                    println!("\n📊 Progress: {} successful fetches, overall avg {:.1} elem/s",
                                             local_stats.successful_fetches,
                                             local_stats.elements_per_second());
                                }
                            }
                        }
                        None => {
                            println!("Event stream ended");
                            break;
                        }
                    }
                }
            }
        }
        
        local_stats
    });
    
    // Wait for shutdown signal
    let _ = shutdown_rx.recv().await;
    
    println!("🛑 Stopping recorder and collecting final metrics...");
    recorder.stop().await?;
    
    // Collect final stats from the event processing task
    match tokio::time::timeout(Duration::from_secs(5), event_processing_task).await {
        Ok(Ok(final_stats)) => {
            performance_stats = final_stats;
        }
        Ok(Err(e)) => {
            println!("⚠️  Error collecting final stats: {}", e);
        }
        Err(_) => {
            println!("⚠️  Timeout collecting final stats");
        }
    }
    
    // Print detailed performance summary
    performance_stats.print_detailed_summary();
    
    // Save detailed report to JSON
    let report_file = "cache_performance_report.json";
    match serde_json::to_string_pretty(&performance_stats.all_metrics) {
        Ok(json_data) => {
            if let Err(e) = std::fs::write(report_file, json_data) {
                println!("⚠️  Failed to save report to {}: {}", report_file, e);
            } else {
                println!("💾 Detailed performance report saved to: {}", report_file);
            }
        }
        Err(e) => {
            println!("⚠️  Failed to serialize performance data: {}", e);
        }
    }
    
    println!("✅ Cache Performance Test Completed Successfully!");
    
    Ok(())
} 