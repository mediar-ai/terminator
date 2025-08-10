//! Desktop UI automation through accessibility APIs
//!
//! This module provides a cross-platform API for automating desktop applications
//! through accessibility APIs, inspired by Playwright's web automation model.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use tracing::{debug, error, instrument};

pub mod browser_script;
pub mod element;
pub mod errors;
pub mod extension_bridge;
pub mod locator;
pub mod platforms;
pub mod selector;
#[cfg(test)]
mod tests;
pub mod types;
pub mod utils;

pub use element::{SerializableUIElement, UIElement, UIElementAttributes};
pub use errors::AutomationError;
pub use locator::Locator;
pub use selector::Selector;
pub use types::{FontStyle, HighlightHandle, TextPosition};

/// Recommend to use any of these: ["Default", "Chrome", "Firefox", "Edge", "Brave", "Opera", "Vivaldi"]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Browser {
    Default,
    Chrome,
    Firefox,
    Edge,
    Brave,
    Opera,
    Vivaldi,
    Custom(String),
}

#[cfg(target_os = "windows")]
pub use platforms::windows::{convert_uiautomation_element_to_terminator, get_process_name_by_pid};

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

/// Represents a monitor/display device
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Monitor {
    /// Unique identifier for the monitor
    pub id: String,
    /// Human-readable name of the monitor
    pub name: String,
    /// Whether this is the primary monitor
    pub is_primary: bool,
    /// Monitor dimensions
    pub width: u32,
    pub height: u32,
    /// Monitor position (top-left corner)
    pub x: i32,
    pub y: i32,
    /// Scale factor (e.g., 1.0 for 100%, 1.25 for 125%)
    pub scale_factor: f64,
}

impl Monitor {
    /// Capture a screenshot of this monitor
    #[instrument(skip(self, desktop))]
    pub async fn capture(&self, desktop: &Desktop) -> Result<ScreenshotResult, AutomationError> {
        desktop.engine.capture_monitor_by_id(&self.id).await
    }

    /// Check if this monitor contains the given coordinates
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && x < self.x + self.width as i32
            && y >= self.y
            && y < self.y + self.height as i32
    }

    /// Get the center point of this monitor
    pub fn center(&self) -> (i32, i32) {
        (
            self.x + self.width as i32 / 2,
            self.y + self.height as i32 / 2,
        )
    }
}

