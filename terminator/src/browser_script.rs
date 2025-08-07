//! Cross-platform browser script execution using terminator SDK
//!
//! Uses terminator SDK selectors for cross-platform browser automation.
//! Finds console tab and prompt using proper selectors, runs JavaScript, extracts results.

use crate::{AutomationError, Desktop};
use std::time::Duration;
use tracing::info;

/// Execute JavaScript in browser using terminator SDK selectors
pub async fn execute_script(
    browser_element: &crate::UIElement,
    script: &str,
) -> Result<String, AutomationError> {
    info!("🚀 Executing JavaScript using terminator SDK: {}", script);

    // Step 1: Focus the browser window
    info!("🎯 Focusing browser window");
    browser_element.focus()?;
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Step 2: Open dev tools if not already open (Ctrl+Shift+J)
    info!("⚙️ Opening dev tools (Ctrl+Shift+J)");
    browser_element.press_key("{Ctrl}{Shift}J")?;
    tokio::time::sleep(Duration::from_millis(2000)).await;

    // Step 4: Clear console using Ctrl + L
    // info!("🧹 Clearing console using Ctrl + L");
    // browser_element.press_key("{Ctrl}L")?;
    // tokio::time::sleep(Duration::from_millis(500)).await;

    let desktop = Desktop::new(true, false)?;

    // Step 5: Find console prompt using terminator selector
    info!("🔍 Finding console prompt using name:Console prompt");
    let console_prompt = desktop
        .locator("role:document|name:DevTools >> name:Console prompt")
        .first(None)
        .await?;

    info!("⌨️ Typing JavaScript into console prompt");
    console_prompt.type_text(script, true)?;
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Step 6: Execute the script (Enter)
    info!("🚀 Executing script with Enter");
    console_prompt.press_key("{ENTER}")?;
    tokio::time::sleep(Duration::from_millis(2000)).await;

    // Step 7: Get result from console messages area using improved approach
    info!("📄 Getting result from console messages");
    let result = match get_console_result_from_ui(&desktop).await {
        Ok(text) => {
            info!(
                "✅ Found console result from UI tree: {}",
                &text[..text.len().min(100)]
            );
            text
        }
        Err(_) => {
            info!("⚠️ Couldn't find console result in UI, trying clipboard approach");
            get_console_result_via_clipboard(&console_prompt).await?
        }
    };

    // Step 8: Close dev tools
    info!("🚪 Closing dev tools");
    browser_element.press_key("{F12}")?;

    info!("✅ Script execution completed: {}", result);
    Ok(result)
}

/// Primary method: get console result from UI tree
async fn get_console_result_from_ui(desktop: &Desktop) -> Result<String, AutomationError> {
    info!("🔍 Getting console result from UI tree");

    // Wait a bit for the result to appear
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // Try to get the console result text directly first
    if let Ok(text_elements) = desktop
        .locator("role:document|name:DevTools >> role:text")
        .all(None, None)
        .await
    {
        for element in text_elements.iter().rev().take(10) {
            if let Some(text) = element.name() {
                let trimmed = text.trim();
                // Look for text elements that look like console output (quoted strings with content)
                if (trimmed.starts_with("'") && trimmed.ends_with("'") && trimmed.len() > 10)
                    || (trimmed.starts_with("\"") && trimmed.ends_with("\"") && trimmed.len() > 10)
                {
                    info!("🎯 Found console result in text elements");
                    // Remove the surrounding quotes
                    let cleaned = trimmed
                        .trim_start_matches(['\'', '"'])
                        .trim_end_matches(['\'', '"']);
                    return Ok(cleaned.to_string());
                }
            }
        }
    }

    // Fallback: Get all group elements in the console area
    let console_groups = desktop
        .locator("role:document|name:DevTools >> role:group")
        .all(None, None)
        .await?;

    if console_groups.is_empty() {
        return Err(AutomationError::ElementNotFound(
            "No console groups found".to_string(),
        ));
    }

    info!(
        "🔍 Found {} console groups, analyzing...",
        console_groups.len()
    );

    // Look for the console result - it's typically in one of the last few groups
    // We look backward from the end to find the first group with meaningful content
    for (i, group) in console_groups.iter().rev().take(8).enumerate() {
        if let Some(text) = group.name() {
            let trimmed = text.trim();
            info!(
                "🔍 Group -{}: {} chars: {}",
                i + 1,
                trimmed.len(),
                &trimmed[..trimmed.len().min(50)]
            );

            // Skip empty, whitespace-only, or very short text
            if !trimmed.is_empty() && trimmed.len() > 10 {
                // Skip groups that are just navigation elements or the command itself
                if !trimmed.contains("messages in console")
                    && !trimmed.contains("Clear console")
                    && !trimmed.contains("Filter")
                    && !trimmed.contains("document.getElementById")  // Skip the command line
                    && !trimmed.contains("function")  // Skip function definitions
                    && !trimmed.contains("undefined")  // Skip undefined results
                    && trimmed.len() > 20
                // Must have substantial content
                {
                    info!("🎯 Found console result in UI tree");
                    return Ok(trimmed.to_string());
                }
            }
        }
    }

    Err(AutomationError::ElementNotFound(
        "No meaningful console result found in UI tree".to_string(),
    ))
}

