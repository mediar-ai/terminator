#[cfg(test)]
mod minimal_tests {
    use serde_json::json;
    use std::time::{Duration, Instant};
    use terminator::{Desktop, Selector};

    // Simple test result structure for accuracy measurement
    #[derive(Debug)]
    struct TestResult {
        name: String,
        passed: bool,
        duration: Duration,
        error: Option<String>,
    }

    impl TestResult {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                passed: false,
                duration: Duration::from_secs(0),
                error: None,
            }
        }

        fn success(mut self, duration: Duration) -> Self {
            self.passed = true;
            self.duration = duration;
            self
        }

        fn failure(mut self, error: String, duration: Duration) -> Self {
            self.passed = false;
            self.error = Some(error);
            self.duration = duration;
            self
        }
    }

    async fn run_test<F, Fut>(name: &str, test_fn: F) -> TestResult
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<(), String>>,
    {
        let start = Instant::now();
        let result = TestResult::new(name);

        match test_fn().await {
            Ok(()) => result.success(start.elapsed()),
            Err(e) => result.failure(e, start.elapsed()),
        }
    }

    #[tokio::test]
    async fn test_desktop_initialization() {
        let result = run_test("Desktop Initialization", || async {
            Desktop::new(false, false)
                .map(|_| ())
                .map_err(|e| format!("Failed to create desktop: {}", e))
        })
        .await;

        assert!(
            result.passed,
            "Desktop initialization failed: {:?}",
            result.error
        );
        println!("✓ Desktop initialization: {:?}", result.duration);
    }

    #[tokio::test]
    async fn test_list_applications() {
        let result = run_test("List Applications", || async {
            let desktop = Desktop::new(false, false)
                .map_err(|e| format!("Failed to create desktop: {}", e))?;

            let apps = desktop
                .applications()
                .map_err(|e| format!("Failed to list applications: {}", e))?;

            if apps.is_empty() {
                return Err("No applications found".to_string());
            }

            println!("Found {} applications", apps.len());
            for app in apps.iter().take(5) {
                if let Some(name) = app.name() {
                    println!("  - {}", name);
                }
            }

            Ok(())
        })
        .await;

        assert!(
            result.passed,
            "List applications failed: {:?}",
            result.error
        );
        println!("✓ List applications: {:?}", result.duration);
    }

    #[tokio::test]
    async fn test_focused_element() {
        let result = run_test("Get Focused Element", || async {
            let desktop = Desktop::new(false, false)
                .map_err(|e| format!("Failed to create desktop: {}", e))?;

            let element = desktop
                .focused_element()
                .map_err(|e| format!("Failed to get focused element: {}", e))?;

            println!("Focused element:");
            println!("  Role: {}", element.role());
            if let Some(name) = element.name() {
                println!("  Name: {}", name);
            }

            match element.process_id() {
                Ok(pid) => println!("  PID: {}", pid),
                Err(_) => println!("  PID: unavailable"),
            }

            Ok(())
        })
        .await;

        assert!(
            result.passed,
            "Get focused element failed: {:?}",
            result.error
        );
        println!("✓ Get focused element: {:?}", result.duration);
    }

    #[tokio::test]
    async fn test_locator_creation() {
        let result = run_test("Locator Creation", || async {
            let desktop = Desktop::new(false, false)
                .map_err(|e| format!("Failed to create desktop: {}", e))?;

            // Test different selector formats
            let selectors = vec!["button|OK", "#12345", "name:Submit", "Document|Main Window"];

            for selector_str in selectors {
                let selector = Selector::from(selector_str);
                let _locator = desktop.locator(selector);
                println!("Created locator for: {}", selector_str);
            }

            Ok(())
        })
        .await;

        assert!(result.passed, "Locator creation failed: {:?}", result.error);
        println!("✓ Locator creation: {:?}", result.duration);
    }

    #[tokio::test]
    async fn test_accuracy_suite() {
        println!("\n=== Running Minimal Accuracy Test Suite ===\n");

        let tests = vec![
            test_desktop_init_accuracy().await,
            test_app_discovery_accuracy().await,
            test_element_detection_accuracy().await,
        ];

        let mut results = Vec::new();
        for (i, result) in tests.into_iter().enumerate() {
            let name = match i {
                0 => "Desktop Initialization",
                1 => "Application Discovery",
                2 => "UI Element Detection",
                _ => "Unknown",
            };

            println!("{}: {}", name, if result.passed { "PASS" } else { "FAIL" });
            if let Some(error) = &result.error {
                println!("  Error: {}", error);
            }
            println!("  Duration: {:?}", result.duration);
            results.push(result);
        }

        // Calculate accuracy
        let total = results.len();
        let passed = results.iter().filter(|r| r.passed).count();
        let accuracy = (passed as f64 / total as f64) * 100.0;

        println!("\n=== Accuracy Summary ===");
        println!("Total tests: {}", total);
        println!("Passed: {}", passed);
        println!("Failed: {}", total - passed);
        println!("Accuracy: {:.1}%", accuracy);
        println!(
            "Total time: {:?}",
            results.iter().map(|r| r.duration).sum::<Duration>()
        );

        assert!(accuracy >= 60.0, "Accuracy too low: {:.1}%", accuracy);
    }

    async fn test_desktop_init_accuracy() -> TestResult {
        run_test("Desktop Init Accuracy", || async {
            // Try multiple times to ensure consistency
            for i in 0..3 {
                let desktop = Desktop::new(false, false)
                    .map_err(|e| format!("Attempt {} failed: {}", i + 1, e))?;

                // Verify desktop is functional
                let _ = desktop
                    .focused_element()
                    .map_err(|e| format!("Desktop not functional: {}", e))?;
            }
            Ok(())
        })
        .await
    }

    async fn test_app_discovery_accuracy() -> TestResult {
        run_test("App Discovery Accuracy", || async {
            let desktop = Desktop::new(false, false)
                .map_err(|e| format!("Failed to create desktop: {}", e))?;

            let apps = desktop
                .applications()
                .map_err(|e| format!("Failed to list applications: {}", e))?;

            // Verify we can get basic info from apps
            let mut valid_apps = 0;
            for app in apps.iter() {
                if app.name().is_some() || app.id().is_some() {
                    valid_apps += 1;
                }
            }

            if valid_apps == 0 {
                return Err("No valid applications found".to_string());
            }

            let validity_rate = (valid_apps as f64 / apps.len() as f64) * 100.0;
            println!("App validity rate: {:.1}%", validity_rate);

            if validity_rate < 80.0 {
                return Err(format!("Low app validity rate: {:.1}%", validity_rate));
            }

            Ok(())
        })
        .await
    }

    async fn test_element_detection_accuracy() -> TestResult {
        run_test("Element Detection Accuracy", || async {
            let desktop = Desktop::new(false, false)
                .map_err(|e| format!("Failed to create desktop: {}", e))?;

            // Get focused element and try to find its window tree
            let element = desktop
                .focused_element()
                .map_err(|e| format!("Failed to get focused element: {}", e))?;

            match element.process_id() {
                Ok(pid) if pid > 0 => {
                    let tree = desktop
                        .get_window_tree(pid, None, None)
                        .map_err(|e| format!("Failed to get window tree: {}", e))?;

                    // Verify tree has content
                    let tree_json = serde_json::to_value(&tree)
                        .map_err(|e| format!("Failed to serialize tree: {}", e))?;

                    if tree_json.is_null()
                        || (tree_json.is_object() && tree_json.as_object().unwrap().is_empty())
                    {
                        return Err("Window tree is empty".to_string());
                    }

                    println!("Successfully retrieved window tree for PID {}", pid);
                }
                Ok(_) => {
                    println!("Focused element has PID 0, skipping tree check");
                }
                Err(e) => {
                    println!("Could not get PID: {}, skipping tree check", e);
                }
            }

            Ok(())
        })
        .await
    }
}
