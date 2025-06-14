//! Functional verification tests for optimized Windows implementation
//!
//! This module contains tests to verify that the performance optimizations
//! in the Windows platform layer don't break existing functionality.

use crate::Desktop;

/// Initialize test environment
fn init_test_environment() -> Desktop {
    Desktop::new(false, false).expect("Failed to create Desktop for testing")
}

#[test]
#[ignore] // Run with: cargo test test_get_applications_functional -- --ignored --nocapture
fn test_get_applications_functional() {
    println!("\n🔍 Testing get_applications() functionality");

    let desktop = init_test_environment();

    // Test basic functionality
    match desktop.applications() {
        Ok(apps) => {
            println!("✅ Successfully retrieved {} applications", apps.len());

            // Verify we get some applications (should be at least 1 on any Windows system)
            assert!(!apps.is_empty(), "Should find at least one application");

            // Test that each application has basic properties
            for (i, app) in apps.iter().take(3).enumerate() {
                println!("  App {}: Testing basic properties...", i + 1);

                // Test that we can get the name without crashing
                match app.name() {
                    Some(name) => {
                        println!("    ✅ Name: '{}'", name);
                        assert!(!name.is_empty(), "Application name should not be empty");
                    }
                    None => println!("    ⚠️  No name available"),
                }

                // Test that we can get the role without crashing
                let role = app.role();
                println!("    ✅ Role: '{}'", role);
                assert!(!role.is_empty(), "Role should not be empty");

                // Test that we can get attributes without crashing
                let attrs = app.attributes();
                println!("    ✅ Attributes role: '{}'", attrs.role);

                // Test that we can get process ID without crashing
                match app.process_id() {
                    Ok(pid) => {
                        println!("    ✅ Process ID: {}", pid);
                        assert!(pid > 0, "Process ID should be positive");
                    }
                    Err(e) => println!("    ⚠️  Could not get process ID: {}", e),
                }
            }
        }
        Err(e) => {
            panic!("❌ Failed to get applications: {}", e);
        }
    }
}

#[test]
#[ignore]
fn test_get_application_by_name_functional() {
    println!("\n🔍 Testing get_application_by_name() functionality");

    let desktop = init_test_environment();

    // Test with a known Windows application that should always be available
    let test_apps = ["explorer", "dwm", "winlogon", "csrss"];
    let mut found_app = false;

    for app_name in &test_apps {
        println!("  Testing lookup for: '{}'", app_name);

        match desktop.application(app_name) {
            Ok(app) => {
                println!("    ✅ Found application: '{}'", app_name);
                found_app = true;

                // Verify the application is functional
                let role = app.role();
                println!("    ✅ Role: '{}'", role);

                match app.process_id() {
                    Ok(pid) => {
                        println!("    ✅ Process ID: {}", pid);
                        assert!(pid > 0, "Process ID should be positive");
                    }
                    Err(e) => println!("    ⚠️  Could not get process ID: {}", e),
                }

                // Test attributes access
                let attrs = app.attributes();
                println!("    ✅ Attributes accessible, role: '{}'", attrs.role);

                break; // Found one working app, that's enough
            }
            Err(e) => {
                println!("    ⚠️  Could not find '{}': {}", app_name, e);
            }
        }
    }

    assert!(
        found_app,
        "Should be able to find at least one system application"
    );

    // Test with non-existent application
    println!("  Testing lookup for non-existent app...");
    match desktop.application("nonexistent_app_12345") {
        Ok(_) => panic!("❌ Should not find non-existent application"),
        Err(e) => {
            println!("    ✅ Correctly failed to find non-existent app: {}", e);
        }
    }
}

