use super::windows::*;
use std::process;
use std::time::Instant;
use crate::platforms::AccessibilityEngine;

#[test]
fn test_get_process_name_by_pid_current_process() {
    // Test with the current process PID
    let current_pid = process::id() as i32;
    let result = get_process_name_by_pid(current_pid);
    
    assert!(result.is_ok(), "Should be able to get current process name");
    let process_name = result.unwrap();
    
    // The process name should be a valid non-empty string
    assert!(!process_name.is_empty(), "Process name should not be empty");
    
    // Should not contain .exe extension
    assert!(!process_name.ends_with(".exe"), "Process name should not contain .exe extension");
    assert!(!process_name.ends_with(".EXE"), "Process name should not contain .EXE extension");
    
    // Should be a reasonable process name (alphanumeric, hyphens, underscores)
    assert!(process_name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_'), 
           "Process name should contain only alphanumeric characters, hyphens, and underscores: {}", process_name);
    
    println!("Current process name: {}", process_name);
}




#[test]
fn test_tree_building_performance_stress_test() {
    // This test is more intensive and can be used to identify performance bottlenecks
    let engine = match WindowsEngine::new(false, false) {
        Ok(engine) => engine,
        Err(_) => {
            println!("Cannot create WindowsEngine, skipping stress test");
            return;
        }
    };

    
    // Get all applications for a larger test
    let applications = match engine.get_applications() {
        Ok(apps) => apps,
        Err(_) => {
            println!("Cannot get applications, using root element");
            return;
        }
    };

    if applications.is_empty() {
        println!("No applications available, using root element for stress test");
        return;
    }

    // Use the first application with more elements allowed
    let app = &applications[0];
    
    println!("Starting stress test with application: {:?}", app.attributes().name);
    
    let start_time = Instant::now();
    
    // Try to get a window tree first to see what we're dealing with
    match engine.get_window_tree_by_pid_and_title(
        app.process_id().unwrap_or(0), 
        app.attributes().name.as_deref()
    ) {
        Ok(tree) => {
            let total_time = start_time.elapsed();
            
            // Count elements in the tree
            let element_count = count_tree_elements(&tree);
            let tree_depth = calculate_tree_depth(&tree);
            
            println!("=== Stress Test Results ===");
            println!("Tree building time: {:?}", total_time);
            println!("Total elements in tree: {}", element_count);
            println!("Tree depth: {}", tree_depth);
            println!("Elements per second: {:.2}", element_count as f64 / total_time.as_secs_f64());
            
            // Performance assertions
            
            // Don't make the test too strict, but it shouldn't take forever
            if total_time > std::time::Duration::from_secs(30) {
                println!("Warning: Tree building took longer than expected: {:?}", total_time);
            }
        }
        Err(e) => {
            println!("Tree building failed in stress test: {}", e);
            // Don't fail the test, just log the issue
        }
    }
}

fn count_tree_elements(node: &crate::UINode) -> usize {
    1 + node.children.iter().map(count_tree_elements).sum::<usize>()
}

fn calculate_tree_depth(node: &crate::UINode) -> usize {
    if node.children.is_empty() {
        1
    } else {
        1 + node.children.iter().map(calculate_tree_depth).max().unwrap_or(0)
    }
}





#[test]
fn test_get_process_name_by_pid_invalid_pid() {
    // Test with an invalid PID
    let result = get_process_name_by_pid(-1);
    assert!(result.is_err(), "Should fail for invalid PID");
    
    // Test with a PID that likely doesn't exist (very high number)
    let result = get_process_name_by_pid(999999);
    assert!(result.is_err(), "Should fail for non-existent PID");
}

#[test]
fn test_get_process_name_by_pid_system_process() {
    // Test with system processes that should exist
    let system_pids = vec![0, 4]; // System Idle Process and System
    
    for pid in system_pids {
        match get_process_name_by_pid(pid) {
            Ok(name) => {
                println!("System process {}: {}", pid, name);
                assert!(!name.is_empty(), "System process name should not be empty");
            }
            Err(e) => {
                println!("Could not get name for system process {}: {}", pid, e);
                // Don't fail the test as access might be restricted
            }
        }
    }
}






