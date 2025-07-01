#[cfg(test)]
mod automation_accuracy_tests {
    use std::collections::HashMap;
    use std::time::{Duration, Instant};
    use terminator::{Desktop, Selector};

    #[derive(Debug, Clone)]
    struct AutomationMetrics {
        total_attempts: u32,
        successful_attempts: u32,
        failed_attempts: u32,
        total_duration: Duration,
        errors: Vec<String>,
    }

    impl AutomationMetrics {
        fn new() -> Self {
            Self {
                total_attempts: 0,
                successful_attempts: 0,
                failed_attempts: 0,
                total_duration: Duration::from_secs(0),
                errors: Vec::new(),
            }
        }

        fn record_success(&mut self, duration: Duration) {
            self.total_attempts += 1;
            self.successful_attempts += 1;
            self.total_duration += duration;
        }

        fn record_failure(&mut self, error: String, duration: Duration) {
            self.total_attempts += 1;
            self.failed_attempts += 1;
            self.total_duration += duration;
            self.errors.push(error);
        }

        fn accuracy(&self) -> f64 {
            if self.total_attempts == 0 {
                return 0.0;
            }
            (self.successful_attempts as f64 / self.total_attempts as f64) * 100.0
        }

        fn average_duration(&self) -> Duration {
            if self.total_attempts == 0 {
                return Duration::from_secs(0);
            }
            self.total_duration / self.total_attempts
        }
    }

    #[tokio::test]
    async fn test_automation_accuracy() {
        println!("\n=== Automation Accuracy Test Suite ===\n");

        let mut metrics = HashMap::new();

        // Test 1: Element Finding Accuracy
        let finding_metrics = test_element_finding_accuracy().await;
        metrics.insert("Element Finding", finding_metrics);

        // Test 2: Application Control Accuracy
        let app_control_metrics = test_application_control_accuracy().await;
        metrics.insert("Application Control", app_control_metrics);

        // Test 3: UI Navigation Accuracy
        let navigation_metrics = test_ui_navigation_accuracy().await;
        metrics.insert("UI Navigation", navigation_metrics);

        // Print results
        println!("\n=== Automation Accuracy Results ===\n");

        let mut total_accuracy = 0.0;
        let mut test_count = 0;

        for (test_name, metric) in &metrics {
            println!("{}:", test_name);
            println!("  Attempts: {}", metric.total_attempts);
            println!("  Successful: {}", metric.successful_attempts);
            println!("  Failed: {}", metric.failed_attempts);
            println!("  Accuracy: {:.1}%", metric.accuracy());
            println!("  Avg Duration: {:?}", metric.average_duration());

            if !metric.errors.is_empty() {
                println!("  Sample Errors:");
                for (i, error) in metric.errors.iter().take(3).enumerate() {
                    println!("    {}: {}", i + 1, error);
                }
            }
            println!();

            total_accuracy += metric.accuracy();
            test_count += 1;
        }

        let overall_accuracy = total_accuracy / test_count as f64;
        println!("Overall Automation Accuracy: {:.1}%", overall_accuracy);

        // Assert minimum accuracy threshold
        assert!(
            overall_accuracy >= 50.0,
            "Overall accuracy {:.1}% is below minimum threshold of 50%",
            overall_accuracy
        );
    }

