//! Desktop UI automation through accessibility APIs
//!
//! This module provides a cross-platform API for automating desktop applications
//! through accessibility APIs, inspired by Playwright's web automation model.

use std::sync::Arc;
use std::time::{Instant};
use std::fmt;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, instrument, warn};

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

    /// Gets the root element representing the entire desktop.
    ///
    /// This is the top-level element that contains all applications, windows,
    /// and UI elements on the desktop. You can use it as a starting point for
    /// element searches.
    ///
    /// # Examples
    ///
    /// ```
    /// use terminator::Desktop;
    /// let desktop = Desktop::new(false, false)?;
    /// let root = desktop.root();
    /// println!("Root element ID: {:?}", root.id());
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

    #[instrument(skip(self, pid, title, config))]
    pub fn get_window_tree(&self, pid: u32, title: Option<&str>, config: Option<crate::platforms::TreeBuildConfig>) -> Result<UINode, AutomationError> {
        let start = Instant::now();
        info!(pid, ?title, "Getting window tree with config");

        let tree_config = config.unwrap_or_default();
        let window_tree_root = self.engine.get_window_tree(pid, title, tree_config)?;

        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            pid = pid,
            ?title,
            "Window tree retrieved"
        );

        Ok(window_tree_root)
    }

    /// Convenience method: Get window tree by title only (assumes you have the PID)
    /// For backward compatibility - you should prefer get_window_tree() with PID
    #[instrument(skip(self, title))]
    pub fn get_window_tree_by_title(&self, title: &str) -> Result<UINode, AutomationError> {
        // This is a legacy method - in practice you should always use get_window_tree() with PID
        // We'll try to find a window by title and extract its PID, but this is less reliable
        
        warn!("get_window_tree_by_title is deprecated - use get_window_tree() with PID for better reliability");
        
        // Try to find a window with this title to get its PID  
        // This is a best-effort fallback for backward compatibility
        let applications = self.engine.get_applications()?;
        
        for app in applications {
            if let Ok(pid) = app.process_id() {
                // Try this PID with the title
                if let Ok(tree) = self.get_window_tree(pid, Some(title), None) {
                    return Ok(tree);
                }
            }
        }
        
        Err(AutomationError::ElementNotFound(
            format!("Could not find window with title '{}' - consider using get_window_tree() with PID", title)
        ))
    }

    /// Get window tree by PID and optional title (main recommended method)
    #[instrument(skip(self, pid, title))]
    pub fn get_window_tree_by_pid_and_title(&self, pid: u32, title: Option<&str>) -> Result<UINode, AutomationError> {
        // This is now just a wrapper around the main get_window_tree method
        self.get_window_tree(pid, title, None)
    }

    /// Get all window elements for a given application by name
    #[instrument(skip(self, app_name))]
    pub async fn windows_for_application(&self, app_name: &str) -> Result<Vec<UIElement>, AutomationError> {
        let start = Instant::now();
        debug!(app_name, "Getting windows for application");

        // 1. Find the application element
        let app_element = match self.application(app_name) {
            Ok(app) => app,
            Err(e) => {
                error!("Application '{}' not found: {}", app_name, e);
                return Err(e);
            }
        };
        debug!("Found application element for '{}'", app_name);

        // 2. Get children of the application element
        let children = match app_element.children() {
            Ok(ch) => ch,
            Err(e) => {
                error!("Failed to get children for application '{}': {}", app_name, e);
                return Err(e);
            }
        };
        debug!(child_count = children.len(), "Found children for application '{}" , app_name);

        // 3. Filter children to find windows (cross-platform)
        let windows: Vec<UIElement> = children
            .into_iter()
            .filter(|el| {
                let role = el.role().to_lowercase();
                #[cfg(target_os = "macos")]
                {
                    role == "axwindow" || role == "window"
                }
                #[cfg(target_os = "windows")]
                {
                    role == "window"
                }
                #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                {
                    // Fallback: just look for 'window' role
                    role == "window"
                }
            })
            .collect();

            debug!(window_count = windows.len(), "Found windows for application '{}'", app_name);
        let duration = start.elapsed();
        debug!(duration_ms = duration.as_millis(), "windows_for_application complete");
        Ok(windows)
    }
}

impl Clone for Desktop {
    fn clone(&self) -> Self {
        Self {
            engine: self.engine.clone(),
        }
    }
}

