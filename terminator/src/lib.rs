//! Desktop UI automation through accessibility APIs
//!
//! This module provides a cross-platform API for automating desktop applications
//! through accessibility APIs, inspired by Playwright's web automation model.

use std::sync::Arc;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use tracing::{info, instrument, warn};

pub mod element;
pub mod errors;
pub mod locator;
pub mod platforms;
pub mod selector;
#[cfg(test)]
mod tests;
pub mod utils;

pub use element::{UIElement, UIElementAttributes};
pub use errors::AutomationError;
pub use locator::Locator;
pub use selector::Selector;

// Define a new struct to hold click result information - move to module level
pub struct ClickResult {
    pub method: String,
    pub coordinates: Option<(f64, f64)>,
    pub details: String,
}

/// Holds the output of a terminal command execution
pub struct CommandOutput {
    pub exit_status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Represents a node in the UI tree, containing its attributes and children.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UINode {
    pub attributes: UIElementAttributes,
    pub children: Vec<UINode>,
}

/// Holds the screenshot data
#[derive(Debug, Clone)]
pub struct ScreenshotResult {
    /// Raw image data (e.g., RGBA)
    pub image_data: Vec<u8>,
    /// Width of the image
    pub width: u32,
    /// Height of the image
    pub height: u32,
}

/// The main entry point for UI automation
pub struct Desktop {
    engine: Arc<dyn platforms::AccessibilityEngine>,
}

impl Desktop {
    #[instrument(skip(use_background_apps, activate_app))]
    pub async fn new(
        use_background_apps: bool,
        activate_app: bool,
    ) -> Result<Self, AutomationError> {
        let start = Instant::now();
        info!("Initializing Desktop automation engine");
        
        let engine = platforms::create_engine(use_background_apps, activate_app)?;
        
        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            use_background_apps,
            activate_app,
            "Desktop automation engine initialized"
        );
        
        Ok(Self {
            engine: Arc::from(engine),
        })
    }

    #[instrument(skip(self))]
    pub fn root(&self) -> UIElement {
        let start = Instant::now();
        info!("Getting root element");
        
        let element = self.engine.get_root_element();
        
        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            element_id = element.id().unwrap_or_default(),
            "Root element retrieved"
        );
        