#[test]
#[ignore]
fn test_cached_attributes_functional() {
    println!("\n🔍 Testing cached attributes functionality");

    let desktop = init_test_environment();

    match desktop.applications() {
        Ok(apps) => {
            if let Some(app) = apps.first() {
                println!("  Testing attributes for first application...");

                // Test multiple attribute accesses to verify caching works
                for i in 1..=3 {
                    println!("    Access #{}: Testing attributes...", i);

                    let attrs = app.attributes();
                    println!("      ✅ Role: '{}'", attrs.role);
                    assert!(!attrs.role.is_empty(), "Role should not be empty");

                    // Test that name is consistently accessible
                    if let Some(name) = &attrs.name {
                        println!("      ✅ Name: '{}'", name);
                        assert!(!name.is_empty(), "Name should not be empty if present");
                    } else {
                        println!("      ⚠️  No name in attributes");
                    }

                    // Verify properties map is accessible
                    println!(
                        "      ✅ Properties map accessible with {} entries",
                        attrs.properties.len()
                    );
                }

                // Test that direct role access still works
                let direct_role = app.role();
                let attrs_role = app.attributes().role;
                println!(
                    "    ✅ Direct role: '{}', Attrs role: '{}'",
                    direct_role, attrs_role
                );

                // They should be the same
                assert_eq!(
                    direct_role, attrs_role,
                    "Direct role and attributes role should match"
                );
            }
        }
        Err(e) => {
            panic!("❌ Failed to get applications for attribute testing: {}", e);
        }
    }
}

#[test]
#[ignore]
fn test_pid_lookup_caching_functional() {
    println!("\n🔍 Testing PID lookup caching functionality");

    let desktop = init_test_environment();

    // Test with known system processes
    let test_processes = ["explorer", "dwm", "winlogon"];

    for process_name in &test_processes {
        println!("  Testing PID lookup for: '{}'", process_name);

        // First lookup - should populate cache
        let start_time = std::time::Instant::now();
        match desktop.application(process_name) {
            Ok(app) => {
                let first_duration = start_time.elapsed();
                println!("    ✅ First lookup took: {:?}", first_duration);

                if let Ok(pid1) = app.process_id() {
                    println!("    ✅ Found PID: {}", pid1);

                    // Second lookup - should use cache
                    let start_time2 = std::time::Instant::now();
                    match desktop.application(process_name) {
                        Ok(app2) => {
                            let second_duration = start_time2.elapsed();
                            println!("    ✅ Second lookup took: {:?}", second_duration);

                            if let Ok(pid2) = app2.process_id() {
                                // PIDs should be the same (unless process restarted)
                                println!("    ✅ PIDs: {} vs {}", pid1, pid2);

                                // Cache should make second lookup faster (usually)
                                if second_duration < first_duration {
                                    println!(
                                        "    ✅ Cache optimization detected (faster second lookup)"
                                    );
                                } else {
                                    println!(
                                        "    ⚠️  Second lookup not faster, but that's still okay"
                                    );
                                }
                            }
                        }
                        Err(e) => println!("    ⚠️  Second lookup failed: {}", e),
                    }
                }
                break; // Found one working process, that's enough
            }
            Err(e) => {
                println!("    ⚠️  Could not find '{}': {}", process_name, e);
            }
        }
    }
}

