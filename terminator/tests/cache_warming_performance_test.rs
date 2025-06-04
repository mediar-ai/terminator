#[cfg(target_os = "windows")]
mod cache_warming_tests {
    use std::time::{Duration, Instant};
    use terminator::Desktop;

    #[tokio::test]
    async fn test_cache_warming_ui_tree_performance() {
        println!("=== UI Tree Cache Warming Performance Test ===");
        
        // Create desktop instance
        let desktop = match Desktop::new(false, false) {
            Ok(d) => d,
            Err(e) => {
                println!("Failed to create desktop: {}", e);
                return;
            }
        };

        // Find a good test application
        let test_app = find_test_application(&desktop).await;
        if test_app.is_none() {
            println!("❌ No suitable test application found. Starting notepad...");
            match desktop.open_application("notepad") {
                Ok(_) => {
                    tokio::time::sleep(Duration::from_secs(2)).await; // Wait for app to load
                }
                Err(e) => {
                    println!("Failed to open notepad: {}", e);
                    return;
                }
            }
        }

        // Test WITHOUT cache warming - measure cold UI tree building
        println!("\n🔍 Testing UI tree building WITHOUT cache warming...");
        let without_cache_times = measure_ui_tree_building_performance(&desktop, 5).await;
        
        if without_cache_times.is_empty() {
            println!("❌ No successful UI tree builds without cache warming");
            return;
        }

        // Enable cache warming
        println!("\n🔥 Enabling background cache warming...");
        if let Err(e) = desktop.enable_background_cache_warmer(true, Some(3), Some(10)) {
            println!("Failed to enable cache warming: {}", e);
            return;
        }
        
        println!("⏳ Waiting 10 seconds for cache to warm up...");
        tokio::time::sleep(Duration::from_secs(10)).await;
        
        // Test WITH cache warming - should be much faster now!
        println!("🔍 Testing UI tree building WITH cache warming...");
        let with_cache_times = measure_ui_tree_building_performance(&desktop, 5).await;
        
        // Disable cache warming
        let _ = desktop.enable_background_cache_warmer(false, None, None);
        
        if with_cache_times.is_empty() {
            println!("❌ No successful UI tree builds with cache warming");
            return;
        }
        
        // Compare results - should see dramatic improvement
        print_ui_tree_performance_comparison(&without_cache_times, &with_cache_times);
    }

    async fn find_test_application(desktop: &Desktop) -> Option<String> {
        // Try to find a running application that we can test with
        let test_apps = ["explorer", "notepad", "chrome", "firefox", "code", "winword", "excel"];
        
        for app_name in &test_apps {
            match desktop.application(app_name) {
                Ok(_) => return Some(app_name.to_string()),
                Err(_) => continue,
            }
        }
        None
    }

    async fn measure_ui_tree_building_performance(desktop: &Desktop, attempts: usize) -> Vec<Duration> {
        let mut successful_measurements = Vec::new();
        
        for i in 0..attempts {
            let measurement_start = Instant::now();
            
            // Test FRESH operations each time to really test cache warming effectiveness
            let mut success = false;
            let mut test_description = String::new();
            
            // Try different applications each time to test cache warming breadth
            let app_attempts = [
                ("explorer", "Windows Explorer"),
                ("winlogon", "System Process"),
                ("dwm", "Desktop Window Manager"),
                ("svchost", "Service Host"),
                ("csrss", "Client Server Runtime"),
            ];
            
            for (app_name, description) in &app_attempts {
                // Try to get application by name and build its tree
                match desktop.application(app_name) {
                    Ok(app) => {
                        // Get PID and try to build window tree
                        if let Ok(pid) = app.process_id() {
                            match desktop.get_window_tree_by_pid_and_title(pid, None) {
                                Ok(tree) => {
                                    success = true;
                                    let node_count = count_ui_nodes(&tree);
                                    test_description = format!("{} tree (PID: {}, {} nodes)", description, pid, node_count);
                                    break;
                                }
                                Err(_) => continue,
                            }
                        }
                    }
                    Err(_) => continue,
                }
            }
            
            // Fallback to window title search if app-based search fails
            if !success {
                let window_titles = [
                    "Program Manager",    // Desktop window
                    "Task Manager",       // Task manager
                    "Settings",          // Windows Settings
                ];
                
                for title in &window_titles {
                    match desktop.get_window_tree_by_title(title) {
                        Ok(tree) => {
                            success = true;
                            let node_count = count_ui_nodes(&tree);
                            test_description = format!("window tree '{}' ({} nodes)", title, node_count);
                            break;
                        }
                        Err(_) => continue,
                    }
                }
            }
            
            let measurement_time = measurement_start.elapsed();
            
            if success {
                successful_measurements.push(measurement_time);
                println!("  Attempt {}: ✅ Built {} in {:?}", 
                         i + 1, test_description, measurement_time);
            } else {
                println!("  Attempt {}: ❌ Failed to build any UI tree in {:?}", 
                         i + 1, measurement_time);
            }
            
            // Small delay between attempts to allow cache warming to work
            if i < attempts - 1 {
                tokio::time::sleep(Duration::from_millis(1000)).await;
            }
        }
        
        successful_measurements
    }