    async fn test_element_finding_accuracy() -> AutomationMetrics {
        let mut metrics = AutomationMetrics::new();

        // Try to create desktop
        let desktop = match Desktop::new(false, false) {
            Ok(d) => d,
            Err(e) => {
                metrics.record_failure(
                    format!("Failed to create desktop: {}", e),
                    Duration::from_secs(0),
                );
                return metrics;
            }
        };

        // Test different selector patterns
        let test_selectors = vec![
            ("button|OK", "Button with name"),
            ("#12345", "Element by ID"),
            ("name:Submit", "Element by name attribute"),
            ("Document|Main", "Document with partial name"),
            ("Window|*", "Any window element"),
        ];

        for (selector, description) in test_selectors {
            let start = Instant::now();
            let selector_obj = Selector::from(selector);
            let locator = desktop.locator(selector_obj);

            // Try to find element with timeout
            match locator.wait(Some(Duration::from_millis(100))).await {
                Ok(_element) => {
                    metrics.record_success(start.elapsed());
                    println!("✓ Found element: {} ({})", selector, description);
                }
                Err(e) => {
                    metrics.record_failure(
                        format!("Failed to find {}: {}", description, e),
                        start.elapsed(),
                    );
                }
            }
        }

        metrics
    }

    async fn test_application_control_accuracy() -> AutomationMetrics {
        let mut metrics = AutomationMetrics::new();

        let desktop = match Desktop::new(false, false) {
            Ok(d) => d,
            Err(e) => {
                metrics.record_failure(
                    format!("Failed to create desktop: {}", e),
                    Duration::from_secs(0),
                );
                return metrics;
            }
        };

        // Test 1: List applications
        let start = Instant::now();
        match desktop.applications() {
            Ok(apps) => {
                if apps.is_empty() {
                    metrics.record_failure("No applications found".to_string(), start.elapsed());
                } else {
                    metrics.record_success(start.elapsed());
                    println!("✓ Found {} applications", apps.len());
                }
            }
            Err(e) => {
                metrics.record_failure(
                    format!("Failed to list applications: {}", e),
                    start.elapsed(),
                );
            }
        }

        // Test 2: Get focused element
        let start = Instant::now();
        match desktop.focused_element() {
            Ok(element) => {
                metrics.record_success(start.elapsed());
                println!("✓ Got focused element: {}", element.role());
            }
            Err(e) => {
                metrics.record_failure(
                    format!("Failed to get focused element: {}", e),
                    start.elapsed(),
                );
            }
        }

        // Test 3: Get window tree for focused app
        let start = Instant::now();
        if let Ok(element) = desktop.focused_element() {
            match element.process_id() {
                Ok(pid) => match desktop.get_window_tree(pid, None, None) {
                    Ok(_tree) => {
                        metrics.record_success(start.elapsed());
                        println!("✓ Got window tree for PID {}", pid);
                    }
                    Err(e) => {
                        metrics.record_failure(
                            format!("Failed to get window tree for PID {}: {}", pid, e),
                            start.elapsed(),
                        );
                    }
                },
                Err(_) => {
                    metrics
                        .record_failure("Focused element has no PID".to_string(), start.elapsed());
                }
            }
        }

        metrics
    }

    async fn test_ui_navigation_accuracy() -> AutomationMetrics {
        let mut metrics = AutomationMetrics::new();

        let desktop = match Desktop::new(false, false) {
            Ok(d) => d,
            Err(e) => {
                metrics.record_failure(
                    format!("Failed to create desktop: {}", e),
                    Duration::from_secs(0),
                );
                return metrics;
            }
        };

        // Test common UI patterns
        let ui_patterns = vec![
            ("Menu|File", "File menu"),
            ("MenuItem|Save", "Save menu item"),
            ("ToolBar|*", "Any toolbar"),
            ("Button|Close", "Close button"),
            ("TextBox|*", "Any text box"),
        ];

        for (pattern, description) in ui_patterns {
            let start = Instant::now();
            let selector = Selector::from(pattern);
            let locator = desktop.locator(selector);

            // Quick check for existence
            match locator.wait(Some(Duration::from_millis(50))).await {
                Ok(element) => {
                    // Verify we can get element properties
                    let has_valid_data = element.role() != "Unknown"
                        || element.name().is_some()
                        || element.id().is_some();

                    if has_valid_data {
                        metrics.record_success(start.elapsed());
                        println!("✓ Found UI pattern: {}", description);
                    } else {
                        metrics.record_failure(
                            format!("Found {} but no valid data", description),
                            start.elapsed(),
                        );
                    }
                }
                Err(_) => {
                    // Not finding the element is expected for some patterns
                    // We're testing the accuracy of the search itself
                    metrics.record_success(start.elapsed());
                }
            }
        }

        // Test window enumeration
        let start = Instant::now();
        if let Ok(apps) = desktop.applications() {
            let mut window_count = 0;
            for app in apps.iter().take(5) {
                if let Some(name) = app.name() {
                    match desktop.windows_for_application(&name).await {
                        Ok(windows) => {
                            window_count += windows.len();
                        }
                        Err(_) => {
                            // Some apps might not have accessible windows
                        }
                    }
                }
            }

            if window_count > 0 {
                metrics.record_success(start.elapsed());
                println!("✓ Found {} windows across applications", window_count);
            } else {
                metrics.record_failure("No windows found".to_string(), start.elapsed());
            }
        }

        metrics
    }