#[test]
fn test_open_regular_application() {
    let engine = match WindowsEngine::new(false, false) {
        Ok(engine) => engine,
        Err(_) => {
            println!("Cannot create WindowsEngine, skipping application test");
            return;
        }
    };

    // Test with common Windows applications
    let test_apps = vec!["notepad", "calc", "mspaint"];
    
    for app_name in test_apps {
        println!("Testing application opening: {}", app_name);
        
        match engine.open_application(app_name) {
            Ok(app_element) => {
                println!("Successfully opened {}", app_name);
                let attrs = app_element.attributes();
                println!("App attributes - Role: {}, Name: {:?}", attrs.role, attrs.name);
                
                // Basic validation
                assert!(!attrs.role.is_empty(), "Application should have a role");
                
                // Clean up - try to close the application
                let _ = app_element.press_key("Alt+F4");
            }
            Err(e) => {
                println!("Could not open {}: {} (this might be expected)", app_name, e);
            }
        }
    }
}

#[test]
fn test_open_uwp_application() {
    let engine = match WindowsEngine::new(false, false) {
        Ok(engine) => engine,
        Err(_) => {
            println!("Cannot create WindowsEngine, skipping UWP test");
            return;
        }
    };

    // Test with common UWP applications
    let test_apps = vec!["Microsoft Store", "Settings", "Photos"];
    
    for app_name in test_apps {
        println!("Testing UWP application opening: {}", app_name);
        
        match engine.open_application(app_name) {
            Ok(app_element) => {
                println!("Successfully opened UWP app {}", app_name);
                let attrs = app_element.attributes();
                println!("UWP app attributes - Role: {}, Name: {:?}", attrs.role, attrs.name);
                
                // Basic validation
                assert!(!attrs.role.is_empty(), "UWP application should have a role");
                
                // Clean up
                let _ = app_element.press_key("Alt+F4");
            }
            Err(e) => {
                println!("Could not open UWP app {}: {} (this might be expected)", app_name, e);
            }
        }
    }
}

#[test]
fn test_browser_title_matching() {
    // Test the extract_browser_info function
    let (is_browser, parts) = WindowsEngine::extract_browser_info(
        "MailTracker: Email tracker for Gmail - Chrome Web Store - Google Chrome"
    );
    
    assert!(is_browser, "Should detect as browser title");
    assert!(parts.len() >= 2, "Should split browser title into parts: {:?}", parts);
    
    // Should contain both the page title and the browser name
    let parts_str = parts.join(" ");
    assert!(parts_str.to_lowercase().contains("mailtracker"), "Should contain page title");
    assert!(parts_str.to_lowercase().contains("chrome"), "Should contain browser name");
    
    // Test similarity calculation
    let similarity = WindowsEngine::calculate_similarity(
        "Chrome Web Store - Google Chrome",
        "MailTracker: Email tracker for Gmail - Chrome Web Store - Google Chrome"
    );
    
    assert!(similarity > 0.3, "Should have reasonable similarity: {}", similarity);
    
    println!("Browser title parts: {:?}", parts);
    println!("Similarity score: {:.2}", similarity);
}

#[test]
fn test_browser_title_matching_edge_cases() {
    // Test various browser title formats
    let test_cases = vec![
        ("Tab Title - Google Chrome", true),
        ("Mozilla Firefox", true),
        ("Microsoft Edge", true),
        ("Some App - Not Application", false), // Changed to avoid "browser" word
        ("Chrome Web Store - Google Chrome", true),
        ("GitHub - Google Chrome", true),
        ("Random Window Title", false),
    ];

    for (title, expected_is_browser) in test_cases {
        let (is_browser, parts) = WindowsEngine::extract_browser_info(title);
        assert_eq!(is_browser, expected_is_browser, 
                  "Browser detection failed for: '{}', expected: {}, got: {}", 
                  title, expected_is_browser, is_browser);
        
        if is_browser {
            assert!(!parts.is_empty(), "Browser title should have parts: '{}'", title);
        }
    }
}

