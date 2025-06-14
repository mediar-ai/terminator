use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sysinfo::System;
use terminator_workflow_recorder::{WorkflowRecorder, WorkflowRecorderConfig};
use tokio::time::interval;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct PerformanceSnapshot {
    timestamp: u64,
    cpu_usage: f32,
    memory_usage_mb: u64,
    total_memory_mb: u64,
    memory_usage_percent: f32,
    recorder_process: Option<ProcessInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ProcessInfo {
    name: String,
    pid: u32,
    cpu_usage: f32,
    memory_mb: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct PerformanceReport {
    session_start: u64,
    session_end: u64,
    duration_seconds: f64,
    snapshots: Vec<PerformanceSnapshot>,
    summary: PerformanceSummary,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct PerformanceSummary {
    avg_cpu: f32,
    max_cpu: f32,
    min_cpu: f32,
    avg_memory_percent: f32,
    max_memory_percent: f32,
    peak_memory_mb: u64,
    total_samples: usize,
    recorder_stats: RecorderStats,
    high_cpu_events: Vec<HighCpuEvent>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct RecorderStats {
    avg_recorder_cpu: f32,
    max_recorder_cpu: f32,
    min_recorder_cpu: f32,
    avg_recorder_memory_mb: u64,
    max_recorder_memory_mb: u64,
    samples_with_recorder: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct HighCpuEvent {
    timestamp: u64,
    cpu_usage: f32,
    recorder_cpu: Option<f32>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔴 STARTING ENHANCED CPU/MEMORY MONITORING RECORDER");
    println!("==================================================");
    println!("💡 Press Ctrl+C anytime to stop and save results");

    // Simple config for recording
    let config = WorkflowRecorderConfig {
        record_mouse: true,
        record_keyboard: true,
        record_window: true,
        capture_ui_elements: true,
        record_clipboard: true,
        record_text_selection: true,
        ..Default::default()
    };

    // Create recorder
    let mut recorder = WorkflowRecorder::new("cpu_memory_test_workflow".to_string(), config);

    // Performance monitoring setup
    let running = Arc::new(AtomicBool::new(true));
    let performance_data = Arc::new(Mutex::new(Vec::<PerformanceSnapshot>::new()));
    let session_start = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

    // Setup Ctrl+C handler
    let running_ctrlc = Arc::clone(&running);
    ctrlc::set_handler(move || {
        println!("\n🛑 Ctrl+C detected! Stopping gracefully...");
        running_ctrlc.store(false, Ordering::Relaxed);
    })?;

    // Start performance monitoring task
    let running_monitor = Arc::clone(&running);
    let performance_data_monitor = Arc::clone(&performance_data);
    let monitor_task = tokio::spawn(async move {
        let mut interval = interval(Duration::from_millis(500));
        let mut system = System::new_all();

        while running_monitor.load(Ordering::Relaxed) {
            interval.tick().await;
            system.refresh_all();

            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            // CPU usage
            let avg_cpu = system.cpus().iter().map(|cpu| cpu.cpu_usage()).sum::<f32>()
                / system.cpus().len() as f32;

            // Memory usage
            let total_memory = system.total_memory();
            let used_memory = system.used_memory();
            let memory_percent = (used_memory as f32 / total_memory as f32) * 100.0;

            // Get top processes by CPU/memory
            let mut processes: Vec<_> = system.processes().iter().collect();
            processes.sort_by(|a, b| b.1.cpu_usage().partial_cmp(&a.1.cpu_usage()).unwrap());

            // Get current process name for filtering
            let current_process_name = std::env::current_exe()
                .ok()
                .and_then(|path| {
                    path.file_name()
                        .map(|name| name.to_string_lossy().to_string())
                })
                .unwrap_or_else(|| "simple_cpu_recorder.exe".to_string());

            // Find only our recorder process
            let recorder_process = system
                .processes()
                .iter()
                .find(|(_, process)| process.name() == current_process_name)
                .map(|(pid, process)| ProcessInfo {
                    name: process.name().to_string(),
                    pid: pid.as_u32(),
                    cpu_usage: process.cpu_usage(),
                    memory_mb: process.memory() / 1024 / 1024,
                });

            // Real-time display - show recorder-specific info
            let recorder_info = if let Some(ref proc) = recorder_process {
                format!(
                    " | 📊 Recorder: {:.1}% CPU, {} MB",
                    proc.cpu_usage, proc.memory_mb
                )
            } else {
                String::new()
            };

            let snapshot = PerformanceSnapshot {
                timestamp,
                cpu_usage: avg_cpu,
                memory_usage_mb: used_memory / 1024 / 1024,
                total_memory_mb: total_memory / 1024 / 1024,
                memory_usage_percent: memory_percent,
                recorder_process: recorder_process.clone(),
            };

            // Store snapshot
            if let Ok(mut data) = performance_data_monitor.lock() {
                data.push(snapshot);
            }

            print!(
                "\r💻 System CPU: {:.1}% | 🧠 RAM: {:.1}% ({} MB / {} MB){}",
                avg_cpu,
                memory_percent,
                used_memory / 1024 / 1024,
                total_memory / 1024 / 1024,
                recorder_info
            );
            std::io::Write::flush(&mut std::io::stdout()).unwrap();

            if avg_cpu > 80.0 {
                println!("\n⚠️  HIGH CPU: {:.1}%", avg_cpu);
            }
            if memory_percent > 85.0 {
                println!("\n⚠️  HIGH MEMORY: {:.1}%", memory_percent);
            }
        }
    });

    // Start recording
    println!("\n🟢 Starting workflow recording...");
    recorder.start().await?;

    // Print instructions
    print_instructions();

    // Wait for user actions or Ctrl+C
    println!("\n⌨️  Press ENTER to stop, or use Ctrl+C for graceful shutdown...");

    // Non-blocking input check
    let input_task = tokio::spawn(async {
        let mut input = String::new();
        tokio::task::spawn_blocking(move || std::io::stdin().read_line(&mut input))
            .await
            .unwrap()
            .unwrap();
    });

    // Wait for either input or Ctrl+C
    tokio::select! {
        _ = input_task => {},
        _ = tokio::signal::ctrl_c() => {
            println!("\n🛑 Ctrl+C received!");
        }
    }

    // Stop everything
    running.store(false, Ordering::Relaxed);
    println!("\n🛑 Stopping recorder and monitors...");

    recorder.stop().await?;
    monitor_task.await?;

    // Generate performance report
    let session_end = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let performance_report =
        generate_performance_report(session_start, session_end, &performance_data)?;

    // Save files
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let workflow_filename = format!("workflow_{}.json", timestamp);
    let performance_filename = format!("performance_{}.json", timestamp);

    recorder.save(&workflow_filename)?;

    let performance_json = serde_json::to_string_pretty(&performance_report)?;
    std::fs::write(&performance_filename, performance_json)?;

    // Print summary
    print_final_summary(
        &performance_report,
        &workflow_filename,
        &performance_filename,
    );

    Ok(())
}

fn generate_performance_report(
    session_start: u64,
    session_end: u64,
    performance_data: &Arc<Mutex<Vec<PerformanceSnapshot>>>,
) -> Result<PerformanceReport, Box<dyn std::error::Error>> {
    let snapshots = performance_data.lock().unwrap().clone();

    if snapshots.is_empty() {
        return Ok(PerformanceReport {
            session_start,
            session_end,
            duration_seconds: (session_end - session_start) as f64,
            snapshots: vec![],
            summary: PerformanceSummary {
                avg_cpu: 0.0,
                max_cpu: 0.0,
                min_cpu: 0.0,
                avg_memory_percent: 0.0,
                max_memory_percent: 0.0,
                peak_memory_mb: 0,
                total_samples: 0,
                recorder_stats: RecorderStats {
                    avg_recorder_cpu: 0.0,
                    max_recorder_cpu: 0.0,
                    min_recorder_cpu: 0.0,
                    avg_recorder_memory_mb: 0,
                    max_recorder_memory_mb: 0,
                    samples_with_recorder: 0,
                },
                high_cpu_events: vec![],
            },
        });
    }

    let cpu_values: Vec<f32> = snapshots.iter().map(|s| s.cpu_usage).collect();
    let memory_values: Vec<f32> = snapshots.iter().map(|s| s.memory_usage_percent).collect();

    let avg_cpu = cpu_values.iter().sum::<f32>() / cpu_values.len() as f32;
    let max_cpu = cpu_values.iter().fold(0.0f32, |a, &b| a.max(b));
    let min_cpu = cpu_values.iter().fold(100.0f32, |a, &b| a.min(b));

    let avg_memory_percent = memory_values.iter().sum::<f32>() / memory_values.len() as f32;
    let max_memory_percent = memory_values.iter().fold(0.0f32, |a, &b| a.max(b));
    let peak_memory_mb = snapshots
        .iter()
        .map(|s| s.memory_usage_mb)
        .max()
        .unwrap_or(0u64);

    // Calculate recorder-specific stats
    let recorder_data: Vec<_> = snapshots
        .iter()
        .filter_map(|s| s.recorder_process.as_ref())
        .collect();

    let recorder_stats = if !recorder_data.is_empty() {
        let recorder_cpu_values: Vec<f32> = recorder_data.iter().map(|p| p.cpu_usage).collect();
        let recorder_memory_values: Vec<u64> = recorder_data.iter().map(|p| p.memory_mb).collect();

        let avg_recorder_cpu =
            recorder_cpu_values.iter().sum::<f32>() / recorder_cpu_values.len() as f32;
        let max_recorder_cpu = recorder_cpu_values.iter().fold(0.0f32, |a, &b| a.max(b));
        let min_recorder_cpu = recorder_cpu_values.iter().fold(100.0f32, |a, &b| a.min(b));
        let avg_recorder_memory =
            recorder_memory_values.iter().sum::<u64>() / recorder_memory_values.len() as u64;
        let max_recorder_memory = recorder_memory_values.iter().max().cloned().unwrap_or(0);

        RecorderStats {
            avg_recorder_cpu,
            max_recorder_cpu,
            min_recorder_cpu,
            avg_recorder_memory_mb: avg_recorder_memory,
            max_recorder_memory_mb: max_recorder_memory,
            samples_with_recorder: recorder_data.len(),
        }
    } else {
        RecorderStats {
            avg_recorder_cpu: 0.0,
            max_recorder_cpu: 0.0,
            min_recorder_cpu: 0.0,
            avg_recorder_memory_mb: 0,
            max_recorder_memory_mb: 0,
            samples_with_recorder: 0,
        }
    };

    let mut high_cpu_events = Vec::new();

    for snapshot in &snapshots {
        if let Some(ref process) = snapshot.recorder_process {
            if process.cpu_usage > 80.0 {
                high_cpu_events.push(HighCpuEvent {
                    timestamp: snapshot.timestamp,
                    cpu_usage: process.cpu_usage,
                    recorder_cpu: Some(process.cpu_usage),
                });
            }
        }
    }

    let summary = PerformanceSummary {
        avg_cpu,
        max_cpu,
        min_cpu,
        avg_memory_percent,
        max_memory_percent,
        peak_memory_mb,
        total_samples: snapshots.len(),
        recorder_stats,
        high_cpu_events,
    };

    Ok(PerformanceReport {
        session_start,
        session_end,
        duration_seconds: (session_end - session_start) as f64,
        snapshots,
        summary,
    })
}

fn print_final_summary(report: &PerformanceReport, workflow_file: &str, performance_file: &str) {
    println!("\n📊 FINAL PERFORMANCE SUMMARY");
    println!("============================");
    println!("⏱️  Duration: {:.1} seconds", report.duration_seconds);
    println!(
        "📈 CPU - Avg: {:.1}%, Max: {:.1}%, Min: {:.1}%",
        report.summary.avg_cpu, report.summary.max_cpu, report.summary.min_cpu
    );
    println!(
        "🧠 Memory - Avg: {:.1}%, Max: {:.1}%, Peak: {} MB",
        report.summary.avg_memory_percent,
        report.summary.max_memory_percent,
        report.summary.peak_memory_mb
    );
    println!("📊 Total Samples: {}", report.summary.total_samples);
    println!(
        "⚠️  High CPU Events: {}",
        report.summary.high_cpu_events.len()
    );

    println!("\n🎯 Recorder Stats:");
    println!(
        "   • Avg CPU: {:.1}%",
        report.summary.recorder_stats.avg_recorder_cpu
    );
    println!(
        "   • Max CPU: {:.1}%",
        report.summary.recorder_stats.max_recorder_cpu
    );
    println!(
        "   • Min CPU: {:.1}%",
        report.summary.recorder_stats.min_recorder_cpu
    );
    println!(
        "   • Avg Memory: {:.1} MB",
        report.summary.recorder_stats.avg_recorder_memory_mb
    );
    println!(
        "   • Max Memory: {} MB",
        report.summary.recorder_stats.max_recorder_memory_mb
    );
    println!(
        "   • Samples with Recorder: {}",
        report.summary.recorder_stats.samples_with_recorder
    );

    if !report.summary.high_cpu_events.is_empty() {
        println!("\n🔥 High CPU Events:");
        for event in &report.summary.high_cpu_events {
            let time = chrono::DateTime::from_timestamp(event.timestamp as i64, 0)
                .unwrap()
                .format("%H:%M:%S");
            println!(
                "   • {}: {:.1}% (Recorder: {})",
                time,
                event.cpu_usage,
                match event.recorder_cpu {
                    Some(cpu) => format!("{:.1}%", cpu),
                    None => "Unknown".to_string(),
                }
            );
        }
    }

    println!("\n💾 Files Saved:");
    println!("   📝 Workflow: {}", workflow_file);
    println!("   📊 Performance: {}", performance_file);
    println!("\n✅ Analysis complete! Check the JSON files for detailed data.");
}

fn print_instructions() {
    println!("\n📋 PERFORM THESE ACTIONS (monitoring CPU/memory):");
    println!("=================================================");
    println!("1. 🌐 Open Google Chrome");
    println!("2. 🔗 Navigate to: https://pages.dataiku.com/guide-to-ai-agents");
    println!("3. ⏳ Wait for page to load completely");
    println!("4. 📝 Select some text from the article (drag to highlight)");
    println!("5. 📋 Copy the text (Ctrl+C)");
    println!("6. 📄 Open Notepad");
    println!("7. 📝 Paste the text (Ctrl+V)");
    println!("8. 💾 Save the file (Ctrl+S)");
    println!("9. ✨ Try scrolling, clicking, switching apps");
    println!("10. 🔄 Alt+Tab between applications");
    println!();
    println!("💡 Watch for CPU/memory spikes - they'll be logged!");
    println!("🛑 Use Ctrl+C anytime to stop and get your performance report");
    println!();
}