/// Fallback method: get console result via clipboard
async fn get_console_result_via_clipboard(
    console_prompt: &crate::UIElement,
) -> Result<String, AutomationError> {
    info!("📋 Getting console result via clipboard");

    // Clear clipboard first
    let _ = set_clipboard_content("").await;

    // Navigate to the last console output using multiple down arrows instead of Ctrl+End
    info!("🏃 Moving to bottom of console using arrow keys");
    for _ in 0..10 {
        console_prompt.press_key("{DOWN}")?;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Go up TWO lines to get the result (skip the empty line and get the actual result)
    console_prompt.press_key("{UP}")?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    console_prompt.press_key("{UP}")?;
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Select the line and copy
    console_prompt.press_key("{Ctrl}a")?;
    tokio::time::sleep(Duration::from_millis(200)).await;

    console_prompt.press_key("{Ctrl}c")?;
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Get clipboard content
    get_clipboard_content().await
}

/// Set clipboard content (cross-platform)
async fn set_clipboard_content(content: &str) -> Result<(), AutomationError> {
    use std::process::Command;

    #[cfg(target_os = "windows")]
    {
        let output = Command::new("powershell")
            .args(["-command", &format!("Set-Clipboard '{content}'")])
            .output()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to set clipboard: {e}")))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(AutomationError::PlatformError(format!(
                "PowerShell set clipboard error: {error}"
            )));
        }
    }

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("pbcopy").arg(content).output().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to set clipboard: {}", e))
        })?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(AutomationError::PlatformError(format!(
                "pbcopy error: {}",
                error
            )));
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Try xclip first, then xsel as fallback
        let mut success = false;

        if let Ok(output) = Command::new("xclip")
            .args(&["-selection", "clipboard"])
            .arg(content)
            .output()
        {
            if output.status.success() {
                success = true;
            }
        }

        if !success {
            let output = Command::new("xsel")
                .args(&["--clipboard", "--input"])
                .arg(content)
                .output()
                .map_err(|e| {
                    AutomationError::PlatformError(format!(
                        "Failed to set clipboard (xclip/xsel not available): {}",
                        e
                    ))
                })?;

            if !output.status.success() {
                let error = String::from_utf8_lossy(&output.stderr);
                return Err(AutomationError::PlatformError(format!(
                    "xsel error: {}",
                    error
                )));
            }
        }
    }

    Ok(())
}

/// Get content from clipboard (cross-platform)
async fn get_clipboard_content() -> Result<String, AutomationError> {
    use std::process::Command;

    #[cfg(target_os = "windows")]
    {
        let output = Command::new("powershell")
            .args(["-command", "Get-Clipboard"])
            .output()
            .map_err(|e| {
                AutomationError::PlatformError(format!("Failed to run PowerShell: {e}"))
            })?;

        if output.status.success() {
            let content = String::from_utf8_lossy(&output.stdout).trim().to_string();
            info!("📋 Clipboard content: {}", content);
            Ok(content)
        } else {
            let error = String::from_utf8_lossy(&output.stderr);
            Err(AutomationError::PlatformError(format!(
                "PowerShell error: {error}"
            )))
        }
    }

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("pbpaste")
            .output()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to run pbpaste: {}", e)))?;

        if output.status.success() {
            let content = String::from_utf8_lossy(&output.stdout).trim().to_string();
            info!("📋 Clipboard content: {}", content);
            Ok(content)
        } else {
            let error = String::from_utf8_lossy(&output.stderr);
            Err(AutomationError::PlatformError(format!(
                "pbpaste error: {}",
                error
            )))
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Try xclip first, then xsel as fallback
        if let Ok(output) = Command::new("xclip")
            .args(&["-selection", "clipboard", "-o"])
            .output()
        {
            if output.status.success() {
                let content = String::from_utf8_lossy(&output.stdout).trim().to_string();
                info!("📋 Clipboard content (xclip): {}", content);
                return Ok(content);
            }
        }

        let output = Command::new("xsel")
            .args(&["--clipboard", "--output"])
            .output()
            .map_err(|e| {
                AutomationError::PlatformError(format!(
                    "Failed to get clipboard (xclip/xsel not available): {}",
                    e
                ))
            })?;

        if output.status.success() {
            let content = String::from_utf8_lossy(&output.stdout).trim().to_string();
            info!("📋 Clipboard content (xsel): {}", content);
            Ok(content)
        } else {
            let error = String::from_utf8_lossy(&output.stderr);
            Err(AutomationError::PlatformError(format!(
                "xsel error: {}",
                error
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_clipboard_operations() {
        // Test clipboard functionality
        let test_content = "test content";

        match set_clipboard_content(test_content).await {
            Ok(()) => println!("✅ Set clipboard successfully"),
            Err(e) => println!("❌ Set clipboard error: {e}"),
        }

        match get_clipboard_content().await {
            Ok(content) => println!("✅ Clipboard content: {content}"),
            Err(e) => println!("❌ Get clipboard error: {e}"),
        }
    }
}