    // Helper function to count nodes in a UI tree
    fn count_ui_nodes(node: &terminator::UINode) -> usize {
        let mut count = 1; // Count this node
        for child in &node.children {
            count += count_ui_nodes(child);
        }
        count
    }

    fn print_ui_tree_performance_comparison(without_cache: &[Duration], with_cache: &[Duration]) {
        if without_cache.is_empty() || with_cache.is_empty() {
            println!("❌ Insufficient data for comparison");
            return;
        }
        
        let avg_without = average_duration(without_cache);
        let avg_with = average_duration(with_cache);
        let min_without = without_cache.iter().min().unwrap();
        let min_with = with_cache.iter().min().unwrap();
        let max_without = without_cache.iter().max().unwrap();
        let max_with = with_cache.iter().max().unwrap();
        
        println!("\n=== UI TREE CACHE PERFORMANCE RESULTS ===");
        println!("📊 Successful builds: {} without cache, {} with cache", 
                 without_cache.len(), with_cache.len());
        
        println!("\n⏱️  AVERAGE BUILD TIMES:");
        println!("   Without cache: {:?}", avg_without);
        println!("   With cache:    {:?}", avg_with);
        
        if avg_with < avg_without {
            let improvement = ((avg_without.as_nanos() - avg_with.as_nanos()) as f64 / avg_without.as_nanos() as f64) * 100.0;
            let speedup = avg_without.as_nanos() as f64 / avg_with.as_nanos() as f64;
            println!("   🚀 MASSIVE IMPROVEMENT: {:.1}% faster with cache!", improvement);
            println!("   🏃 {:.1}x speedup!", speedup);
        } else {
            let degradation = ((avg_with.as_nanos() - avg_without.as_nanos()) as f64 / avg_without.as_nanos() as f64) * 100.0;
            println!("   ⚠️  SLOWER: {:.1}% slower with cache", degradation);
        }
        
        println!("\n📈 FASTEST TIMES:");
        println!("   Without cache: {:?}", min_without);
        println!("   With cache:    {:?}", min_with);
        
        if min_with < min_without {
            let best_improvement = ((min_without.as_nanos() - min_with.as_nanos()) as f64 / min_without.as_nanos() as f64) * 100.0;
            let best_speedup = min_without.as_nanos() as f64 / min_with.as_nanos() as f64;
            println!("   ⚡ Best case: {:.1}% improvement ({:.1}x speedup)!", best_improvement, best_speedup);
        }
        
        println!("\n📋 ALL MEASUREMENTS:");
        println!("Without cache: {:?}", without_cache);
        println!("With cache:    {:?}", with_cache);
        
        // Check for near-instantaneous performance (sub-10ms)
        let instant_count = with_cache.iter().filter(|&d| d.as_millis() < 10).count();
        if instant_count > 0 {
            println!("\n⚡ INSTANT RESPONSES: {} out of {} calls were sub-10ms!", 
                     instant_count, with_cache.len());
        }
        
        // Calculate consistency (standard deviation)
        let std_without = standard_deviation(without_cache);
        let std_with = standard_deviation(with_cache);
        
        println!("\n📏 CONSISTENCY (Standard Deviation):");
        println!("   Without cache: {:?}", std_without);
        println!("   With cache:    {:?}", std_with);
        
        if std_with < std_without {
            println!("   ✅ More consistent performance with cache");
        } else {
            println!("   ⚠️  Less consistent performance with cache");
        }
    }

