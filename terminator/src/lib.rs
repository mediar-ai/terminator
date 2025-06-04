//! Desktop UI automation through accessibility APIs
//!
//! This module provides a cross-platform API for automating desktop applications
//! through accessibility APIs, inspired by Playwright's web automation model.

use std::sync::Arc;
use std::time::{Duration, Instant};
use std::fmt;
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

pub use element::{UIElement, UIElementAttributes, SerializableUIElement};
pub use errors::AutomationError;
pub use locator::Locator;
pub use selector::Selector;

#[cfg(target_os = "windows")]
pub use platforms::windows::convert_uiautomation_element_to_terminator;

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
#[derive(Clone, Serialize, Deserialize)]
pub struct UINode {
    pub attributes: UIElementAttributes,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<UINode>,
}

impl fmt::Debug for UINode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.debug_with_depth(f, 0, 100)
    }
}

impl UINode {
    /// Helper method for debug formatting with depth control
    fn debug_with_depth(&self, f: &mut fmt::Formatter<'_>, current_depth: usize, max_depth: usize) -> fmt::Result {
        let mut debug_struct = f.debug_struct("UINode");
        debug_struct.field("attributes", &self.attributes);
        
        if !self.children.is_empty() {
            if current_depth < max_depth {
                debug_struct.field("children", &DebugChildrenWithDepth {
                    children: &self.children,
                    current_depth,
                    max_depth,
                });
            } else {
                debug_struct.field("children", &format!("[{} children (depth limit reached)]", self.children.len()));
            }
        }
        
        debug_struct.finish()
    }
}

/// Helper struct for debug formatting children with depth control
struct DebugChildrenWithDepth<'a> {
    children: &'a Vec<UINode>,
    current_depth: usize,
    max_depth: usize,
}

impl<'a> fmt::Debug for DebugChildrenWithDepth<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut list = f.debug_list();
        
        // Show ALL children, no limit
        for child in self.children.iter() {
            list.entry(&DebugNodeWithDepth {
                node: child,
                current_depth: self.current_depth + 1,
                max_depth: self.max_depth,
            });
        }
        
        list.finish()
    }
}

/// Helper struct for debug formatting a single node with depth control
struct DebugNodeWithDepth<'a> {
    node: &'a UINode,
    current_depth: usize,
    max_depth: usize,
}

impl<'a> fmt::Debug for DebugNodeWithDepth<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.node.debug_with_depth(f, self.current_depth, self.max_depth)
    }
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
    pub fn new(
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

    /// Enable or disable background cache warming for improved performance
    /// 
    /// This feature spawns a background thread that periodically builds UI trees for
    /// frequently used applications, keeping the native platform cache warm.
    /// This can significantly improve performance when your recorder needs to fetch
    /// full UI trees for applications.
    /// 
    /// # Arguments
    /// * `enable` - Whether to enable (true) or disable (false) the cache warmer
    /// * `interval_seconds` - How often to refresh the cache (default: 30 seconds)
    /// * `max_apps_to_cache` - Maximum number of recent apps to keep cached (default: 10)
    /// 
    /// # Example
    /// ```no_run
    /// use terminator::Desktop;
    /// 
    /// let desktop = Desktop::new(false, false)?;
    /// 
    /// // Enable cache warming every 60 seconds for up to 15 apps
    /// desktop.enable_background_cache_warmer(true, Some(60), Some(15))?;
    /// 
    /// // Later, disable cache warming
    /// desktop.enable_background_cache_warmer(false, None, None)?;
    /// # Ok::<(), terminator::AutomationError>(())
    /// ```
    pub fn enable_background_cache_warmer(
        &self,
        enable: bool,
        interval_seconds: Option<u64>,
        max_apps_to_cache: Option<usize>,
    ) -> Result<(), AutomationError> {
        self.engine.enable_background_cache_warmer(enable, interval_seconds, max_apps_to_cache)
    }

    /// Check if the background cache warmer is currently running
    /// 
    /// # Returns
    /// `true` if the cache warmer is active, `false` otherwise
    pub fn is_cache_warmer_enabled(&self) -> bool {
        self.engine.is_cache_warmer_enabled()
    }

    /// Returns the root element of the desktop
    /// 
    /// The root element represents the desktop and provides access to all UI elements
    /// currently visible on the screen.
    /// 
    /// # Examples
    /// 
    /// ```no_run
    /// use terminator::Desktop;
    /// 
    /// let desktop = Desktop::new(false, false)?;
    /// let root = desktop.root();
    /// println!("Desktop root: {:?}", root.role());
    /// # Ok::<(), terminator::AutomationError>(())
    /// ```
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
    pub fn open_application(&self, app_name: &str) -> Result<UIElement, AutomationError> {
        let start = Instant::now();
        info!(app_name, "Opening application");
        
        let app = self.engine.open_application(app_name)?;
        
        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            "Application opened"
        );
        
        Ok(app)
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

    #[instrument(skip(self))]
    pub async fn get_active_monitor_name(&self) -> Result<String, AutomationError> {
        // Get all windows
        let windows = xcap::Window::all().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get windows: {}", e))
        })?;

        // Find the focused window
        let focused_window = windows.iter()
            .find(|w| w.is_focused().unwrap_or(false))
            .ok_or_else(|| {
                AutomationError::ElementNotFound("No focused window found".to_string())
            })?;

        // Get the monitor name for the focused window
        let monitor = focused_window.current_monitor().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get current monitor: {}", e))
        })?;

        let monitor_name = monitor.name().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitor name: {}", e))
        })?;

        Ok(monitor_name)
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

    #[instrument(skip(self, pid, title))]
    pub fn get_window_tree_by_pid_and_title(&self, pid: u32, title: Option<&str>) -> Result<UINode, AutomationError> {
        let start = Instant::now();
        info!(pid, ?title, "Getting window tree by PID and title");

        let window_tree_root = self.engine.get_window_tree_by_pid_and_title(pid, title)?;

        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            pid = pid,
            ?title,
            "Window tree retrieved by PID and title"
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

