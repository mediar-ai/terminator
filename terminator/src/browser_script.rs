//! Browser script execution using Chrome extension ONLY
//!
//! Simple and clean - just uses the extension bridge, no DevTools fallback.

use crate::{AutomationError, Browser, Desktop, Selector, UIElement};
use std::time::Duration;
use tracing::{debug, error, info, warn};

async fn wake_extension_service_worker_and_restore_focus(previously_focused: Option<UIElement>) {
    // Best-effort: try to open Chrome's extensions page and click the Service Worker link
    // Then restore the previously focused UI element to avoid disrupting the user.
    let wake_attempt = (|| -> Result<(), AutomationError> {
        let desktop = Desktop::new_default()?;

        // Open in Chrome explicitly to ensure we're targeting the correct browser settings UI
        let _chrome_window = desktop.open_url("chrome://extensions", Some(Browser::Chrome))?;

        Ok(())
    })();

    if wake_attempt.is_err() {
        warn!(
            error = %wake_attempt.err().unwrap(),
            "Failed to open chrome://extensions for service worker wakeup"
        );
        return;
    }

    // Give the page a brief moment to render
    tokio::time::sleep(Duration::from_millis(800)).await;

    // Try a few selector variants that commonly match the service worker inspect link text
    // This is intentionally permissive and case-insensitive via contains behavior in matchers.
    let candidate_selectors = ["role:link|name:service worker"];

    // Best-effort clicking; ignore errors if not found.
    if let Ok(desktop) = Desktop::new_default() {
        for selector_str in candidate_selectors.iter() {
            let try_click = async {
                let locator = desktop.locator(Selector::from(*selector_str));
                match locator.first(Some(Duration::from_millis(1500))).await {
                    Ok(link) => link.invoke().map(|_| true).unwrap_or(false),
                    Err(_) => false,
                }
            };

            if try_click.await {
                info!(
                    selector = *selector_str,
                    "Clicked service worker link to wake extension"
                );
                // Allow devtools window to spawn and the worker to initialize
                tokio::time::sleep(Duration::from_millis(800)).await;
                break;
            }
        }
    }

    // Restore original focus to minimize disruption
    if let Some(prev) = previously_focused {
        let _ = prev.activate_window();
    }
}

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

    // Capture current focus to restore later if we have to open chrome://extensions
    let previously_focused = Desktop::new_default()
        .ok()
        .and_then(|d| d.focused_element().ok());

    // Focus the browser to ensure the extension targets the right tab
    browser_element.focus()?;
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Wait for extension to connect if not already connected (tolerate worker backoff)
    let ext = crate::extension_bridge::ExtensionBridge::global().await;
    if !ext.is_client_connected().await {
        info!("Waiting for extension client to connect...");
        // Try a short wait first, then attempt to wake the service worker via extensions UI,
        // then continue to wait up to the original overall budget (~60s total)
        let mut connected = false;
        let mut attempted_wakeup = false;
        for i in 0..120 {
            tokio::time::sleep(Duration::from_millis(500)).await;

            if ext.is_client_connected().await {
                info!("Extension client connected after {} ms", (i + 1) * 500);
                connected = true;
                break;
            }

            // After ~4 seconds without a connection, try to wake the service worker once
            if !attempted_wakeup && i >= 8 {
                attempted_wakeup = true;
                info!("Attempting to wake extension service worker via chrome://extensions");
                wake_extension_service_worker_and_restore_focus(previously_focused.clone()).await;

                // Re-focus the browser window to ensure the active tab is correct for evaluation
                let _ = browser_element.focus();
                tokio::time::sleep(Duration::from_millis(300)).await;
            }

            if i % 6 == 5 {
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

    // Ensure browser tab is active right before eval (in case focus was restored earlier)
    let _ = browser_element.focus();
    tokio::time::sleep(Duration::from_millis(100)).await;

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
            // Best-effort: restore original focus before returning error
            if let Some(prev) = previously_focused {
                let _ = prev.activate_window();
            }
            Err(AutomationError::PlatformError(
                "Extension bridge not connected. Make sure Chrome extension is installed.".into(),
            ))
        }
        Err(e) => {
            error!(
                error = %e,
                "Extension bridge error while executing browser script"
            );
            // Best-effort: restore original focus before returning error
            if let Some(prev) = previously_focused.clone() {
                let _ = prev.activate_window();
            }
            Err(AutomationError::PlatformError(format!(
                "Extension bridge error: {e}"
            )))
        }
    }
}