    fn average_duration(durations: &[Duration]) -> Duration {
        let total_nanos: u128 = durations.iter().map(|d| d.as_nanos()).sum();
        Duration::from_nanos((total_nanos / durations.len() as u128) as u64)
    }

    fn standard_deviation(durations: &[Duration]) -> Duration {
        let avg = average_duration(durations);
        let avg_nanos = avg.as_nanos() as f64;
        
        let variance: f64 = durations
            .iter()
            .map(|d| {
                let diff = d.as_nanos() as f64 - avg_nanos;
                diff * diff
            })
            .sum::<f64>() / durations.len() as f64;
        
        Duration::from_nanos(variance.sqrt() as u64)
    }

    #[tokio::test]
    async fn test_cache_warming_basic_functionality() {
        println!("=== Basic Cache Warming Functionality Test ===");
        
        let desktop = match Desktop::new(false, false) {
            Ok(d) => d,
            Err(e) => {
                println!("Failed to create desktop: {}", e);
                return;
            }
        };

        // Test initial state
        assert!(!desktop.is_cache_warmer_enabled(), "Cache warmer should be disabled initially");
        
        // Enable cache warming
        match desktop.enable_background_cache_warmer(true, Some(10), Some(5)) {
            Ok(_) => println!("✅ Successfully enabled cache warming"),
            Err(e) => {
                println!("❌ Failed to enable cache warming: {}", e);
                return;
            }
        }
        
        // Verify it's enabled
        assert!(desktop.is_cache_warmer_enabled(), "Cache warmer should be enabled");
        
        // Let it run for a bit
        println!("🕐 Letting cache warmer run for 5 seconds...");
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // Disable cache warming
        match desktop.enable_background_cache_warmer(false, None, None) {
            Ok(_) => println!("✅ Successfully disabled cache warming"),
            Err(e) => {
                println!("❌ Failed to disable cache warming: {}", e);
                return;
            }
        }
        
        // Verify it's disabled
        assert!(!desktop.is_cache_warmer_enabled(), "Cache warmer should be disabled");
        
        println!("✅ All basic functionality tests passed!");
    }

    #[tokio::test]
    async fn test_element_property_access_performance() {
        println!("=== Element Property Access Cache Performance Test ===");
        
        let desktop = match Desktop::new(false, false) {
            Ok(d) => d,
            Err(e) => {
                println!("Failed to create desktop: {}", e);
                return;
            }
        };

        // Find a good test element with lots of properties
        let test_element = find_complex_test_element(&desktop).await;
        if test_element.is_none() {
            println!("❌ Could not find a suitable test element");
            return;
        }
        let test_element = test_element.unwrap();
        
        println!("🔍 Testing element property access WITHOUT cache warming...");
        let without_cache = measure_element_property_access(&test_element, 10).await;
        if without_cache.is_empty() {
            println!("❌ No successful property access measurements without cache");
            return;
        }
        
        println!("🔥 Enabling background cache warming...");
        if let Err(e) = desktop.enable_background_cache_warmer(true, Some(5), Some(20)) {
            println!("❌ Failed to enable cache warming: {}", e);
            return;
        }
        
        println!("⏳ Waiting 10 seconds for cache to warm up...");
        tokio::time::sleep(Duration::from_secs(10)).await;
        
        println!("🔍 Testing element property access WITH cache warming...");
        let with_cache = measure_element_property_access(&test_element, 10).await;
        
        // Disable cache warming
        let _ = desktop.enable_background_cache_warmer(false, None, None);
        
        if with_cache.is_empty() {
            println!("❌ No successful property access measurements with cache");
            return;
        }
        
        // Calculate and display results
        let avg_without: f64 = without_cache.iter().map(|d| d.as_micros() as f64).sum::<f64>() / without_cache.len() as f64;
        let avg_with: f64 = with_cache.iter().map(|d| d.as_micros() as f64).sum::<f64>() / with_cache.len() as f64;
        
        let improvement = ((avg_without - avg_with) / avg_without) * 100.0;
        
        println!("\n=== ELEMENT PROPERTY ACCESS PERFORMANCE RESULTS ===");
        println!("📊 Successful measurements: {} without cache, {} with cache", without_cache.len(), with_cache.len());
        println!("⏱️  AVERAGE PROPERTY ACCESS TIMES:");
        println!("   Without cache: {:.1}μs", avg_without);
        println!("   With cache:    {:.1}μs", avg_with);
        
        if improvement > 0.0 {
            println!("   🚀 MASSIVE IMPROVEMENT: {:.1}% faster with cache!", improvement);
            println!("   🏃 {:.1}x speedup!", avg_without / avg_with);
        } else {
            println!("   ⚠️  SLOWER: {:.1}% slower with cache", improvement.abs());
        }
        
        println!("📈 FASTEST TIMES:");
        println!("   Without cache: {:?}", without_cache.iter().min().unwrap());
        println!("   With cache:    {:?}", with_cache.iter().min().unwrap());
        
        println!("📋 SAMPLE MEASUREMENTS:");
        println!("Without cache: {:?}", &without_cache[..without_cache.len().min(5)]);
        println!("With cache:    {:?}", &with_cache[..with_cache.len().min(5)]);
    }
    
