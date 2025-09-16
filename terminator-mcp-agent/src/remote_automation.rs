// Abstraction layer for UI automation that handles both local and remote scenarios
// This ensures we don't break existing behavior and provides a clean interface

use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::Duration;
use terminator::{Desktop, UIElement, Selector};
use crate::remote_common::{RemoteAction, MouseButton, WaitCondition};
use crate::utils::DesktopWrapper;

/// Trait for UI automation operations that can be implemented for both local and remote scenarios
#[async_trait::async_trait]
pub trait UIAutomation: Send + Sync {
    async fn get_applications(&self) -> Result<Vec<serde_json::Value>>;
    async fn get_window_tree(&self, pid: Option<u32>) -> Result<serde_json::Value>;
    async fn click(&self, selector: &str, button: MouseButton) -> Result<()>;
    async fn type_text(&self, selector: &str, text: &str) -> Result<()>;
    async fn press_key(&self, selector: &str, key: &str) -> Result<()>;
    async fn get_element_properties(&self, selector: &str) -> Result<serde_json::Value>;
    async fn wait_for_element(&self, selector: &str, condition: WaitCondition, timeout_ms: u64) -> Result<()>;
    async fn take_screenshot(&self, selector: Option<&str>) -> Result<Vec<u8>>;
    async fn set_value(&self, selector: &str, value: &str) -> Result<()>;
    async fn invoke_element(&self, selector: &str) -> Result<()>;
    async fn validate_element(&self, selector: &str) -> Result<serde_json::Value>;
}

/// Local implementation that wraps the Desktop API
pub struct LocalUIAutomation {
    desktop: Arc<Desktop>,
}

impl LocalUIAutomation {
    pub fn new(desktop: Arc<Desktop>) -> Self {
        Self { desktop }
    }

    fn create_selector(&self, selector: &str) -> Selector {
        Selector::from(selector)
    }

    async fn find_element(&self, selector: &str) -> Result<UIElement> {
        // Run blocking operation in tokio's blocking thread pool
        let desktop = self.desktop.clone();
        let selector_str = selector.to_string();

        let result = tokio::task::spawn_blocking(move || {
            let selector = Selector::from(selector_str.as_str());
            desktop.locator(selector)
                .first(Some(Duration::from_secs(5)))
        })
        .await?;

        result.context("Element not found")
    }
}

#[async_trait::async_trait]
impl UIAutomation for LocalUIAutomation {
    async fn get_applications(&self) -> Result<Vec<serde_json::Value>> {
        let desktop = self.desktop.clone();
        let apps = tokio::task::spawn_blocking(move || {
            desktop.applications()
        })
        .await??;

        // Convert UIElements to JSON
        let app_json: Vec<serde_json::Value> = apps.iter().map(|app| {
            serde_json::json!({
                "name": app.name().unwrap_or_default(),
                "role": app.role(),
                "window_title": app.window_title(),
            })
        }).collect();

        Ok(app_json)
    }

    async fn get_window_tree(&self, pid: Option<u32>) -> Result<serde_json::Value> {
        let apps = self.get_applications().await?;

        if let Some(_pid) = pid {
            // Note: terminator doesn't provide PID directly, would need Windows API
            Ok(serde_json::json!({
                "applications": apps,
                "note": "PID filtering requires Windows API integration"
            }))
        } else {
            Ok(serde_json::json!({
                "applications": apps
            }))
        }
    }

    async fn click(&self, selector: &str, button: MouseButton) -> Result<()> {
        let element = self.find_element(selector).await?;

        tokio::task::spawn_blocking(move || {
            match button {
                MouseButton::Left => element.click().map(|_| ()),
                MouseButton::Right => element.right_click(),
                MouseButton::Middle => element.click().map(|_| ()), // Middle click not directly supported
            }
        })
        .await??;

        Ok(())
    }

    async fn type_text(&self, selector: &str, text: &str) -> Result<()> {
        let element = self.find_element(selector).await?;
        let text = text.to_string();

        tokio::task::spawn_blocking(move || {
            element.type_text(&text)
        })
        .await??;

        Ok(())
    }

    async fn press_key(&self, selector: &str, key: &str) -> Result<()> {
        let element = self.find_element(selector).await?;
        let key = key.to_string();

        tokio::task::spawn_blocking(move || {
            element.press_key(&key)
        })
        .await??;

        Ok(())
    }

    async fn get_element_properties(&self, selector: &str) -> Result<serde_json::Value> {
        let element = self.find_element(selector).await?;

        let props = tokio::task::spawn_blocking(move || {
            Ok::<_, anyhow::Error>(serde_json::json!({
                "name": element.name()?,
                "role": element.role(),
                "is_enabled": element.is_enabled()?,
                "is_visible": element.is_visible()?,
                "bounds": element.bounds()?,
                "value": element.value().unwrap_or_default(),
            }))
        })
        .await??;

        Ok(props)
    }

    async fn wait_for_element(&self, selector: &str, condition: WaitCondition, timeout_ms: u64) -> Result<()> {
        let timeout = Duration::from_millis(timeout_ms);
        let start = std::time::Instant::now();

        loop {
            if start.elapsed() > timeout {
                return Err(anyhow::anyhow!("Timeout waiting for element"));
            }

            if let Ok(element) = self.find_element(selector).await {
                let met = tokio::task::spawn_blocking(move || {
                    match condition {
                        WaitCondition::Visible => element.is_visible(),
                        WaitCondition::Enabled => element.is_enabled(),
                        WaitCondition::Focused => element.is_focused(),
                        WaitCondition::Exists => Ok(true),
                    }
                })
                .await??;

                if met {
                    return Ok(());
                }
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    async fn take_screenshot(&self, selector: Option<&str>) -> Result<Vec<u8>> {
        if let Some(sel) = selector {
            let element = self.find_element(sel).await?;
            tokio::task::spawn_blocking(move || {
                element.capture_screenshot()
            })
            .await?
        } else {
            let desktop = self.desktop.clone();
            tokio::task::spawn_blocking(move || {
                desktop.screenshot()
            })
            .await?
        }
    }

    async fn set_value(&self, selector: &str, value: &str) -> Result<()> {
        let element = self.find_element(selector).await?;
        let value = value.to_string();

        tokio::task::spawn_blocking(move || {
            element.set_value(&value)
        })
        .await??;

        Ok(())
    }

    async fn invoke_element(&self, selector: &str) -> Result<()> {
        let element = self.find_element(selector).await?;

        tokio::task::spawn_blocking(move || {
            element.invoke()
        })
        .await??;

        Ok(())
    }

    async fn validate_element(&self, selector: &str) -> Result<serde_json::Value> {
        match self.find_element(selector).await {
            Ok(element) => {
                let validation = tokio::task::spawn_blocking(move || {
                    Ok::<_, anyhow::Error>(serde_json::json!({
                        "exists": true,
                        "name": element.name()?,
                        "role": element.role(),
                        "is_enabled": element.is_enabled()?,
                        "is_visible": element.is_visible()?,
                    }))
                })
                .await??;

                Ok(validation)
            }
            Err(_) => {
                Ok(serde_json::json!({ "exists": false }))
            }
        }
    }
}