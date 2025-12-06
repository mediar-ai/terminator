//! Desktop UI automation through accessibility APIs
//!
//! This module provides a cross-platform API for automating desktop applications
//! through accessibility APIs, inspired by Playwright's web automation model.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use tracing::{debug, error, info, instrument};

pub mod browser_script;
pub mod element;
pub mod errors;
pub mod extension_bridge;
pub mod health;
pub mod locator;
pub mod platforms;
pub mod screenshot;
pub mod selector;
#[cfg(test)]
mod tests;
pub mod tree_formatter;
pub mod types;
pub mod ui_tree_diff;
pub mod utils;

#[cfg(target_os = "windows")]
pub mod computer_use;

pub use element::{OcrElement, SerializableUIElement, UIElement, UIElementAttributes};
pub use errors::AutomationError;
pub use locator::Locator;
pub use screenshot::ScreenshotResult;
pub use selector::Selector;
pub use tokio_util::sync::CancellationToken;
pub use tree_formatter::{
    format_clustered_tree_from_caches, format_ocr_tree_as_compact_yaml, format_tree_as_compact_yaml,
    format_ui_node_as_compact_yaml, ClusteredFormattingResult, ElementSource, OcrFormattingResult,
    TreeFormattingResult, UnifiedElement,
};
pub use types::{FontStyle, HighlightHandle, OmniparserItem, TextPosition, VisionElement};

// Re-export types from terminator-computer-use crate
#[cfg(target_os = "windows")]
pub use terminator_computer_use::{
    call_computer_use_backend, convert_normalized_to_screen, translate_gemini_keys,
    ComputerUseActionResponse, ComputerUseFunctionCall, ComputerUsePreviousAction,
    ComputerUseResponse, ComputerUseResult, ComputerUseStep, ProgressCallback,
};

// Re-export cross-platform types from platforms
pub use platforms::{OverlayDisplayMode, PropertyLoadingMode, TreeBuildConfig};

// Re-export window manager types (Windows only)
#[cfg(target_os = "windows")]
pub use platforms::windows::window_manager::{
    WindowCache, WindowInfo, WindowManager, WindowPlacement,
};

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

/// Type of mouse click to perform
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClickType {
    /// Single left click (default)
    Left,
    /// Double left click
    Double,
    /// Single right click
    Right,
}

#[cfg(target_os = "windows")]
pub use platforms::windows::{
    convert_uiautomation_element_to_terminator, get_process_name_by_pid, hide_inspect_overlay,
    is_browser_process, set_recording_mode, show_inspect_overlay, stop_all_highlights,
    InspectElement, InspectOverlayHandle, KNOWN_BROWSER_PROCESS_NAMES,
};

// Define a new struct to hold click result information - move to module level
pub struct ClickResult {
    pub method: String,
    pub coordinates: Option<(f64, f64)>,
    pub details: String,
}

/// Generic result struct for UI actions with state tracking
pub struct ActionResult {
    pub action: String,
    pub details: String,
    pub data: Option<serde_json::Value>,
}