/// Represents a node in the UI tree, containing its attributes and children.
#[derive(Clone, Serialize, Deserialize, Default)]
pub struct UINode {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
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
    fn debug_with_depth(
        &self,
        f: &mut fmt::Formatter<'_>,
        current_depth: usize,
        max_depth: usize,
    ) -> fmt::Result {
        let mut debug_struct = f.debug_struct("UINode");
        debug_struct.field("attributes", &self.attributes);

        if !self.children.is_empty() {
            if current_depth < max_depth {
                debug_struct.field(
                    "children",
                    &DebugChildrenWithDepth {
                        children: &self.children,
                        current_depth,
                        max_depth,
                    },
                );
            } else {
                debug_struct.field(
                    "children",
                    &format!("[{} children (depth limit reached)]", self.children.len()),
                );
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

impl fmt::Debug for DebugChildrenWithDepth<'_> {
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

impl fmt::Debug for DebugNodeWithDepth<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.node
            .debug_with_depth(f, self.current_depth, self.max_depth)
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
    /// Monitor information if captured from a specific monitor
    pub monitor: Option<Monitor>,
}

/// The main entry point for UI automation
pub struct Desktop {
    engine: Arc<dyn platforms::AccessibilityEngine>,
}

impl Desktop {
    #[instrument(skip(use_background_apps, activate_app))]
    pub fn new(use_background_apps: bool, activate_app: bool) -> Result<Self, AutomationError> {
        let engine = platforms::create_engine(use_background_apps, activate_app)?;
        Ok(Self { engine })
    }

    /// Initializet the desktop without arguments
    ///
    /// This is a convenience method that calls `new` with default arguments.
    ///
    /// # Examples
    ///
    /// ```
    /// use terminator::Desktop;
    /// let desktop = Desktop::new_default()?;
    /// # Ok::<(), terminator::AutomationError>(())
    /// ```
    pub fn new_default() -> Result<Self, AutomationError> {
        Self::new(false, false)
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
        self.engine.get_root_element()
    }

    #[instrument(level = "debug", skip(self, selector))]
    pub fn locator(&self, selector: impl Into<Selector>) -> Locator {
        let selector = selector.into();
        Locator::new(self.engine.clone(), selector)
    }

    #[instrument(skip(self))]
    pub fn focused_element(&self) -> Result<UIElement, AutomationError> {
        self.engine.get_focused_element()
    }

    #[instrument(skip(self))]
    pub fn applications(&self) -> Result<Vec<UIElement>, AutomationError> {
        self.engine.get_applications()
    }

    #[instrument(skip(self, name))]
    pub fn application(&self, name: &str) -> Result<UIElement, AutomationError> {
        self.engine.get_application_by_name(name)
    }

    #[instrument(skip(self, app_name))]
    pub fn open_application(&self, app_name: &str) -> Result<UIElement, AutomationError> {
        self.engine.open_application(app_name)
    }

    #[instrument(skip(self, app_name))]
    pub fn activate_application(&self, app_name: &str) -> Result<(), AutomationError> {
        self.engine.activate_application(app_name)
    }

    #[instrument(skip(self, url, browser))]
    pub fn open_url(
        &self,
        url: &str,
        browser: Option<Browser>,
    ) -> Result<UIElement, AutomationError> {
        self.engine.open_url(url, browser)
    }

    #[instrument(skip(self, file_path))]
    pub fn open_file(&self, file_path: &str) -> Result<(), AutomationError> {
        self.engine.open_file(file_path)
    }

    #[instrument(skip(self, windows_command, unix_command))]
    pub async fn run_command(
        &self,
        windows_command: Option<&str>,
        unix_command: Option<&str>,
    ) -> Result<CommandOutput, AutomationError> {
        self.engine.run_command(windows_command, unix_command).await
    }

    // ============== NEW MONITOR ABSTRACTIONS ==============

    /// List all available monitors/displays
    ///
    /// Returns a vector of Monitor structs containing information about each display,
    /// including dimensions, position, scale factor, and whether it's the primary monitor.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use terminator::Desktop;
    /// #[tokio::main]
    /// async fn main() {
    ///     let desktop = Desktop::new_default().unwrap();
    ///     let monitors = desktop.list_monitors().await.unwrap();
    ///     for monitor in monitors {
    ///         println!("Monitor: {} ({}x{})", monitor.name, monitor.width, monitor.height);
    ///     }
    /// }
    /// ```
    #[instrument(skip(self))]
    pub async fn list_monitors(&self) -> Result<Vec<Monitor>, AutomationError> {
        self.engine.list_monitors().await
    }

    /// Get the primary monitor
    ///
    /// Returns the monitor marked as primary in the system settings.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use terminator::Desktop;
    /// #[tokio::main]
    /// async fn main() {
    ///     let desktop = Desktop::new_default().unwrap();
    ///     let primary = desktop.get_primary_monitor().await.unwrap();
    ///     println!("Primary monitor: {}", primary.name);
    /// }
    /// ```
    #[instrument(skip(self))]
    pub async fn get_primary_monitor(&self) -> Result<Monitor, AutomationError> {
        self.engine.get_primary_monitor().await
    }

    /// Get the monitor containing the currently focused window
    ///
    /// Returns the monitor that contains the currently active/focused window.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use terminator::Desktop;
    /// #[tokio::main]
    /// async fn main() {
    ///     let desktop = Desktop::new_default().unwrap();
    ///     let active = desktop.get_active_monitor().await.unwrap();
    ///     println!("Active monitor: {}", active.name);
    /// }
    /// ```
    #[instrument(skip(self))]
    pub async fn get_active_monitor(&self) -> Result<Monitor, AutomationError> {
        self.engine.get_active_monitor().await
    }

    /// Get a monitor by its ID
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use terminator::Desktop;
    /// #[tokio::main]
    /// async fn main() {
    ///     let desktop = Desktop::new_default().unwrap();
    ///     let monitor = desktop.get_monitor_by_id("monitor_id").await.unwrap();
    /// }
    /// ```
    #[instrument(skip(self, id))]
    pub async fn get_monitor_by_id(&self, id: &str) -> Result<Monitor, AutomationError> {
        self.engine.get_monitor_by_id(id).await
    }

    /// Get a monitor by its name
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use terminator::Desktop;
    /// #[tokio::main]
    /// async fn main() {
    ///     let desktop = Desktop::new_default().unwrap();
    ///     let monitor = desktop.get_monitor_by_name("Dell Monitor").await.unwrap();
    /// }
    /// ```
    #[instrument(skip(self, name))]
    pub async fn get_monitor_by_name(&self, name: &str) -> Result<Monitor, AutomationError> {
        self.engine.get_monitor_by_name(name).await
    }

    /// Capture a screenshot of a specific monitor
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use terminator::Desktop;
    /// #[tokio::main]
    /// async fn main() {
    ///     let desktop = Desktop::new_default().unwrap();
    ///     let monitor = desktop.get_primary_monitor().await.unwrap();
    ///     let screenshot = desktop.capture_monitor(&monitor).await.unwrap();
    /// }
    /// ```
    #[instrument(skip(self, monitor))]
    pub async fn capture_monitor(
        &self,
        monitor: &Monitor,
    ) -> Result<ScreenshotResult, AutomationError> {
        let mut result = self.engine.capture_monitor_by_id(&monitor.id).await?;
        result.monitor = Some(monitor.clone());
        Ok(result)
    }

    /// Capture screenshots of all monitors
    ///
    /// Returns a vector of (Monitor, ScreenshotResult) pairs for each display.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use terminator::Desktop;
    /// #[tokio::main]
    /// async fn main() {
    ///     let desktop = Desktop::new_default().unwrap();
    ///     let screenshots = desktop.capture_all_monitors().await.unwrap();
    ///     for (monitor, screenshot) in screenshots {
    ///         println!("Captured monitor: {} ({}x{})", monitor.name, screenshot.width, screenshot.height);
    ///     }
    /// }
    /// ```
    #[instrument(skip(self))]
    pub async fn capture_all_monitors(
        &self,
    ) -> Result<Vec<(Monitor, ScreenshotResult)>, AutomationError> {
        let monitors = self.list_monitors().await?;
        let mut results = Vec::new();

        for monitor in monitors {
            match self.capture_monitor(&monitor).await {
                Ok(screenshot) => results.push((monitor, screenshot)),
                Err(e) => {
                    error!("Failed to capture monitor {}: {}", monitor.name, e);
                    // Continue with other monitors rather than failing completely
                }
            }
        }

        if results.is_empty() {
            return Err(AutomationError::PlatformError(
                "Failed to capture any monitors".to_string(),
            ));
        }

        Ok(results)
    }

    // ============== DEPRECATED METHODS ==============

    // ============== END DEPRECATED METHODS ==============

    #[instrument(skip(self, image_path))]
    pub async fn ocr_image_path(&self, image_path: &str) -> Result<String, AutomationError> {
        self.engine.ocr_image_path(image_path).await
    }

    #[instrument(skip(self, screenshot))]
    pub async fn ocr_screenshot(
        &self,
        screenshot: &ScreenshotResult,
    ) -> Result<String, AutomationError> {
        self.engine.ocr_screenshot(screenshot).await
    }

    #[instrument(skip(self, title))]
    pub fn activate_browser_window_by_title(&self, title: &str) -> Result<(), AutomationError> {
        self.engine.activate_browser_window_by_title(title)
    }

    #[instrument(skip(self))]
    pub async fn get_current_browser_window(&self) -> Result<UIElement, AutomationError> {
        self.engine.get_current_browser_window().await
    }

    #[instrument(skip(self))]
    pub async fn get_current_window(&self) -> Result<UIElement, AutomationError> {
        self.engine.get_current_window().await
    }

    #[instrument(skip(self))]
    pub async fn get_current_application(&self) -> Result<UIElement, AutomationError> {
        self.engine.get_current_application().await
    }

    #[instrument(skip(self, pid, title, config))]
    pub fn get_window_tree(
        &self,
        pid: u32,
        title: Option<&str>,
        config: Option<crate::platforms::TreeBuildConfig>,
    ) -> Result<UINode, AutomationError> {
        let tree_config = config.unwrap_or_default();
        self.engine.get_window_tree(pid, title, tree_config)
    }

    /// Get the UI tree for all open applications in parallel.
    ///
    /// This function retrieves the UI hierarchy for every running application
    /// on the desktop. It processes applications in parallel for better performance.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use terminator::Desktop;
    /// #[tokio::main]
    /// async fn main() {
    ///     let desktop = Desktop::new_default().unwrap();
    ///     let app_trees = desktop.get_all_applications_tree().await.unwrap();
    ///     for tree in app_trees {
    ///         println!("Application Tree: {:#?}", tree);
    ///     }
    /// }
    /// ```
    #[instrument(skip(self))]
    pub async fn get_all_applications_tree(&self) -> Result<Vec<UINode>, AutomationError> {
        let applications = self.applications()?;

        let futures = applications.into_iter().map(|app| {
            let desktop = self.clone();
            tokio::task::spawn_blocking(move || {
                let pid = match app.process_id() {
                    Ok(pid) if pid > 0 => pid,
                    _ => return None, // Skip apps with invalid or zero/negative PIDs
                };

                // TODO: tbh not sure it cannot lead to crash to run this in threads on windows :)
                match desktop.get_window_tree(pid, None, None) {
                    Ok(tree) => {
                        if !tree.children.is_empty() || tree.attributes.name.is_some() {
                            Some(tree)
                        } else {
                            None
                        }
                    }
                    Err(e) => {
                        let app_name = app.name().unwrap_or_else(|| "Unknown".to_string());
                        tracing::warn!(
                            "Could not get window tree for app '{}' (PID: {}): {}",
                            app_name,
                            pid,
                            e
                        );
                        None
                    }
                }
            })
        });

        let results = futures::future::join_all(futures).await;

        let trees: Vec<UINode> = results
            .into_iter()
            .filter_map(|res| match res {
                Ok(Some(tree)) => Some(tree),
                Ok(None) => None,
                Err(e) => {
                    error!("A task for getting a window tree panicked: {}", e);
                    None
                }
            })
            .collect();

        Ok(trees)
    }

    /// Get all window elements for a given application by name
    #[instrument(skip(self, app_name))]
    pub async fn windows_for_application(
        &self,
        app_name: &str,
    ) -> Result<Vec<UIElement>, AutomationError> {
        // 1. Find the application element
        let app_element = match self.application(app_name) {
            Ok(app) => app,
            Err(e) => {
                error!("Application '{}' not found: {}", app_name, e);
                return Err(e);
            }
        };

        // 2. Get children of the application element
        let children = match app_element.children() {
            Ok(ch) => ch,
            Err(e) => {
                error!(
                    "Failed to get children for application '{}': {}",
                    app_name, e
                );
                return Err(e);
            }
        };

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

        debug!(
            window_count = windows.len(),
            "Found windows for application '{}'", app_name
        );

        Ok(windows)
    }

    pub async fn press_key(&self, key: &str) -> Result<(), AutomationError> {
        self.engine.press_key(key)
    }

    pub async fn zoom_in(&self, level: u32) -> Result<(), AutomationError> {
        self.engine.zoom_in(level)
    }

    pub async fn zoom_out(&self, level: u32) -> Result<(), AutomationError> {
        self.engine.zoom_out(level)
    }

    /// Sets the zoom level to a specific percentage
    ///
    /// # Arguments
    /// * `percentage` - The zoom percentage (e.g., 100 for 100%, 150 for 150%, 50 for 50%)
    ///
    /// # Examples
    /// ```no_run
    /// use terminator::Desktop;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let desktop = Desktop::new_default().unwrap();
    ///     // Set zoom to 150%
    ///     desktop.set_zoom(150).await.unwrap();
    ///
    ///     // Reset zoom to 100%
    ///     desktop.set_zoom(100).await.unwrap();
    /// }
    /// ```
    pub async fn set_zoom(&self, percentage: u32) -> Result<(), AutomationError> {
        self.engine.set_zoom(percentage)
    }
}

impl Clone for Desktop {
    fn clone(&self) -> Self {
        Self {
            engine: self.engine.clone(),
        }
    }
}
