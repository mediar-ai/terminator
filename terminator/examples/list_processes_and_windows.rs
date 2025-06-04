use std::collections::HashMap;
use terminator::{Desktop, AutomationError};
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), AutomationError> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    info!("Starting process and window enumeration...");
    
    // Create desktop automation instance
    let desktop = Desktop::new(false, false)?;
    
    // Get all applications
    let applications = desktop.applications()?;
    info!("Found {} applications", applications.len());

    // Group windows by process ID to show the relationship
    let mut process_windows: HashMap<u32, Vec<String>> = HashMap::new();
    let mut process_names: HashMap<u32, String> = HashMap::new();
    
    println!("\n=== Process ID and Window/App Mapping ===");
    println!("{:<8} {:<30} {:<50}", "PID", "Process Name", "Window/App Title");
    println!("{}", "-".repeat(90));
    
    for (index, app) in applications.iter().enumerate() {
        match get_app_info(app, index) {
            Ok((pid, process_name, window_title)) => {
                // Store process name
                process_names.entry(pid).or_insert_with(|| process_name.clone());
                
                // Add window title to this process
                process_windows.entry(pid)
                    .or_insert_with(Vec::new)
                    .push(window_title.clone());
                
                println!("{:<8} {:<30} {:<50}", 
                    pid, 
                    truncate_string(&process_name, 29),
                    truncate_string(&window_title, 49)
                );
            }
            Err(e) => {
                warn!("Failed to get info for application {}: {}", index, e);
            }
        }
    }

    // Show summary of processes with multiple windows
    println!("\n=== Process Summary (Processes with Multiple Windows) ===");
    println!("{:<8} {:<30} {:<10} {}", "PID", "Process Name", "Windows", "Window Titles");
    println!("{}", "-".repeat(100));
    
    for (pid, windows) in process_windows.iter() {
        if windows.len() > 1 {
            let false_process_name = "Unknown".to_string();
            let process_name = process_names.get(pid).unwrap_or(&false_process_name);
            println!("{:<8} {:<30} {:<10} {}", 
                pid, 
                truncate_string(process_name, 29),
                windows.len(),
                windows.join(", ")
            );
        }
    }

    // Demonstrate browser tab detection
    println!("\n=== Browser Tab Detection Example ===");
    detect_browser_tabs(&desktop).await?;

    Ok(())
}

fn get_app_info(app: &terminator::UIElement, index: usize) -> Result<(u32, String, String), AutomationError> {
    // Get process ID
    let pid = app.process_id()?;
    
    // Get process name using the process ID
    #[cfg(target_os = "windows")]
    let process_name = {
        use terminator::platforms::windows::get_process_name_by_pid;
        get_process_name_by_pid(pid as i32)
            .unwrap_or_else(|_| format!("Unknown-{}", pid))
    };
    
    #[cfg(not(target_os = "windows"))]
    let process_name = format!("Process-{}", pid);
    
    // Get window/app title
    let attributes = app.attributes();
    let window_title = attributes.name.unwrap_or_else(|| 
        format!("Unnamed-App-{}", index)
    );
    
    Ok((pid, process_name, window_title))
}

async fn detect_browser_tabs(desktop: &Desktop) -> Result<(), AutomationError> {
    info!("Attempting to detect browser tabs...");
    
    // Try to get the current browser window
    match desktop.get_current_browser_window().await {
        Ok(browser_window) => {
            let pid = browser_window.process_id()?;
            let attrs = browser_window.attributes();
            let title = attrs.name.unwrap_or("Unknown".to_string());
            
            println!("Current browser window:");
            println!("  PID: {}", pid);
            println!("  Title: {}", title);
            println!("  Role: {}", attrs.role);
            
            // Try to find all browser tabs/documents
            info!("Searching for browser tabs...");
            
            // Look for Document control types (common for browser content areas)
            let document_selector = terminator::Selector::Role { 
                role: "document".to_string(), 
                name: None 
            };
            let document_locator = desktop.locator(document_selector);
            
            match document_locator.all(Some(std::time::Duration::from_secs(2)), None).await {
                Ok(documents) => {
                    println!("\nFound {} document elements (potential browser tabs):", documents.len());
                    
                    for (i, doc) in documents.iter().take(5).enumerate() { // Limit to first 5
                        if let Ok(doc_pid) = doc.process_id() {
                            let doc_attrs = doc.attributes();
                            let doc_title = doc_attrs.name.unwrap_or("Untitled".to_string());
                            
                            // Check if this document belongs to a browser process
                            if is_browser_process(doc_pid) {
                                println!("  Tab {}: PID={}, Title={}", 
                                    i + 1, 
                                    doc_pid, 
                                    truncate_string(&doc_title, 50)
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to find document elements: {}", e);
                }
            }
        }
        Err(e) => {
            warn!("No current browser window detected: {}", e);
        }
    }
    
    Ok(())
}

fn is_browser_process(pid: u32) -> bool {
    const KNOWN_BROWSER_PROCESS_NAMES: &[&str] = &[
        "chrome", "firefox", "msedge", "iexplore", "opera", "brave", "vivaldi", "browser", "arc"
    ];
    
    #[cfg(target_os = "windows")]
    {
        use terminator::platforms::windows::get_process_name_by_pid;
        if let Ok(process_name) = get_process_name_by_pid(pid as i32) {
            let process_name_lower = process_name.to_lowercase();
            KNOWN_BROWSER_PROCESS_NAMES.iter().any(|&browser| process_name_lower.contains(browser))
        } else {
            false
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    false
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
} 