        element
    }

    #[instrument(skip(self, selector))]
    pub fn locator(&self, selector: impl Into<Selector>) -> Locator {
        let start = Instant::now();
        let selector = selector.into();
        info!(?selector, "Creating locator");
        
        let locator = Locator::new(self.engine.clone(), selector);
        
        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            "Locator created"
        );
        
        locator
    }

    #[instrument(skip(self))]
    pub fn focused_element(&self) -> Result<UIElement, AutomationError> {
        let start = Instant::now();
        info!("Getting focused element");
        
        let element = self.engine.get_focused_element()?;
        
        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            element_id = element.id().unwrap_or_default(),
            "Focused element retrieved"
        );
        
        Ok(element)
    }

    #[instrument(skip(self))]
    pub fn applications(&self) -> Result<Vec<UIElement>, AutomationError> {
        let start = Instant::now();
        info!("Getting all applications");
        
        let apps = self.engine.get_applications()?;
        
        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            app_count = apps.len(),
            "Applications retrieved"
        );
        
        Ok(apps)
    }

    #[instrument(skip(self, name))]
    pub fn application(&self, name: &str) -> Result<UIElement, AutomationError> {
        let start = Instant::now();
        info!(app_name = name, "Getting application by name");
        
        let app = self.engine.get_application_by_name(name)?;
        
        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            app_id = app.id().unwrap_or_default(),
            "Application retrieved"
        );
        
        Ok(app)
    }

    #[instrument(skip(self, app_name))]
    pub fn open_application(&self, app_name: &str) -> Result<(), AutomationError> {
        let start = Instant::now();
        info!(app_name, "Opening application");
        
        self.engine.open_application(app_name)?;
        
        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            "Application opened"
        );
        
        Ok(())
    }

    #[instrument(skip(self, app_name))]
    pub fn activate_application(&self, app_name: &str) -> Result<(), AutomationError> {
        let start = Instant::now();
        info!(app_name, "Activating application");
        
        self.engine.activate_application(app_name)?;
        
        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            "Application activated"
        );
        
        Ok(())
    }

    #[instrument(skip(self, url, browser))]
    pub fn open_url(&self, url: &str, browser: Option<&str>) -> Result<(), AutomationError> {
        let start = Instant::now();
        info!(url, ?browser, "Opening URL");
        
        self.engine.open_url(url, browser)?;
        
        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            "URL opened"
        );
        
        Ok(())
    }

    #[instrument(skip(self, file_path))]
    pub fn open_file(&self, file_path: &str) -> Result<(), AutomationError> {
        let start = Instant::now();
        info!(file_path, "Opening file");
        
        self.engine.open_file(file_path)?;
        
        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            "File opened"
        );
        
        Ok(())
    }

    #[instrument(skip(self, windows_command, unix_command))]
    pub async fn run_command(
        &self,
        windows_command: Option<&str>,
        unix_command: Option<&str>,
    ) -> Result<CommandOutput, AutomationError> {
        let start = Instant::now();
        info!(?windows_command, ?unix_command, "Running command");
        
        let output = self.engine.run_command(windows_command, unix_command).await?;
        
        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            exit_code = output.exit_status,
            stdout_len = output.stdout.len(),
            stderr_len = output.stderr.len(),
            "Command completed"
        );
        
        Ok(output)
    }

    #[instrument(skip(self))]
    pub async fn capture_screen(&self) -> Result<ScreenshotResult, AutomationError> {
        let start = Instant::now();
        info!("Capturing screen");
        
        let screenshot = self.engine.capture_screen().await?;
        
        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            width = screenshot.width,
            height = screenshot.height,
            "Screen captured"
        );
        
        Ok(screenshot)
    }

    #[instrument(skip(self, name))]
    pub async fn capture_monitor_by_name(
        &self,
        name: &str,
    ) -> Result<ScreenshotResult, AutomationError> {
        let start = Instant::now();
        info!(monitor_name = name, "Capturing monitor");
        
        let screenshot = self.engine.capture_monitor_by_name(name).await?;
        
        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            width = screenshot.width,
            height = screenshot.height,
            "Monitor captured"
        );
        
        Ok(screenshot)
    }

    #[instrument(skip(self, image_path))]
    pub async fn ocr_image_path(&self, image_path: &str) -> Result<String, AutomationError> {
        let start = Instant::now();
        info!(image_path, "Performing OCR on image file");
        
        let text = self.engine.ocr_image_path(image_path).await?;
        
        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            text_length = text.len(),
            "OCR completed"
        );
        
        Ok(text)
    }

    #[instrument(skip(self, screenshot))]
    pub async fn ocr_screenshot(
        &self,
        screenshot: &ScreenshotResult,
    ) -> Result<String, AutomationError> {
        let start = Instant::now();
        info!(
            width = screenshot.width,
            height = screenshot.height,
            "Performing OCR on screenshot"
        );
        
        let text = self.engine.ocr_screenshot(screenshot).await?;
        
        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            text_length = text.len(),
            "OCR completed"
        );
        
        Ok(text)
    }

    #[instrument(skip(self, title))]
    pub fn activate_browser_window_by_title(&self, title: &str) -> Result<(), AutomationError> {
        let start = Instant::now();
        info!(title, "Activating browser window");
        
        self.engine.activate_browser_window_by_title(title)?;
        
        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            "Browser window activated"
        );
        
        Ok(())
    }

    #[instrument(skip(self, title_contains, timeout))]
    pub async fn find_window_by_criteria(
        &self,
        title_contains: Option<&str>,
        timeout: Option<Duration>,
    ) -> Result<UIElement, AutomationError> {
        let start = Instant::now();
        info!(?title_contains, ?timeout, "Finding window by criteria");
        
        let window = self.engine.find_window_by_criteria(title_contains, timeout).await?;
        
        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            window_id = window.id().unwrap_or_default(),
            "Window found"
        );
        
        Ok(window)
    }

    #[instrument(skip(self))]
    pub async fn get_current_browser_window(&self) -> Result<UIElement, AutomationError> {
        let start = Instant::now();
        info!("Getting current browser window");
        
        let window = self.engine.get_current_browser_window().await?;
        
        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            window_id = window.id().unwrap_or_default(),
            "Current browser window retrieved"
        );
        
        Ok(window)
    }

    #[instrument(skip(self))]
    pub async fn get_current_window(&self) -> Result<UIElement, AutomationError> {
        let start = Instant::now();
        info!("Getting current window");

        let window = self.engine.get_current_window().await?;

        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            window_id = window.id().unwrap_or_default(),
            "Current window retrieved"
        );

        Ok(window)
    }

    #[instrument(skip(self))]
    pub async fn get_current_application(&self) -> Result<UIElement, AutomationError> {
        let start = Instant::now();
        info!("Getting current application");

        let application = self.engine.get_current_application().await?;

        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            app_id = application.id().unwrap_or_default(),
            "Current application retrieved"
        );

        Ok(application)
    }

    #[instrument(skip(self, title))]
    pub fn get_window_tree_by_title(&self, title: &str) -> Result<UINode, AutomationError> {
        let start = Instant::now();
        info!(title, "Getting window tree by title");

        let window_tree_root = self.engine.get_window_tree_by_title(title)?;

        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            title = title,
            "Window tree retrieved"
        );

        Ok(window_tree_root)
    }
}

impl Clone for Desktop {
    fn clone(&self) -> Self {
        Self {
            engine: self.engine.clone(),
        }
    }
}