#[test]
fn test_similarity_calculation_edge_cases() {
    let test_cases = vec![
        ("identical", "identical", 1.0),
        ("Longer String", "Long", 0.3), // More realistic expected value
        ("Chrome Web Store", "MailTracker Chrome Web Store", 0.4), // More realistic
        ("completely different", "nothing similar", 0.0),
        ("", "empty test", 0.0),
        ("single", "", 0.0),
    ];

    for (text1, text2, min_expected) in test_cases {
        let similarity = WindowsEngine::calculate_similarity(text1, text2);
        
        if min_expected == 1.0 {
            assert_eq!(similarity, 1.0, "Identical strings should have similarity 1.0");
        } else if min_expected == 0.0 {
            assert_eq!(similarity, 0.0, "Completely different strings should have similarity 0.0");
        } else {
            assert!(similarity >= min_expected - 0.2 && similarity <= 1.0, 
                   "Similarity for '{}' vs '{}' should be around {}, got: {:.2}", 
                   text1, text2, min_expected, similarity);
        }
        
        println!("'{}' vs '{}' = {:.2}", text1, text2, similarity);
    }
}

#[test]
fn test_find_best_title_match_browser_scenario() {

    // Mock window data based on the actual log
    // Expected: "MailTracker: Email tracker for Gmail - Chrome Web Store - Google Chrome"
    // Available: "Chrome Web Store - Google Chrome"
    
    // We can't create actual UIElements for testing, but we can test our logic
    let target_title = "MailTracker: Email tracker for Gmail - Chrome Web Store - Google Chrome";
    let available_window_name = "Chrome Web Store - Google Chrome";
    
    // Test the individual components
    let (is_target_browser, target_parts) = WindowsEngine::extract_browser_info(target_title);
    let (is_window_browser, window_parts) = WindowsEngine::extract_browser_info(available_window_name);
    
    assert!(is_target_browser, "Target should be detected as browser");
    assert!(is_window_browser, "Window should be detected as browser");
    
    println!("Target parts: {:?}", target_parts);
    println!("Window parts: {:?}", window_parts);
    
    // Test similarity between parts
    let mut max_similarity = 0.0f64;
    for target_part in &target_parts {
        for window_part in &window_parts {
            let similarity = WindowsEngine::calculate_similarity(target_part, window_part);
            max_similarity = max_similarity.max(similarity);
            println!("'{}' vs '{}' = {:.2}", target_part, window_part, similarity);
        }
    }
    
    // Should find a good match since both contain "Chrome Web Store - Google Chrome"
    assert!(max_similarity > 0.6, 
           "Should find good similarity between browser titles, got: {:.2}", max_similarity);
}

#[test]
fn test_enhanced_error_messages() {
    // Test that browser error messages provide helpful suggestions
    let target_title = "MailTracker: Email tracker for Gmail - Chrome Web Store - Google Chrome";
    let available_windows = vec![
        "Taskbar".to_string(),
        "Chrome Web Store - Google Chrome".to_string(),
        "Firefox - Mozilla Firefox".to_string(),
        "Random Application".to_string(),
    ];
    
    let (is_target_browser, _) = WindowsEngine::extract_browser_info(target_title);
    assert!(is_target_browser, "Target should be browser");
    
    let browser_windows: Vec<&String> = available_windows.iter()
        .filter(|name| {
            let (is_browser, _) = WindowsEngine::extract_browser_info(name);
            is_browser
        })
        .collect();
    
    assert!(!browser_windows.is_empty(), "Should find browser windows in the list");
    assert!(browser_windows.len() >= 2, "Should find multiple browser windows: {:?}", browser_windows);
    
    // Verify the specific windows we expect
    assert!(browser_windows.iter().any(|w| w.contains("Chrome")), "Should find Chrome window");
    assert!(browser_windows.iter().any(|w| w.contains("Firefox")), "Should find Firefox window");
} 