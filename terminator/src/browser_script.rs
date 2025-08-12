//! Browser script execution using Chrome extension ONLY
//!
//! Simple and clean - just uses the extension bridge, no DevTools fallback.

use crate::AutomationError;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Execute JavaScript in browser using extension bridge ONLY
pub async fn execute_script(
    browser_element: &crate::UIElement,
    script: &str,
) -> Result<String, AutomationError> {
    info!("🚀 Executing JavaScript via extension bridge");
    debug!(
        script_bytes = script.len(),
        script_preview = %script.chars().take(200).collect::<String>(),
        "Preparing to execute browser script"
    );

    // Focus browser to ensure extension targets the right tab
    browser_element.focus()?;
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Wait for extension to connect if not already connected (tolerate worker backoff)
    let ext = crate::extension_bridge::ExtensionBridge::global().await;
    if !ext.is_client_connected().await {
        info!("Waiting for extension client to connect...");
        // Give the service worker time to reconnect with its own backoff (up to 3s now, previously 15s)
        let mut connected = false;
        for i in 0..120 {
            // up to ~60s
            tokio::time::sleep(Duration::from_millis(500)).await;
            if ext.is_client_connected().await {
                info!("Extension client connected after {} ms", (i + 1) * 500);
                connected = true;
                break;
            }
            if i % 6 == 5 {
                // every ~3s
                info!(
                    "Still waiting for extension client... {}s",
                    ((i + 1) * 500) / 1000
                );
            }
        }
        if !connected {
            warn!("Extension still not connected after waiting; proceeding (will error if eval is required)");
        }
    }

    // Execute via extension bridge
    match crate::extension_bridge::try_eval_via_extension(script, Duration::from_secs(360)).await {
        Ok(Some(result)) => {
            info!("✅ Script executed successfully via extension");
            if result.trim_start().starts_with("ERROR:") {
                let raw = result.trim_start().trim_start_matches("ERROR:").trim();
                // Try to parse structured JSON error
                match serde_json::from_str::<serde_json::Value>(raw) {
                    Ok(val) => {
                        let msg = val.get("message").and_then(|v| v.as_str()).unwrap_or("");
                        let details = val
                            .get("details")
                            .cloned()
                            .unwrap_or(serde_json::Value::Null);
                        error!(message = %msg, details = %details, "Browser script error (structured)");
                    }
                    Err(_) => {
                        warn!(
                            error_head = %result.lines().next().unwrap_or("ERROR"),
                            "Browser script returned an error payload"
                        );
                    }
                }
            }
            Ok(result)
        }
        Ok(None) => {
            error!(
                "Extension eval returned None (no client?). Ensure extension is installed and connected"
            );
            Err(AutomationError::PlatformError(
                "Extension bridge not connected. Make sure Chrome extension is installed.".into(),
            ))
        }
        Err(e) => {
            error!(
                error = %e,
                "Extension bridge error while executing browser script"
            );
            Err(AutomationError::PlatformError(format!(
                "Extension bridge error: {e}"
            )))
        }
    }
}
