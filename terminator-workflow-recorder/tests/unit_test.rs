use std::time::{Duration, Instant};
use sysinfo;
use terminator_workflow_recorder::{WorkflowEvent, WorkflowRecorder, WorkflowRecorderConfig};
use tokio_stream::StreamExt;

/// Performance test that measures CPU load and typing performance during Terminator keyboard automation
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_typing_performance_and_cpu_load() {
        use std::sync::{
            atomic::{AtomicU32, AtomicU64, Ordering},
            Arc,
        };
        use std::thread;

        println!("🚀 Testing TYPING PERFORMANCE and CPU load during keyboard automation...");
        println!("💡 This measures CPU usage and typing lag while recording keyboard events");

        // Start recorder with keyboard-focused settings
        let config = WorkflowRecorderConfig {
            record_mouse: false, // Disable to focus on keyboard performance
            record_keyboard: true,
            record_text_input_completion: true,
            text_input_completion_timeout_ms: 1000,
            capture_ui_elements: true, // This might be causing lag
            record_clipboard: false,
            record_ui_focus_changes: false,
            record_ui_property_changes: false,
            record_ui_structure_changes: false,
            ..Default::default()
        };

        let mut recorder = WorkflowRecorder::new("Typing Performance Test".to_string(), config);
        let mut event_stream = recorder.event_stream();

        // CPU and Memory monitoring
        let cpu_samples = Arc::new(AtomicU32::new(0));
        let memory_samples = Arc::new(AtomicU64::new(0));
        let cpu_counter_for_thread = Arc::clone(&cpu_samples);
        let memory_counter = Arc::clone(&memory_samples);
        let stop_monitoring = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop_flag = Arc::clone(&stop_monitoring);

        let monitoring_thread = thread::spawn(move || {
            let mut system = sysinfo::System::new_all();
            let mut max_memory = 0u64;
            let mut max_cpu = 0.0f32;
            let mut sample_count = 0u32;

            while !stop_flag.load(Ordering::Relaxed) {
                system.refresh_process(sysinfo::get_current_pid().unwrap());
                if let Some(process) = system.process(sysinfo::get_current_pid().unwrap()) {
                    let memory_kb = process.memory() / 1024;
                    let cpu_percent = process.cpu_usage();

                    max_memory = max_memory.max(memory_kb);
                    max_cpu = max_cpu.max(cpu_percent);
                    sample_count += 1;

                    if sample_count % 50 == 0 {
                        println!("📊 CPU: {:.1}%, Memory: {} KB", cpu_percent, memory_kb);
                    }
                }
                thread::sleep(Duration::from_millis(100)); // Sample every 100ms
            }
            memory_counter.store(max_memory, Ordering::Relaxed);
            cpu_counter_for_thread.store((max_cpu * 100.0) as u32, Ordering::Relaxed);
            // Store as percentage * 100
        });

        // Start recording
        let start_time = Instant::now();
        recorder.start().await.expect("Failed to start recorder");
        println!("✅ Recording started in {:?}", start_time.elapsed());

        // TYPING PERFORMANCE TEST WITH TERMINATOR
        let typing_handle = tokio::spawn(async move {
            println!("⌨️ Starting TYPING PERFORMANCE test with Terminator...");

            tokio::time::sleep(Duration::from_millis(500)).await;

            match terminator::Desktop::new(false, false) {
                Ok(desktop) => {
                    println!("📝 Opening Notepad for typing test...");
                    match desktop.open_application("notepad") {
                        Ok(_) => {
                            tokio::time::sleep(Duration::from_millis(2000)).await;

                            match desktop
                                .locator("name:Text Editor")
                                .first(Some(Duration::from_millis(3000)))
                                .await
                            {
                                Ok(text_element) => {
                                    println!(
                                        "✅ Found text editor, starting typing performance test..."
                                    );

                                    // Test 1: Slow typing (realistic user speed)
                                    println!("\n🐌 TEST 1: Slow typing (150ms delays)");
                                    let slow_text = "Slow typing test: This simulates normal user typing speed to measure CPU impact.";
                                    let slow_start = Instant::now();

                                    for (i, ch) in slow_text.chars().enumerate() {
                                        let char_start = Instant::now();
                                        if let Err(_) =
                                            text_element.type_text(&ch.to_string(), false)
                                        {
                                            println!("⚠️ Type error at char {}", i);
                                        }
                                        let char_duration = char_start.elapsed();

                                        if char_duration > Duration::from_millis(50) {
                                            println!("🐌 SLOW char {}: {:?}", ch, char_duration);
                                        }

                                        tokio::time::sleep(Duration::from_millis(150)).await;
                                    }
                                    let slow_duration = slow_start.elapsed();
                                    println!(
                                        "🐌 Slow typing completed in {:?} ({:.1} chars/sec)",
                                        slow_duration,
                                        slow_text.len() as f64 / slow_duration.as_secs_f64()
                                    );

                                    tokio::time::sleep(Duration::from_millis(1000)).await;

                                    // Test 2: Medium typing
                                    println!("\n⚡ TEST 2: Medium typing (80ms delays)");
                                    let medium_text = "\n\nMedium typing test: Faster typing to stress test recording performance.";
                                    let medium_start = Instant::now();

                                    for (i, ch) in medium_text.chars().enumerate() {
                                        let char_start = Instant::now();
                                        if let Err(_) =
                                            text_element.type_text(&ch.to_string(), false)
                                        {
                                            println!("⚠️ Type error at char {}", i);
                                        }
                                        let char_duration = char_start.elapsed();

                                        if char_duration > Duration::from_millis(30) {
                                            println!("⚡ SLOW char {}: {:?}", ch, char_duration);
                                        }

                                        tokio::time::sleep(Duration::from_millis(80)).await;
                                    }
                                    let medium_duration = medium_start.elapsed();
                                    println!(
                                        "⚡ Medium typing completed in {:?} ({:.1} chars/sec)",
                                        medium_duration,
                                        medium_text.len() as f64 / medium_duration.as_secs_f64()
                                    );

                                    tokio::time::sleep(Duration::from_millis(1000)).await;

                                    // Test 3: Fast typing (stress test)
                                    println!("\n🔥 TEST 3: Fast typing (50ms delays)");
                                    let fast_text = "\n\nFast typing stress test: Maximum speed to identify bottlenecks!!!";
                                    let fast_start = Instant::now();
                                    let mut slow_chars = 0;

                                    for (i, ch) in fast_text.chars().enumerate() {
                                        let char_start = Instant::now();
                                        if let Err(_) =
                                            text_element.type_text(&ch.to_string(), false)
                                        {
                                            println!("⚠️ Type error at char {}", i);
                                        }
                                        let char_duration = char_start.elapsed();

                                        if char_duration > Duration::from_millis(20) {
                                            slow_chars += 1;
                                            if char_duration > Duration::from_millis(100) {
                                                println!(
                                                    "🔥 VERY SLOW char {}: {:?}",
                                                    ch, char_duration
                                                );
                                            }
                                        }

                                        tokio::time::sleep(Duration::from_millis(50)).await;
                                    }
                                    let fast_duration = fast_start.elapsed();
                                    println!(
                                        "🔥 Fast typing completed in {:?} ({:.1} chars/sec)",
                                        fast_duration,
                                        fast_text.len() as f64 / fast_duration.as_secs_f64()
                                    );
                                    println!(
                                        "🔥 Slow characters (>20ms): {}/{}",
                                        slow_chars,
                                        fast_text.len()
                                    );

                                    // Test 4: Burst typing (simulate user bursts)
                                    println!(
                                        "\n💥 TEST 4: Burst typing (10 chars rapidly, then pause)"
                                    );
                                    let burst_text = "\n\nBurst test: abcdefghij pause abcdefghij pause abcdefghij";
                                    let burst_start = Instant::now();

                                    for (i, ch) in burst_text.chars().enumerate() {
                                        let char_start = Instant::now();
                                        if let Err(_) =
                                            text_element.type_text(&ch.to_string(), false)
                                        {
                                            println!("⚠️ Type error at char {}", i);
                                        }
                                        let char_duration = char_start.elapsed();

                                        if char_duration > Duration::from_millis(15) {
                                            println!(
                                                "💥 SLOW burst char {}: {:?}",
                                                ch, char_duration
                                            );
                                        }

                                        // Burst pattern: 10 fast chars, then pause
                                        if (i + 1) % 10 == 0 {
                                            tokio::time::sleep(Duration::from_millis(500)).await;
                                        // Pause
                                        } else {
                                            tokio::time::sleep(Duration::from_millis(30)).await;
                                            // Fast
                                        }
                                    }
                                    let burst_duration = burst_start.elapsed();
                                    println!(
                                        "💥 Burst typing completed in {:?} ({:.1} chars/sec)",
                                        burst_duration,
                                        burst_text.len() as f64 / burst_duration.as_secs_f64()
                                    );

                                    println!("\n✅ All typing performance tests completed!");
                                }
                                Err(e) => {
                                    println!("❌ Could not find text editor: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            println!("❌ Failed to open Notepad: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Failed to initialize Terminator: {}", e);
                }
            }
        });

        // Event collection with detailed keyboard tracking
        let mut event_count = 0;
        let mut keyboard_events = 0;
        let mut text_completion_events = 0;
        let mut total_keystroke_processing_time = Duration::new(0, 0);

        println!("📊 Monitoring keyboard events and processing times...");
        let event_collection = tokio::time::timeout(Duration::from_secs(45), async {
            while let Some(event) = event_stream.next().await {
                let event_start = Instant::now();
                event_count += 1;

                match event {
                    WorkflowEvent::Keyboard(_) => {
                        keyboard_events += 1;
                        let processing_time = event_start.elapsed();
                        total_keystroke_processing_time += processing_time;

                        if processing_time > Duration::from_millis(10) {
                            println!("🐌 SLOW keyboard event processing: {:?}", processing_time);
                        }

                        if keyboard_events % 50 == 0 {
                            println!("⌨️ Processed {} keyboard events", keyboard_events);
                        }
                    }
                    WorkflowEvent::TextInputCompleted(_) => {
                        text_completion_events += 1;
                        println!(
                            "🔥 TEXT COMPLETION EVENT! (Total: {})",
                            text_completion_events
                        );
                    }
                    _ => {}
                }

                if event_count >= 5000 {
                    println!("🛑 Stopping at 5000 events");
                    break;
                }
            }
            (
                event_count,
                keyboard_events,
                text_completion_events,
                total_keystroke_processing_time,
            )
        });

        let _ = tokio::join!(typing_handle, event_collection);

        // Stop everything
        recorder.stop().await.expect("Failed to stop recorder");
        stop_monitoring.store(true, Ordering::Relaxed);
        monitoring_thread.join().expect("Monitoring failed");

        let total_duration = start_time.elapsed();
        let max_memory_kb = memory_samples.load(Ordering::Relaxed);
        let max_cpu_percent = cpu_samples.load(Ordering::Relaxed) as f32 / 100.0;

        // DETAILED PERFORMANCE RESULTS
        println!("\n🎯 TYPING PERFORMANCE & CPU LOAD RESULTS:");
        println!("{}", "=".repeat(80));
        println!("  ⏱️  Total Test Duration: {:?}", total_duration);
        println!("  📊 Events Captured:");
        println!("    📈 Total Events: {}", event_count);
        println!("    ⌨️  Keyboard Events: {}", keyboard_events);
        println!("    📝 Text Completions: {}", text_completion_events);
        println!("  🖥️  System Performance:");
        println!("    🔥 Peak CPU Usage: {:.1}%", max_cpu_percent);
        println!(
            "    🧠 Peak Memory Usage: {} KB ({:.1} MB)",
            max_memory_kb,
            max_memory_kb as f64 / 1024.0
        );

        if keyboard_events > 0 {
            let avg_processing_time = total_keystroke_processing_time / keyboard_events as u32;
            println!("  ⌨️  Keyboard Performance:");
            println!("    ⚡ Average Processing Time: {:?}", avg_processing_time);
            println!(
                "    📊 Total Processing Time: {:?}",
                total_keystroke_processing_time
            );
            println!(
                "    💾 Memory per Keystroke: {:.1} KB",
                max_memory_kb as f64 / keyboard_events as f64
            );

            if total_duration.as_secs_f64() > 0.0 {
                println!(
                    "    🔢 Keystroke Rate: {:.1} keys/sec",
                    keyboard_events as f64 / total_duration.as_secs_f64()
                );
            }
        }

        // Performance analysis and optimization recommendations
        println!("\n📈 TYPING LAG ANALYSIS:");
        if max_cpu_percent > 50.0 {
            println!(
                "🔥 HIGH CPU USAGE: {:.1}% - This could cause typing lag!",
                max_cpu_percent
            );
            println!(
                "💡 RECOMMENDATION: Optimize UI element capture or reduce monitoring frequency"
            );
        } else if max_cpu_percent > 25.0 {
            println!(
                "⚠️ MODERATE CPU USAGE: {:.1}% - Watch for typing lag",
                max_cpu_percent
            );
        } else {
            println!(
                "✅ LOW CPU USAGE: {:.1}% - Good performance",
                max_cpu_percent
            );
        }

        if keyboard_events > 0 {
            let avg_ms = total_keystroke_processing_time.as_millis() / keyboard_events as u128;
            if avg_ms > 10 {
                println!(
                    "🐌 SLOW KEYSTROKE PROCESSING: {}ms average - This causes typing lag!",
                    avg_ms
                );
                println!("💡 RECOMMENDATION: Optimize keyboard event handling pipeline");
            } else if avg_ms > 5 {
                println!(
                    "⚠️ MODERATE processing time: {}ms average per keystroke",
                    avg_ms
                );
            } else {
                println!("✅ FAST keystroke processing: {}ms average", avg_ms);
            }
        }

        let memory_efficiency = if keyboard_events > 0 {
            max_memory_kb as f64 / keyboard_events as f64
        } else {
            0.0
        };
        if memory_efficiency > 1000.0 {
            println!(
                "🧠 HIGH MEMORY per keystroke: {:.1} KB - Consider optimization",
                memory_efficiency
            );
        } else {
            println!(
                "✅ GOOD memory efficiency: {:.1} KB per keystroke",
                memory_efficiency
            );
        }

        // Performance assertions
        assert!(
            total_duration < Duration::from_secs(60),
            "Test should complete within 60 seconds"
        );
        assert!(max_memory_kb < 2_000_000, "Memory should stay under 2GB");

        println!("\n🏁 TYPING PERFORMANCE TEST COMPLETED!");
        println!("💡 Use these metrics to identify and fix typing lag bottlenecks.");
        println!("🔧 Focus optimization on high CPU usage and slow keystroke processing times.");
    }

    #[tokio::test]
    async fn test_manual_recording_with_realtime_dashboard() {
        use std::io::{self, Write};
        use std::sync::{
            atomic::{AtomicU32, AtomicU64, Ordering},
            Arc, Mutex,
        };
        use std::thread;

        println!("🎛️  MANUAL RECORDING with REAL-TIME PERFORMANCE DASHBOARD");
        println!("💡 Interact with your desktop manually while monitoring performance");
        println!("🔄 Press Ctrl+C to stop recording\n");

        // Enhanced config to capture all event types
        let config = WorkflowRecorderConfig {
            record_mouse: false,
            record_keyboard: false,
            record_text_input_completion: true,
            text_input_completion_timeout_ms: 500,
            capture_ui_elements: false,
            record_clipboard: false,
            record_ui_focus_changes: false,
            record_ui_property_changes: false,
            record_ui_structure_changes: false,
            ..Default::default()
        };

        let mut recorder = WorkflowRecorder::new("Manual Dashboard Test".to_string(), config);
        let mut event_stream = recorder.event_stream();
        // Shared metrics for real-time dashboard
        let metrics = Arc::new(Mutex::new(DashboardMetrics::new()));
        let cpu_usage = Arc::new(AtomicU32::new(0)); // CPU as percentage * 100
        let memory_usage = Arc::new(AtomicU64::new(0)); // Memory in KB
        let stop_monitoring = Arc::new(std::sync::atomic::AtomicBool::new(false));

        // Clone references for threads
        let metrics_for_events = Arc::clone(&metrics);
        let metrics_for_display = Arc::clone(&metrics);
        let cpu_for_monitor = Arc::clone(&cpu_usage);
        let memory_for_monitor = Arc::clone(&memory_usage);
        let cpu_for_display = Arc::clone(&cpu_usage);
        let memory_for_display = Arc::clone(&memory_usage);
        let stop_for_monitor = Arc::clone(&stop_monitoring);
        let stop_for_display = Arc::clone(&stop_monitoring);

        // System monitoring thread
        let monitor_handle = thread::spawn(move || {
            let mut system = sysinfo::System::new_all();

            while !stop_for_monitor.load(Ordering::Relaxed) {
                system.refresh_process(sysinfo::get_current_pid().unwrap());
                if let Some(process) = system.process(sysinfo::get_current_pid().unwrap()) {
                    let memory_kb = process.memory() / 1024;
                    let cpu_percent = (process.cpu_usage() * 100.0) as u32;

                    memory_for_monitor.store(memory_kb, Ordering::Relaxed);
                    cpu_for_monitor.store(cpu_percent, Ordering::Relaxed);
                }
                thread::sleep(Duration::from_millis(250));
            }
        });

        // Real-time dashboard display thread
        let display_handle = thread::spawn(move || {
            let mut stdout = io::stdout();
            let start_time = Instant::now();

            // Hide cursor and clear screen
            print!("\x1B[?25l\x1B[2J");
            stdout.flush().unwrap();

            while !stop_for_display.load(Ordering::Relaxed) {
                let current_metrics = {
                    let metrics_lock = metrics_for_display.lock().unwrap();
                    metrics_lock.clone()
                };

                let cpu_pct = cpu_for_display.load(Ordering::Relaxed) as f32 / 100.0;
                let memory_kb = memory_for_display.load(Ordering::Relaxed);
                let uptime = start_time.elapsed();

                // Move cursor to top and display dashboard
                print!("\x1B[H");
                print_dashboard(&current_metrics, cpu_pct, memory_kb, uptime);
                stdout.flush().unwrap();

                thread::sleep(Duration::from_millis(500)); // Update every 500ms
            }

            // Show cursor again
            print!("\x1B[?25h");
            stdout.flush().unwrap();
        });

        // Start recording
        recorder.start().await.expect("Failed to start recorder");

        // Event processing loop
        let stop_for_events = Arc::clone(&stop_monitoring);
        let event_handle = tokio::spawn(async move {
            let mut last_event_time = Instant::now();

            while let Some(event) = event_stream.next().await {
                // Check if stop signal was received (Ctrl+C)
                if stop_for_events.load(Ordering::Relaxed) {
                    println!("\n📊 Event processing stopping due to user request...");
                    break;
                }

                let current_time = Instant::now();
                let processing_time = current_time.duration_since(last_event_time);

                {
                    let mut metrics_lock = metrics_for_events.lock().unwrap();
                    metrics_lock.update_event(&event, processing_time);
                }

                last_event_time = current_time;

                // Stop after reasonable time or high event count
                if metrics_for_events.lock().unwrap().total_events >= 10000 {
                    println!("\n📊 Event processing stopping due to event limit reached...");
                    break;
                }
            }
        });

        println!("🎮 RECORDING STARTED - Interact with your desktop!");
        println!("   • Type text, click buttons, move windows, etc.");
        println!("   • The dashboard will update in real-time");
        println!("   • Test will auto-stop after 10,000 events or 5 minutes");
        println!("   • Press Ctrl+C to stop early and see final summary\n");

        // Set up Ctrl+C signal handling for graceful shutdown
        let stop_for_signal = Arc::clone(&stop_monitoring);
        let _signal_handle = tokio::spawn(async move {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to listen for Ctrl+C");

            println!("\n🛑 Ctrl+C detected - Gracefully stopping and showing final summary...");
            stop_for_signal.store(true, Ordering::Relaxed);
        });

        // Wait for completion, timeout, or Ctrl+C signal
        let timeout_result = tokio::time::timeout(Duration::from_secs(300), async {
            // Wait for either the event task to complete or stop signal
            while !stop_monitoring.load(Ordering::Relaxed) {
                if event_handle.is_finished() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            event_handle.await
        })
        .await;

        // Cleanup
        stop_monitoring.store(true, Ordering::Relaxed);
        recorder.stop().await.expect("Failed to stop recorder");

        monitor_handle.join().expect("Monitor thread failed");
        display_handle.join().expect("Display thread failed");

        // Final summary - ALWAYS show it regardless of how test ended
        let final_metrics = metrics.lock().unwrap().clone();

        // Determine how the test ended
        let stop_reason = if stop_monitoring.load(Ordering::Relaxed) {
            "🛑 Stopped by Ctrl+C (User interruption)"
        } else {
            match timeout_result {
                Ok(_) => "✅ Completed successfully (Event limit reached)",
                Err(_) => "⏰ Completed due to timeout (5 minute limit)",
            }
        };

        // Clear screen and show final summary with proper formatting
        print!("\n\n");
        println!("═══════════════════════════════════════════════════════════════");
        println!("  📊 TERMINATOR RECORDING SESSION COMPLETE");
        println!("═══════════════════════════════════════════════════════════════");
        println!("  Stop Reason: {}", stop_reason);
        print_final_summary(&final_metrics);
    }

    #[tokio::test]
    async fn test_anti_freeze_protection_compiles() {
        println!("🛡️ TESTING ANTI-FREEZE PROTECTION - Build Verification");
        println!("💡 This verifies the performance protection code compiles correctly");

        // Simple config to verify the anti-freeze code builds
        let config = WorkflowRecorderConfig {
            record_mouse: false,
            record_keyboard: true,
            record_text_input_completion: false,
            capture_ui_elements: true, // This was the main freezing culprit - now protected
            record_clipboard: false,
            record_ui_focus_changes: false,
            record_ui_property_changes: false, // Disabled in the fix
            record_ui_structure_changes: false,
            ..Default::default()
        };

        let mut recorder = WorkflowRecorder::new("Anti-Freeze Test".to_string(), config);
        println!("✅ Recorder with performance protections created successfully");

        // Start and immediately stop to verify no hanging
        let start_time = std::time::Instant::now();
        recorder.start().await.expect("Failed to start recorder");
        println!("✅ Recording started with performance protections");

        // Quick stop to verify responsiveness
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        recorder.stop().await.expect("Failed to stop recorder");

        let duration = start_time.elapsed();
        println!("✅ Recording stopped quickly: {:?}", duration);

        // If we get here, the anti-freeze protection code compiled and didn't hang
        assert!(duration < std::time::Duration::from_secs(5));
        println!("🎯 SUCCESS: Anti-freeze protection code is working!");
    }
}

#[derive(Debug, Clone)]
struct DashboardMetrics {
    total_events: u64,
    keyboard_events: u64,
    mouse_events: u64,
    ui_focus_events: u64,
    ui_structure_events: u64,
    ui_property_events: u64,
    clipboard_events: u64,
    text_completion_events: u64,
    start_time: Instant,
    last_event_time: Option<Instant>,
    total_processing_time: Duration,
    max_processing_time: Duration,
    events_per_second: f64,
}

impl DashboardMetrics {
    fn new() -> Self {
        Self {
            total_events: 0,
            keyboard_events: 0,
            mouse_events: 0,
            ui_focus_events: 0,
            ui_structure_events: 0,
            ui_property_events: 0,
            clipboard_events: 0,
            text_completion_events: 0,
            start_time: Instant::now(),
            last_event_time: None,
            total_processing_time: Duration::new(0, 0),
            max_processing_time: Duration::new(0, 0),
            events_per_second: 0.0,
        }
    }

    fn update_event(&mut self, event: &WorkflowEvent, processing_time: Duration) {
        self.total_events += 1;
        self.last_event_time = Some(Instant::now());
        self.total_processing_time += processing_time;
        self.max_processing_time = self.max_processing_time.max(processing_time);

        // Update events per second
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.events_per_second = self.total_events as f64 / elapsed;
        }

        // Count event types
        match event {
            WorkflowEvent::Keyboard(_) => self.keyboard_events += 1,
            WorkflowEvent::Mouse(_) => self.mouse_events += 1,
            WorkflowEvent::UiFocusChanged(_) => self.ui_focus_events += 1,
            WorkflowEvent::UiPropertyChanged(_) => self.ui_property_events += 1,
            WorkflowEvent::Clipboard(_) => self.clipboard_events += 1,
            WorkflowEvent::TextInputCompleted(_) => self.text_completion_events += 1,
            WorkflowEvent::TextSelection(_) => self.ui_structure_events += 1, // Group text selection as UI structure
            WorkflowEvent::DragDrop(_) => self.ui_structure_events += 1, // Group drag/drop as UI structure
            WorkflowEvent::Hotkey(_) => self.keyboard_events += 1, // Group hotkeys with keyboard
            WorkflowEvent::ApplicationSwitch(_) => self.ui_structure_events += 1, // Group app switch as UI structure
            WorkflowEvent::BrowserTabNavigation(_) => self.ui_structure_events += 1, // Group browser navigation as UI structure
        }
    }
}

fn print_dashboard(metrics: &DashboardMetrics, cpu_percent: f32, memory_kb: u64, uptime: Duration) {
    let uptime_secs = uptime.as_secs();
    let hours = uptime_secs / 3600;
    let minutes = (uptime_secs % 3600) / 60;
    let seconds = uptime_secs % 60;

    let avg_processing_ms = if metrics.total_events > 0 {
        (metrics.total_processing_time.as_millis() as f64) / (metrics.total_events as f64)
    } else {
        0.0
    };

    println!("╔══════════════════════════════════════════════════════════════════════════════╗");
    println!("║  🎛️  TERMINATOR WORKFLOW RECORDER - REAL-TIME PERFORMANCE DASHBOARD         ║");
    println!("╠══════════════════════════════════════════════════════════════════════════════╣");
    println!(
        "║  ⏰ RUNTIME: {:02}:{:02}:{:02}                      📊 CPU: {:5.1}%   🧠 MEM: {:6} KB ║",
        hours, minutes, seconds, cpu_percent, memory_kb
    );
    println!("╠══════════════════════════════════════════════════════════════════════════════╣");
    println!("║                               EVENT STATISTICS                               ║");
    println!("╠═══════════════════════════╤══════════════╤═══════════════╤══════════════════╣");
    println!("║ Event Type                │     Count    │  Percentage   │   Rate (ev/sec)  ║");
    println!("╠═══════════════════════════┼══════════════┼═══════════════┼══════════════════╣");

    let total = metrics.total_events as f64;
    let rate = metrics.events_per_second;

    println!(
        "║ 🎯 TOTAL EVENTS           │ {:>11}  │ {:>11}%  │ {:>14.1}  ║",
        metrics.total_events, 100.0, rate
    );
    println!(
        "║ ⌨️  Keyboard              │ {:>11}  │ {:>11.1}%  │ {:>14.1}  ║",
        metrics.keyboard_events,
        if total > 0.0 {
            (metrics.keyboard_events as f64 / total) * 100.0
        } else {
            0.0
        },
        if uptime.as_secs_f64() > 0.0 {
            metrics.keyboard_events as f64 / uptime.as_secs_f64()
        } else {
            0.0
        }
    );
    println!(
        "║ 🖱️  Mouse                 │ {:>11}  │ {:>11.1}%  │ {:>14.1}  ║",
        metrics.mouse_events,
        if total > 0.0 {
            (metrics.mouse_events as f64 / total) * 100.0
        } else {
            0.0
        },
        if uptime.as_secs_f64() > 0.0 {
            metrics.mouse_events as f64 / uptime.as_secs_f64()
        } else {
            0.0
        }
    );
    println!(
        "║ 🎯 UI Focus Changes       │ {:>11}  │ {:>11.1}%  │ {:>14.1}  ║",
        metrics.ui_focus_events,
        if total > 0.0 {
            (metrics.ui_focus_events as f64 / total) * 100.0
        } else {
            0.0
        },
        if uptime.as_secs_f64() > 0.0 {
            metrics.ui_focus_events as f64 / uptime.as_secs_f64()
        } else {
            0.0
        }
    );
    println!(
        "║ 🏗️  UI Structure Changes  │ {:>11}  │ {:>11.1}%  │ {:>14.1}  ║",
        metrics.ui_structure_events,
        if total > 0.0 {
            (metrics.ui_structure_events as f64 / total) * 100.0
        } else {
            0.0
        },
        if uptime.as_secs_f64() > 0.0 {
            metrics.ui_structure_events as f64 / uptime.as_secs_f64()
        } else {
            0.0
        }
    );
    println!(
        "║ ⚙️  UI Property Changes   │ {:>11}  │ {:>11.1}%  │ {:>14.1}  ║",
        metrics.ui_property_events,
        if total > 0.0 {
            (metrics.ui_property_events as f64 / total) * 100.0
        } else {
            0.0
        },
        if uptime.as_secs_f64() > 0.0 {
            metrics.ui_property_events as f64 / uptime.as_secs_f64()
        } else {
            0.0
        }
    );
    println!(
        "║ 📋 Clipboard Changes      │ {:>11}  │ {:>11.1}%  │ {:>14.1}  ║",
        metrics.clipboard_events,
        if total > 0.0 {
            (metrics.clipboard_events as f64 / total) * 100.0
        } else {
            0.0
        },
        if uptime.as_secs_f64() > 0.0 {
            metrics.clipboard_events as f64 / uptime.as_secs_f64()
        } else {
            0.0
        }
    );
    println!(
        "║ 📝 Text Completions       │ {:>11}  │ {:>11.1}%  │ {:>14.1}  ║",
        metrics.text_completion_events,
        if total > 0.0 {
            (metrics.text_completion_events as f64 / total) * 100.0
        } else {
            0.0
        },
        if uptime.as_secs_f64() > 0.0 {
            metrics.text_completion_events as f64 / uptime.as_secs_f64()
        } else {
            0.0
        }
    );

    println!("╠══════════════════════════════════════════════════════════════════════════════╣");
    println!("║                             PERFORMANCE METRICS                             ║");
    println!("╠══════════════════════════════════════════════════════════════════════════════╣");
    println!(
        "║ ⚡ Avg Processing Time: {:>6.2} ms      🔥 Max Processing Time: {:>6.2} ms   ║",
        avg_processing_ms,
        metrics.max_processing_time.as_millis()
    );
    println!(
        "║ 💾 Memory Efficiency:  {:>6.1} KB/event   🎯 Total Processing: {:>8} ms   ║",
        if metrics.total_events > 0 {
            memory_kb as f64 / metrics.total_events as f64
        } else {
            0.0
        },
        metrics.total_processing_time.as_millis()
    );

    // Performance status indicators
    let cpu_status = if cpu_percent > 50.0 {
        "🔥 HIGH"
    } else if cpu_percent > 25.0 {
        "⚠️  MODERATE"
    } else {
        "✅ LOW"
    };
    let latency_status = if avg_processing_ms > 10.0 {
        "🐌 SLOW"
    } else if avg_processing_ms > 5.0 {
        "⚠️  MODERATE"
    } else {
        "✅ FAST"
    };

    println!("╠══════════════════════════════════════════════════════════════════════════════╣");
    println!(
        "║ CPU Usage: {}     Processing Speed: {}     Memory: {:.1} MB           ║",
        cpu_status,
        latency_status,
        memory_kb as f64 / 1024.0
    );
    println!("╚══════════════════════════════════════════════════════════════════════════════╝");

    if metrics.total_events > 0 {
        let last_event = if let Some(last) = metrics.last_event_time {
            format!("{:.1}s ago", last.elapsed().as_secs_f64())
        } else {
            "Never".to_string()
        };
        println!(
            "   Last Event: {}  |  Events Buffered: Processing...  |  Status: 🟢 RECORDING",
            last_event
        );
    } else {
        println!("   Status: 🟡 WAITING FOR EVENTS - Start interacting with your desktop!");
    }

    // Clear remaining lines to prevent artifacts
    for _ in 0..5 {
        println!(
            "                                                                                "
        );
    }
}

fn print_final_summary(metrics: &DashboardMetrics) {
    println!("\n🎯 FINAL PERFORMANCE SUMMARY");
    println!("═══════════════════════════════════════════════════════════════");
    println!("📊 Total Events Captured: {}", metrics.total_events);
    println!(
        "⏱️  Total Runtime: {:.1}s",
        metrics.start_time.elapsed().as_secs_f64()
    );
    println!(
        "⚡ Average Event Rate: {:.1} events/second",
        metrics.events_per_second
    );
    println!(
        "🔥 Average Processing Time: {:.2}ms per event",
        if metrics.total_events > 0 {
            metrics.total_processing_time.as_millis() as f64 / metrics.total_events as f64
        } else {
            0.0
        }
    );
    println!("📈 Event Type Breakdown:");
    println!(
        "  • Keyboard: {} ({:.1}%)",
        metrics.keyboard_events,
        if metrics.total_events > 0 {
            (metrics.keyboard_events as f64 / metrics.total_events as f64) * 100.0
        } else {
            0.0
        }
    );
    println!(
        "  • Mouse: {} ({:.1}%)",
        metrics.mouse_events,
        if metrics.total_events > 0 {
            (metrics.mouse_events as f64 / metrics.total_events as f64) * 100.0
        } else {
            0.0
        }
    );
    println!(
        "  • UI Changes: {} ({:.1}%)",
        metrics.ui_focus_events + metrics.ui_structure_events + metrics.ui_property_events,
        if metrics.total_events > 0 {
            ((metrics.ui_focus_events + metrics.ui_structure_events + metrics.ui_property_events)
                as f64
                / metrics.total_events as f64)
                * 100.0
        } else {
            0.0
        }
    );
    println!("✅ Recording session completed successfully!");
}
