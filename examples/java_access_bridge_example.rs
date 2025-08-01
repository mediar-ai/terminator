//! Java Access Bridge Example
//! 
//! This example demonstrates how to use the Java Access Bridge functionality
//! to automate Java applications on Windows.
//! 
//! Prerequisites:
//! - Windows operating system
//! - Java Access Bridge installed (comes with Java runtime)
//! - A Java application running (e.g., a Swing or AWT application)
//! 
//! To enable Java Access Bridge:
//! 1. Run `jabswitch -enable` as administrator
//! 2. Restart your Java applications
//! 
//! Usage:
//! ```
//! cargo run --example java_access_bridge_example
//! ```

use terminator::{Desktop, Locator, Selector};
use std::error::Error;
use std::time::Duration;

fn main() -> Result<(), Box<dyn Error>> {
    // Initialize the desktop automation
    let desktop = Desktop::new()?;
    
    println!("🔧 Java Access Bridge Example");
    println!("===============================");
    
    // Check if Java Access Bridge is available
    println!("🔍 Checking Java Access Bridge availability...");
    
    // Try to find Java applications
    println!("🔍 Looking for Java applications...");
    
    // Method 1: Find applications using the JavaApp selector
    match desktop.locator(Selector::from("javaapp")).find_element(Some(Duration::from_secs(5))) {
        Ok(java_app) => {
            println!("✅ Found Java application!");
            demonstrate_java_automation(&java_app)?;
        }
        Err(e) => {
            println!("ℹ️ No Java applications found: {}", e);
            println!("   Make sure:");
            println!("   1. Java Access Bridge is enabled (run 'jabswitch -enable' as admin)");
            println!("   2. A Java application (Swing/AWT) is running");
            println!("   3. The Java application has accessibility enabled");
        }
    }
    
    // Method 2: Find specific Java application by name
    println!("\n🔍 Looking for specific Java applications...");
    
    let java_app_names = [
        "Java",
        "Swing",
        "AWT",
        "NetBeans",
        "Eclipse",
        "IntelliJ",
        "JConsole",
    ];
    
    for app_name in &java_app_names {
        let selector = format!("javaapp:{}", app_name);
        match desktop.locator(Selector::from(selector.as_str())).find_element(Some(Duration::from_secs(2))) {
            Ok(java_app) => {
                println!("✅ Found Java application: {}", app_name);
                demonstrate_java_automation(&java_app)?;
                break;
            }
            Err(_) => {
                println!("   - {} not found", app_name);
            }
        }
    }
    
    // Method 3: Find Java applications by window title patterns
    println!("\n🔍 Looking for Java applications by window patterns...");
    
    let java_patterns = [
        "role:window >> javaapp",
        "window >> javaapp",
    ];
    
    for pattern in &java_patterns {
        match desktop.locator(Selector::from(*pattern)).find_element(Some(Duration::from_secs(2))) {
            Ok(java_app) => {
                println!("✅ Found Java application using pattern: {}", pattern);
                demonstrate_java_automation(&java_app)?;
                break;
            }
            Err(_) => {
                println!("   - Pattern '{}' didn't match", pattern);
            }
        }
    }
    
    println!("\n📚 Java Access Bridge Usage Tips:");
    println!("================================");
    println!("1. Use 'javaapp' selector to find any Java application");
    println!("2. Use 'javaapp:AppName' to find specific Java apps");
    println!("3. Chain selectors: 'javaapp >> role:button >> name:OK'");
    println!("4. Java elements support all standard operations (click, type, etc.)");
    println!("5. Use Accessibility Inspector tools to explore Java UI structure");
    
    Ok(())
}

