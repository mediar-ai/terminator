// Test if Win32 ShowWindow works for Calculator (UWP app)
use terminator::{Desktop, maximize_window_by_pid};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing Win32 ShowWindow maximize for UWP...\n");

    let desktop = Desktop::new(false, true)?;
    println!("✅ Desktop initialized");

    // Open Calculator
    println!("📱 Opening Calculator...");
    let ui_element = desktop.open_application("Calculator")?;
    println!("✅ Calculator opened");

    let pid = ui_element.process_id().unwrap_or(0);
    println!("   PID: {}", pid);
    println!("   Title: {}", ui_element.window_title());

    // Get initial bounds
    if let Ok((x, y, width, height)) = ui_element.bounds() {
        println!("\n📏 Initial bounds:");
        println!("   x: {}, y: {}, width: {}, height: {}", x, y, width, height);
    }

    // Try Win32 ShowWindow maximize
    println!("\n🔄 Attempting to maximize via Win32 ShowWindow...");
    let success = maximize_window_by_pid(pid);
    
    if success {
        println!("✅ maximize_window_by_pid() returned true");
    } else {
        println!("❌ maximize_window_by_pid() returned false");
    }

    // Wait for window to update
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Get final bounds
    if let Ok((x, y, width, height)) = ui_element.bounds() {
        println!("\n📏 Final bounds:");
        println!("   x: {}, y: {}, width: {}, height: {}", x, y, width, height);
    }

    println!("\n✨ Test complete! Check if Calculator is actually maximized.");

    Ok(())
}
