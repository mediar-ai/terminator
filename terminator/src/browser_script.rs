//! Browser script execution using Chrome extension ONLY
//!
//! Simple and clean - just uses the extension bridge, no DevTools fallback.

use crate::AutomationError;
use std::time::Duration;
use tracing::info;

/// Execute JavaScript in browser using extension bridge ONLY
pub async fn execute_script(
    browser_element: &crate::UIElement,
    script: &str,
) -> Result<String, AutomationError> {
    info!("🚀 Executing JavaScript via extension bridge");

    // Focus browser to ensure extension targets the right tab
    browser_element.focus()?;
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Wait for extension to connect if not already connected
    let ext = crate::extension_bridge::ExtensionBridge::global().await;
    if !ext.is_client_connected().await {
        info!("Waiting for extension client to connect...");
        for i in 0..20 {
            // Increased from 10 to 20 (6 seconds total)
            tokio::time::sleep(Duration::from_millis(300)).await;
            if ext.is_client_connected().await {
                info!("Extension client connected after {} ms", (i + 1) * 300);
                break;
            }
        }

        // Final check after waiting
        if !ext.is_client_connected().await {
            info!("Extension still not connected after 6 seconds wait");
        }
    }

    // Execute via extension bridge
    match crate::extension_bridge::try_eval_via_extension(script, Duration::from_secs(360)).await {
        Ok(Some(result)) => {
            info!("✅ Script executed successfully via extension");
            Ok(result)
        }
        Ok(None) => Err(AutomationError::PlatformError(
            "Extension bridge not connected. Make sure Chrome extension is installed.".into(),
        )),
        Err(e) => Err(AutomationError::PlatformError(format!(
            "Extension bridge error: {e}"
        ))),
    }
}