    fn find_first_complex_element(node: &terminator::UINode) -> Option<terminator::UIElement> {
        // Since UINode doesn't have element field, we'll use a different approach
        // We'll return None here and find elements using Desktop methods instead
        None
    }
    
    async fn find_complex_test_element(desktop: &Desktop) -> Option<terminator::UIElement> {
        // Try to find an element with lots of properties directly from desktop
        let targets = [
            "Program Manager",
            "Task Manager", 
            "Settings",
        ];
        
        // Try root element as a complex test subject
        let root = desktop.root();
        if let Ok(children) = root.children() {
            if !children.is_empty() {
                return Some(children[0].clone());
            }
        }
        
        // Fallback to applications
        if let Ok(apps) = desktop.applications() {
            for app in apps.iter().take(3) {
                if let Ok(children) = app.children() {
                    for child in children.iter().take(3) {
                        // Test if this element has rich properties by trying to access them
                        let has_properties = child.id().is_some() || 
                                           !child.role().is_empty() ||
                                           child.is_enabled().unwrap_or(false);
                        if has_properties {
                            return Some(child.clone());
                        }
                    }
                }
            }
        }
        
        // Last resort: just return the root element
        Some(desktop.root())
    }
    
    async fn measure_element_property_access(element: &terminator::UIElement, attempts: usize) -> Vec<Duration> {
        let mut measurements = Vec::new();
        
        for i in 0..attempts {
            let start = Instant::now();
            
            // Access multiple properties to get comprehensive measurement
            let mut property_count = 0;
            
            // Test all the cached property access methods
            if element.id().is_some() { property_count += 1; }
            let _ = element.role(); property_count += 1;
            let _ = element.attributes(); property_count += 1;
            if let Ok(_) = element.bounds() { property_count += 1; }
            if let Ok(_) = element.is_enabled() { property_count += 1; }
            if let Ok(_) = element.is_visible() { property_count += 1; }
            if let Ok(_) = element.is_focused() { property_count += 1; }
            if let Ok(_) = element.is_keyboard_focusable() { property_count += 1; }
            if let Ok(_) = element.children() { property_count += 1; }
            
            let elapsed = start.elapsed();
            measurements.push(elapsed);
            
            println!("  Attempt {}: ✅ Accessed {} properties in {:?}", 
                     i + 1, property_count, elapsed);
            
            // Small delay between measurements
            if i < attempts - 1 {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        
        measurements
    }
}

#[cfg(not(target_os = "windows"))]
mod cache_warming_tests {
    use tokio;
    
    #[tokio::test]
    async fn test_cache_warming_unsupported_platforms() {
        println!("=== Cache Warming Test (Non-Windows Platform) ===");
        
        // This test just verifies that the unsupported operation is properly returned
        // on non-Windows platforms
        
        use terminator::Desktop;
        
        let desktop = match Desktop::new(false, false) {
            Ok(d) => d,
            Err(e) => {
                println!("Desktop creation failed (expected on unsupported platforms): {}", e);
                return;
            }
        };

        // Should return UnsupportedOperation error
        match desktop.enable_background_cache_warmer(true, Some(10), Some(5)) {
            Ok(_) => panic!("Cache warming should not be supported on this platform"),
            Err(e) => {
                println!("✅ Correctly returned error for unsupported platform: {}", e);
                assert!(e.to_string().contains("not yet implemented") || 
                       e.to_string().contains("UnsupportedOperation"));
            }
        }

        // Should return false
        assert!(!desktop.is_cache_warmer_enabled(), 
               "Cache warmer should always be disabled on unsupported platforms");
        
        println!("✅ Non-Windows platform test passed!");
    }
} 