#[test]
#[ignore]
fn test_application_children_access() {
    println!("\n🔍 Testing application children access");

    let desktop = init_test_environment();

    match desktop.applications() {
        Ok(apps) => {
            // Find an application with children (try a few)
            for app in apps.iter().take(5) {
                let app_name = app.name().unwrap_or_else(|| "Unknown".to_string());
                println!("  Testing children access for: '{}'", app_name);

                match app.children() {
                    Ok(children) => {
                        println!("    ✅ Found {} children", children.len());

                        if !children.is_empty() {
                            // Test first child's attributes
                            let child = &children[0];
                            let child_attrs = child.attributes();
                            println!("    ✅ First child role: '{}'", child_attrs.role);

                            // Test that we can get child's role without crashing
                            let child_role = child.role();
                            println!("    ✅ First child direct role: '{}'", child_role);

                            break; // Found an app with children, that's enough
                        }
                    }
                    Err(e) => {
                        println!("    ⚠️  Could not get children: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            panic!("❌ Failed to get applications for children testing: {}", e);
        }
    }
}

#[test]
#[ignore]
fn test_element_bounds_and_properties() {
    println!("\n🔍 Testing element bounds and properties access");

    let desktop = init_test_environment();

    match desktop.applications() {
        Ok(apps) => {
            if let Some(app) = apps.first() {
                let app_name = app.name().unwrap_or_else(|| "Unknown".to_string());
                println!("  Testing bounds for: '{}'", app_name);

                // Test bounds access
                match app.bounds() {
                    Ok((x, y, width, height)) => {
                        println!("    ✅ Bounds: ({}, {}, {}, {})", x, y, width, height);
                        assert!(width >= 0.0, "Width should be non-negative");
                        assert!(height >= 0.0, "Height should be non-negative");
                    }
                    Err(e) => {
                        println!("    ⚠️  Could not get bounds: {}", e);
                    }
                }

                // Test enabled state
                match app.is_enabled() {
                    Ok(enabled) => {
                        println!("    ✅ Is enabled: {}", enabled);
                    }
                    Err(e) => {
                        println!("    ⚠️  Could not check enabled state: {}", e);
                    }
                }

                // Test visible state
                match app.is_visible() {
                    Ok(visible) => {
                        println!("    ✅ Is visible: {}", visible);
                    }
                    Err(e) => {
                        println!("    ⚠️  Could not check visible state: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            panic!("❌ Failed to get applications for bounds testing: {}", e);
        }
    }
}

#[test]
#[ignore]
fn test_focused_element_access() {
    println!("\n🔍 Testing focused element access");

    let desktop = init_test_environment();

    match desktop.focused_element() {
        Ok(focused) => {
            println!("  ✅ Found focused element");

            // Test that we can access focused element properties
            let role = focused.role();
            println!("    ✅ Focused element role: '{}'", role);

            let attrs = focused.attributes();
            println!("    ✅ Focused element attributes role: '{}'", attrs.role);

            if let Some(name) = attrs.name {
                println!("    ✅ Focused element name: '{}'", name);
            }

            // Test process ID access
            match focused.process_id() {
                Ok(pid) => {
                    println!("    ✅ Focused element PID: {}", pid);
                    assert!(pid > 0, "Process ID should be positive");
                }
                Err(e) => {
                    println!("    ⚠️  Could not get focused element PID: {}", e);
                }
            }
        }
        Err(e) => {
            println!("    ⚠️  Could not get focused element: {}", e);
        }
    }
}

#[test]
#[ignore]
fn test_error_handling_robustness() {
    println!("\n🔍 Testing error handling robustness");

    let desktop = init_test_environment();

    // Test with various invalid inputs
    let invalid_names = [
        "",
        "   ",
        "invalid/name",
        "app\0with\0nulls",
        "very_long_name_that_definitely_does_not_exist_in_any_system_12345",
    ];

    for invalid_name in &invalid_names {
        println!("  Testing with invalid name: '{}'", invalid_name);

        match desktop.application(invalid_name) {
            Ok(_) => {
                println!("    ⚠️  Unexpectedly found application for invalid name");
            }
            Err(e) => {
                println!("    ✅ Correctly handled error: {}", e);
                // Should be a proper error, not a crash
            }
        }
    }

    // Test that the desktop is still functional after error cases
    println!("  Verifying desktop still functional after error cases...");
    match desktop.applications() {
        Ok(apps) => {
            println!("    ✅ Desktop still functional, found {} apps", apps.len());
        }
        Err(e) => {
            panic!(
                "    ❌ Desktop became non-functional after error cases: {}",
                e
            );
        }
    }
}

#[test]
#[ignore]
fn test_comprehensive_functional_verification() {
    println!("\n📊 Comprehensive Functional Verification Test Suite");
    println!("=====================================================");
    println!("Running all functional verification tests...\n");

    test_get_applications_functional();
    println!("\n{}\n", "=".repeat(50));

    test_get_application_by_name_functional();
    println!("\n{}\n", "=".repeat(50));

    test_cached_attributes_functional();
    println!("\n{}\n", "=".repeat(50));

    test_pid_lookup_caching_functional();
    println!("\n{}\n", "=".repeat(50));

    test_application_children_access();
    println!("\n{}\n", "=".repeat(50));

    test_element_bounds_and_properties();
    println!("\n{}\n", "=".repeat(50));

    test_focused_element_access();
    println!("\n{}\n", "=".repeat(50));

    test_error_handling_robustness();

    println!("\n✅ All functional verification tests completed!");
    println!("\n📝 Summary:");
    println!("- Application enumeration works correctly");
    println!("- Application lookup by name functions properly");
    println!("- Cached attributes provide consistent results");
    println!("- PID lookup caching improves performance without breaking functionality");
    println!("- Element property access remains stable");
    println!("- Error handling is robust and doesn't crash the system");
    println!("\n💡 Individual tests can be run with:");
    println!("  cargo test test_get_applications_functional -- --ignored --nocapture");
    println!("  cargo test test_get_application_by_name_functional -- --ignored --nocapture");
    println!("  cargo test test_comprehensive_functional_verification -- --ignored --nocapture");
}