/// Holds the output of a terminal command execution
pub struct CommandOutput {
    pub exit_status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Result of get_window_tree operation with all computed data
///
/// This struct provides everything needed for UI automation:
/// - Raw UINode tree for programmatic traversal
/// - Formatted output for LLM consumption
/// - Index-to-bounds mapping for click targeting
/// - Metadata about the window/process
#[derive(Debug, Clone)]
pub struct WindowTreeResult {
    /// The raw UI tree structure
    pub tree: UINode,
    /// Process ID of the window
    pub pid: u32,
    /// Whether this is a browser window
    pub is_browser: bool,
    /// Formatted compact YAML output (if format_output was true)
    pub formatted: Option<String>,
    /// Mapping of index to (role, name, bounds, selector) for click targeting
    /// Key is 1-based index, value is (role, name, (x, y, width, height), selector)
    pub index_to_bounds: std::collections::HashMap<u32, (String, String, (f64, f64, f64, f64), Option<String>)>,
    /// Total count of indexed elements (elements with bounds)
    pub element_count: u32,
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
    /// Work area dimensions (screen area excluding taskbar) - Windows only
    /// On other platforms, this will be the same as the full monitor dimensions
    pub work_area: Option<WorkAreaBounds>,
}

/// Represents the work area bounds (excluding taskbar and docked windows)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct WorkAreaBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
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
    /// Chained selector path from root to this node (e.g., "role:Window && name:App >> role:Button && name:Submit")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
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

// Removed struct ScreenshotResult (moved to screenshot.rs)

/// The main entry point for UI automation
pub struct Desktop {
    engine: Arc<dyn platforms::AccessibilityEngine>,
    /// Cancellation token for stopping execution
    cancellation_token: CancellationToken,
}

impl Desktop {
    #[instrument(skip(use_background_apps, activate_app))]
    pub fn new(use_background_apps: bool, activate_app: bool) -> Result<Self, AutomationError> {
        let engine = platforms::create_engine(use_background_apps, activate_app)?;
        Ok(Self {
            engine,
            cancellation_token: CancellationToken::new(),
        })
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

    /// Execute a shell command using GitHub Actions-style syntax
    ///
    /// # Arguments
    /// * `command` - The command to run (can be single or multi-line)
    /// * `shell` - Optional shell to use (defaults to PowerShell on Windows, bash on Unix)
    /// * `working_directory` - Optional working directory for the command
    ///
    /// # Examples
    /// ```no_run
    /// use terminator::Desktop;
    /// #[tokio::main]
    /// async fn main() {
    ///     let desktop = Desktop::new_default().unwrap();
    ///     let output = desktop.run(
    ///         "echo 'Hello, World!'",
    ///         None,
    ///         None
    ///     ).await.unwrap();
    ///     println!("Output: {}", output.stdout);
    /// }
    /// ```
    #[instrument(skip(self, command))]
    pub async fn run(
        &self,
        command: &str,
        shell: Option<&str>,
        working_directory: Option<&str>,
    ) -> Result<CommandOutput, AutomationError> {
        // Determine which shell to use based on platform and user preference
        let (windows_cmd, unix_cmd) = if cfg!(target_os = "windows") {
            let shell = shell.unwrap_or("powershell");
            let command_with_cd = if let Some(cwd) = working_directory {
                match shell {
                    "cmd" => format!("cd /d \"{cwd}\" && {command}"),
                    "powershell" | "pwsh" => format!("cd '{cwd}'; {command}"),
                    _ => command.to_string(),
                }
            } else {
                command.to_string()
            };

            let windows_cmd = match shell {
                "bash" => format!("bash -c \"{}\"", command_with_cd.replace('\"', "\\\"")),
                "sh" => format!("sh -c \"{}\"", command_with_cd.replace('\"', "\\\"")),
                "cmd" => format!("cmd /c \"{command_with_cd}\""),
                "powershell" | "pwsh" => command_with_cd,
                _ => command_with_cd,
            };
            (Some(windows_cmd), None)
        } else {
            let shell = shell.unwrap_or("bash");
            let command_with_cd = if let Some(cwd) = working_directory {
                format!("cd '{cwd}' && {command}")
            } else {
                command.to_string()
            };

            let unix_cmd = match shell {
                "python" => format!("python -c \"{}\"", command_with_cd.replace('\"', "\\\"")),
                "node" => format!("node -e \"{}\"", command_with_cd.replace('\"', "\\\"")),
                _ => command_with_cd,
            };
            (None, Some(unix_cmd))
        };

        self.engine
            .run_command(windows_cmd.as_deref(), unix_cmd.as_deref())
            .await
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

    /// OCR on screenshot with bounding boxes - returns structured OCR elements with absolute screen coordinates
    /// Window coordinates are used to convert OCR bounding boxes to absolute screen positions
    ///
    /// # Arguments
    /// * `screenshot` - The screenshot to perform OCR on
    /// * `window_x` - X offset of the window on screen in logical coordinates
    /// * `window_y` - Y offset of the window on screen in logical coordinates
    /// * `dpi_scale_x` - DPI scale factor for X (screenshot_width / window_logical_width)
    /// * `dpi_scale_y` - DPI scale factor for Y (screenshot_height / window_logical_height)
    #[instrument(skip(self, screenshot))]
    pub fn ocr_screenshot_with_bounds(
        &self,
        screenshot: &ScreenshotResult,
        window_x: f64,
        window_y: f64,
        dpi_scale_x: f64,
        dpi_scale_y: f64,
    ) -> Result<OcrElement, AutomationError> {
        self.engine.ocr_screenshot_with_bounds(
            screenshot,
            window_x,
            window_y,
            dpi_scale_x,
            dpi_scale_y,
        )
    }

    /// Click at absolute screen coordinates
    /// This is useful for clicking on OCR-detected text elements
    #[instrument(skip(self))]
    pub fn click_at_coordinates(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        self.engine.click_at_coordinates(x, y)
    }

    /// Click at absolute screen coordinates with specified click type (left, double, right)
    /// This is useful for clicking on OCR-detected text elements with different click types
    #[instrument(skip(self))]
    pub fn click_at_coordinates_with_type(
        &self,
        x: f64,
        y: f64,
        click_type: ClickType,
    ) -> Result<(), AutomationError> {
        self.engine.click_at_coordinates_with_type(x, y, click_type)
    }

    #[instrument(skip(self, title))]
    pub fn activate_browser_window_by_title(&self, title: &str) -> Result<(), AutomationError> {
        self.engine.activate_browser_window_by_title(title)
    }

    #[instrument(skip(self))]
    pub async fn get_current_browser_window(&self) -> Result<UIElement, AutomationError> {
        self.engine.get_current_browser_window().await
    }

    /// Execute JavaScript in the currently focused browser tab.
    /// Automatically finds the active browser window and executes the script.
    ///
    /// This method respects cancellation - if `stop_execution()` is called,
    /// the operation will be interrupted and return an error.
    #[instrument(skip(self, script))]
    pub async fn execute_browser_script(&self, script: &str) -> Result<String, AutomationError> {
        let browser_window = self.engine.get_current_browser_window().await?;
        tokio::select! {
            result = browser_window.execute_browser_script(script) => result,
            _ = self.cancellation_token.cancelled() => {
                Err(AutomationError::OperationCancelled("Browser script execution cancelled by stop_execution".into()))
            }
        }
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

    /// Get the UI tree with full result including formatting and bounds mapping
    ///
    /// This is the recommended method for getting window trees when you need:
    /// - Formatted YAML output for LLM consumption
    /// - Index-to-bounds mapping for click targeting
    /// - Browser detection
    ///
    /// # Arguments
    /// * `pid` - Process ID of the target application
    /// * `title` - Optional window title filter
    /// * `config` - Tree building configuration (format_output controls formatted output)
    ///
    /// # Returns
    /// `WindowTreeResult` containing the tree, formatted output, and bounds mapping
    #[instrument(skip(self, pid, title, config))]
    pub fn get_window_tree_result(
        &self,
        pid: u32,
        title: Option<&str>,
        config: Option<crate::platforms::TreeBuildConfig>,
    ) -> Result<WindowTreeResult, AutomationError> {
        let tree_config = config.unwrap_or_default();
        let format_output = tree_config.format_output;

        // Get the raw tree
        let tree = self.engine.get_window_tree(pid, title, tree_config)?;

        // Check if browser process
        #[cfg(target_os = "windows")]
        let is_browser = is_browser_process(pid);
        #[cfg(not(target_os = "windows"))]
        let is_browser = false;

        // Format the tree and get bounds mapping if requested
        let (formatted, index_to_bounds, element_count) = if format_output {
            let result = format_ui_node_as_compact_yaml(&tree, 0);
            (
                Some(result.formatted),
                result.index_to_bounds,
                result.element_count,
            )
        } else {
            (None, std::collections::HashMap::new(), 0)
        };

        Ok(WindowTreeResult {
            tree,
            pid,
            is_browser,
            formatted,
            index_to_bounds,
            element_count,
        })
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
    /// This method respects cancellation - if `stop_execution()` is called,
    /// the operation will be interrupted and return an error.
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

        // Use select to allow cancellation while waiting for all futures
        tokio::select! {
            results = futures::future::join_all(futures) => {
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
            _ = self.cancellation_token.cancelled() => {
                Err(AutomationError::OperationCancelled("get_all_applications_tree cancelled by stop_execution".into()))
            }
        }
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

    /// Delay execution for a specified number of milliseconds.
    /// Useful for waiting between actions to ensure UI stability.
    ///
    /// This method respects cancellation - if `stop_execution()` is called,
    /// the delay will be interrupted and return an error.
    pub async fn delay(&self, delay_ms: u64) -> Result<(), AutomationError> {
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_millis(delay_ms)) => Ok(()),
            _ = self.cancellation_token.cancelled() => {
                Err(AutomationError::OperationCancelled("Delay cancelled by stop_execution".into()))
            }
        }
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

    /// Stop all currently executing operations.
    ///
    /// This cancels the internal cancellation token, which will cause any
    /// operations that check `is_cancelled()` to abort. After calling this,
    /// you should create a new Desktop instance to start fresh.
    ///
    /// # Examples
    /// ```no_run
    /// use terminator::Desktop;
    ///
    /// let desktop = Desktop::new_default().unwrap();
    /// // ... start some operations ...
    /// desktop.stop_execution();
    /// ```
    pub fn stop_execution(&self) {
        info!("🛑 Stop execution requested - cancelling all operations");
        self.cancellation_token.cancel();
    }

    /// Check if execution has been cancelled.
    ///
    /// Returns `true` if `stop_execution()` has been called.
    /// Long-running operations should periodically check this and abort if true.
    ///
    /// # Examples
    /// ```no_run
    /// use terminator::Desktop;
    ///
    /// let desktop = Desktop::new_default().unwrap();
    /// if desktop.is_cancelled() {
    ///     println!("Execution was cancelled");
    /// }
    /// ```
    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }

    /// Get a clone of the cancellation token for use in async operations.
    ///
    /// This allows external code to wait on cancellation or create child tokens.
    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation_token.clone()
    }
}

impl Clone for Desktop {
    fn clone(&self) -> Self {
        Self {
            engine: self.engine.clone(),
            // Clone shares the same cancellation token so stop_execution affects all clones
            cancellation_token: self.cancellation_token.clone(),
        }
    }
}