fn demonstrate_java_automation(java_app: &terminator::UIElement) -> Result<(), Box<dyn Error>> {
    println!("\n🎮 Demonstrating Java Application Automation");
    println!("============================================");
    
    // Get basic information about the Java application
    match java_app.get_name() {
        Ok(name) => println!("📱 Application Name: {}", name),
        Err(_) => println!("📱 Application Name: (could not retrieve)"),
    }
    
    match java_app.get_role() {
        Ok(role) => println!("🎭 Application Role: {}", role),
        Err(_) => println!("🎭 Application Role: (could not retrieve)"),
    }
    
    match java_app.get_bounding_rectangle() {
        Ok((x, y, width, height)) => {
            println!("📐 Application Bounds: ({}, {}) {}x{}", x, y, width, height);
        }
        Err(_) => println!("📐 Application Bounds: (could not retrieve)"),
    }
    
    // Try to find common Java UI elements
    println!("\n🔍 Exploring Java UI Elements:");
    
    let common_elements = [
        ("Buttons", "role:button"),
        ("Text Fields", "role:text"),
        ("Labels", "role:label"),
        ("Menus", "role:menu"),
        ("Menu Items", "role:menuitem"),
        ("Panels", "role:panel"),
        ("Lists", "role:list"),
        ("Trees", "role:tree"),
        ("Tables", "role:table"),
    ];
    
    for (element_type, selector) in &common_elements {
        let locator = terminator::Locator::new(java_app, Selector::from(*selector));
        match locator.find_elements(Some(Duration::from_secs(1)), None) {
            Ok(elements) => {
                if !elements.is_empty() {
                    println!("  ✅ Found {} {}", elements.len(), element_type);
                    
                    // Show details for the first few elements
                    for (i, element) in elements.iter().take(3).enumerate() {
                        if let Ok(name) = element.get_name() {
                            if !name.is_empty() {
                                println!("     {}. {}", i + 1, name);
                            }
                        }
                    }
                    
                    if elements.len() > 3 {
                        println!("     ... and {} more", elements.len() - 3);
                    }
                }
            }
            Err(_) => {
                println!("  - No {} found", element_type);
            }
        }
    }
    
    // Try to interact with buttons
    println!("\n🖱️ Attempting to interact with Java elements:");
    
    let button_locator = terminator::Locator::new(java_app, Selector::from("role:button"));
    match button_locator.find_element(Some(Duration::from_secs(2))) {
        Ok(button) => {
            if let Ok(button_name) = button.get_name() {
                println!("🔘 Found button: '{}'", button_name);
                
                // Check if the button is enabled
                match button.is_enabled() {
                    Ok(true) => {
                        println!("   Button is enabled - it's safe to click");
                        // Uncomment the next line to actually click the button
                        // button.click()?;
                        // println!("   ✅ Clicked the button!");
                    }
                    Ok(false) => {
                        println!("   Button is disabled - cannot click");
                    }
                    Err(_) => {
                        println!("   Could not determine button state");
                    }
                }
            }
        }
        Err(_) => {
            println!("🔘 No clickable buttons found");
        }
    }
    
    // Try to find text fields
    let text_field_locator = terminator::Locator::new(java_app, Selector::from("role:text"));
    match text_field_locator.find_element(Some(Duration::from_secs(2))) {
        Ok(text_field) => {
            if let Ok(field_name) = text_field.get_name() {
                println!("📝 Found text field: '{}'", field_name);
                
                // Check if we can type in it
                match text_field.is_enabled() {
                    Ok(true) => {
                        println!("   Text field is enabled - it's safe to type");
                        // Uncomment the next lines to actually type text
                        // text_field.clear_text()?;
                        // text_field.type_text("Hello from Terminator!")?;
                        // println!("   ✅ Typed text into the field!");
                    }
                    Ok(false) => {
                        println!("   Text field is disabled - cannot type");
                    }
                    Err(_) => {
                        println!("   Could not determine text field state");
                    }
                }
            }
        }
        Err(_) => {
            println!("📝 No text fields found");
        }
    }
    
    println!("\n🎯 Java automation demonstration completed!");
    println!("   To enable actual interactions, uncomment the action lines in the code.");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_java_access_bridge_example_logic() {
        // Test that the example logic is sound
        // This doesn't require actual Java applications to be running
        
        // Test selector creation
        let java_app_selector = Selector::from("javaapp");
        match java_app_selector {
            Selector::JavaApp(None) => (),
            _ => panic!("Expected JavaApp selector"),
        }
        
        let specific_java_app_selector = Selector::from("javaapp:Eclipse");
        match specific_java_app_selector {
            Selector::JavaApp(Some(name)) if name == "Eclipse" => (),
            _ => panic!("Expected specific JavaApp selector"),
        }
        
        println!("✅ Java Access Bridge example logic tests passed");
    }
}