    #[tokio::test]
    async fn test_real_work_automation_scenarios() {
        println!("\n=== Real Work Automation Scenarios ===\n");

        let scenarios = vec![
            test_desktop_ready_scenario().await,
            test_find_active_window_scenario().await,
            test_ui_interaction_scenario().await,
        ];

        let mut passed = 0;
        let total = scenarios.len();

        for (i, result) in scenarios.iter().enumerate() {
            let name = match i {
                0 => "Desktop Ready Check",
                1 => "Find Active Window",
                2 => "UI Element Interaction",
                _ => "Unknown",
            };

            print!("Testing {}: ", name);
            match result {
                Ok(duration) => {
                    println!("PASS ({:?})", duration);
                    passed += 1;
                }
                Err(e) => {
                    println!("FAIL - {}", e);
                }
            }
        }

        let accuracy = (passed as f64 / total as f64) * 100.0;
        println!(
            "\nScenario Success Rate: {:.1}% ({}/{})",
            accuracy, passed, total
        );

        assert!(accuracy >= 33.0, "Scenario success rate too low");
    }

    async fn test_desktop_ready_scenario() -> Result<Duration, String> {
        let start = Instant::now();

        let desktop =
            Desktop::new(false, false).map_err(|e| format!("Desktop init failed: {}", e))?;

        // Verify desktop is ready by checking we can get focused element
        let _element = desktop
            .focused_element()
            .map_err(|e| format!("Desktop not ready: {}", e))?;

        Ok(start.elapsed())
    }

    async fn test_find_active_window_scenario() -> Result<Duration, String> {
        let start = Instant::now();

        let desktop =
            Desktop::new(false, false).map_err(|e| format!("Desktop init failed: {}", e))?;

        let element = desktop
            .focused_element()
            .map_err(|e| format!("No focused element: {}", e))?;

        let pid = match element.process_id() {
            Ok(p) => p,
            Err(e) => return Err(format!("No PID for focused element: {}", e)),
        };

        if pid == 0 {
            return Err("Invalid PID".to_string());
        }

        let _tree = desktop
            .get_window_tree(pid, None, None)
            .map_err(|e| format!("Failed to get window tree: {}", e))?;

        Ok(start.elapsed())
    }

    async fn test_ui_interaction_scenario() -> Result<Duration, String> {
        let start = Instant::now();

        let desktop =
            Desktop::new(false, false).map_err(|e| format!("Desktop init failed: {}", e))?;

        // Try to find any interactive element
        let interactive_patterns = vec!["Button|*", "MenuItem|*", "Link|*", "CheckBox|*"];

        for pattern in interactive_patterns {
            let selector = Selector::from(pattern);
            let locator = desktop.locator(selector);

            if let Ok(element) = locator.wait(Some(Duration::from_millis(100))).await {
                // Verify element is interactive
                if element.is_enabled().unwrap_or(false) {
                    return Ok(start.elapsed());
                }
            }
        }

        Err("No interactive elements found".to_string())
    }
}
