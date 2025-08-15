use crate::{AutomationError, Browser, Selector, UIElement, UINode};
use std::sync::Arc;
use std::time::Duration;

/// Configuration for tree building performance and completeness
#[derive(Debug, Clone)]
pub struct TreeBuildConfig {
    /// Property loading strategy
    pub property_mode: PropertyLoadingMode,
    /// Optional timeout per operation in milliseconds
    pub timeout_per_operation_ms: Option<u64>,
    /// Optional yield frequency for responsiveness  
    pub yield_every_n_elements: Option<usize>,
    /// Optional batch size for processing elements
    pub batch_size: Option<usize>,
}

/// Defines how much element property data to load
#[derive(Debug, Clone)]
pub enum PropertyLoadingMode {
    /// Only load essential properties (role + name) - fastest
    Fast,
    /// Load all properties for complete element data - slower but comprehensive  
    Complete,
    /// Load specific properties based on element type - balanced approach
    Smart,
}

impl Default for TreeBuildConfig {
    fn default() -> Self {
        Self {
            property_mode: PropertyLoadingMode::Fast,
            timeout_per_operation_ms: Some(50),
            yield_every_n_elements: Some(50),
            batch_size: Some(50),
        }
    }
}

/// The common trait that all platform-specific engines must implement
#[async_trait::async_trait]
pub trait AccessibilityEngine: Send + Sync {
    /// Get the root UI element
    fn get_root_element(&self) -> UIElement;

    fn get_element_by_id(&self, id: i32) -> Result<UIElement, AutomationError>;

    /// Get the currently focused element
    fn get_focused_element(&self) -> Result<UIElement, AutomationError>;

    /// Get all running applications
    fn get_applications(&self) -> Result<Vec<UIElement>, AutomationError>;

    /// Get application by name
    fn get_application_by_name(&self, name: &str) -> Result<UIElement, AutomationError>;

    /// Get application by process ID
    fn get_application_by_pid(
        &self,
        pid: i32,
        timeout: Option<Duration>,
    ) -> Result<UIElement, AutomationError>;

    /// Find elements using a selector
    fn find_element(
        &self,
        selector: &Selector,
        root: Option<&UIElement>,
        timeout: Option<Duration>,
    ) -> Result<UIElement, AutomationError>;

    /// Find all elements matching a selector
    /// Default implementation returns an UnsupportedOperation error,
    /// allowing platform-specific implementations to override as needed
    fn find_elements(
        &self,
        selector: &Selector,
        root: Option<&UIElement>,
        timeout: Option<Duration>,
        depth: Option<usize>,
    ) -> Result<Vec<UIElement>, AutomationError>;

    /// Open an application by name
    fn open_application(&self, app_name: &str) -> Result<UIElement, AutomationError>;

    /// Activate an application by name
    fn activate_application(&self, app_name: &str) -> Result<(), AutomationError>;

    /// Open a URL in a specified browser (or default if None)
    fn open_url(&self, url: &str, browser: Option<Browser>) -> Result<UIElement, AutomationError>;

    /// Open a file
    fn open_file(&self, file_path: &str) -> Result<(), AutomationError>;

    /// Run a command
    async fn run_command(
        &self,
        windows_command: Option<&str>,
        unix_command: Option<&str>,
    ) -> Result<crate::CommandOutput, AutomationError>;

    // ============== NEW MONITOR ABSTRACTIONS ==============

    /// List all available monitors/displays
    async fn list_monitors(&self) -> Result<Vec<crate::Monitor>, AutomationError>;

    /// Get the primary monitor
    async fn get_primary_monitor(&self) -> Result<crate::Monitor, AutomationError>;

    /// Get the monitor containing the currently focused window
    async fn get_active_monitor(&self) -> Result<crate::Monitor, AutomationError>;

    /// Get a monitor by its ID
    async fn get_monitor_by_id(&self, id: &str) -> Result<crate::Monitor, AutomationError>;

    /// Get a monitor by its name
    async fn get_monitor_by_name(&self, name: &str) -> Result<crate::Monitor, AutomationError>;

    /// Capture a screenshot of a monitor by its ID
    async fn capture_monitor_by_id(
        &self,
        id: &str,
    ) -> Result<crate::ScreenshotResult, AutomationError>;

    // ============== DEPRECATED METHODS ==============

    /// Capture screenshot (deprecated - use monitor-specific methods)
    #[deprecated(
        since = "0.4.9",
        note = "Use get_primary_monitor() and capture_monitor_by_id() instead"
    )]
    async fn capture_screen(&self) -> Result<crate::ScreenshotResult, AutomationError> {
        let primary = self.get_primary_monitor().await?;
        self.capture_monitor_by_id(&primary.id).await
    }

    /// Capture screenshot by monitor name (deprecated)
    #[deprecated(
        since = "0.4.9",
        note = "Use get_monitor_by_name() and capture_monitor_by_id() instead"
    )]
    async fn capture_monitor_by_name(
        &self,
        name: &str,
    ) -> Result<crate::ScreenshotResult, AutomationError> {
        let monitor = self.get_monitor_by_name(name).await?;
        self.capture_monitor_by_id(&monitor.id).await
    }

    /// Get the name of the currently active monitor (deprecated)
    #[deprecated(since = "0.4.9", note = "Use get_active_monitor() instead")]
    async fn get_active_monitor_name(&self) -> Result<String, AutomationError> {
        let monitor = self.get_active_monitor().await?;
        Ok(monitor.name)
    }

    // ============== END DEPRECATED METHODS ==============

    /// OCR on image path
    async fn ocr_image_path(&self, image_path: &str) -> Result<String, AutomationError>;

    /// OCR on screenshot
    async fn ocr_screenshot(
        &self,
        screenshot: &crate::ScreenshotResult,
    ) -> Result<String, AutomationError>;

    /// Activate browser window
    fn activate_browser_window_by_title(&self, title: &str) -> Result<(), AutomationError>;

    /// Get current browser window
    async fn get_current_browser_window(&self) -> Result<UIElement, AutomationError>;

    /// Get current window
    async fn get_current_window(&self) -> Result<UIElement, AutomationError>;

    /// Get current application
    async fn get_current_application(&self) -> Result<UIElement, AutomationError>;

    fn press_key(&self, key: &str) -> Result<(), AutomationError>;
    fn zoom_in(&self, level: u32) -> Result<(), AutomationError>;
    fn zoom_out(&self, level: u32) -> Result<(), AutomationError>;
    /// Sets the zoom level to a specific percentage (e.g., 100 for 100%, 150 for 150%)
    fn set_zoom(&self, percentage: u32) -> Result<(), AutomationError>;

    /// Get the complete UI tree for a window identified by process ID and optional title
    /// This is the single tree building function - replaces get_window_tree_by_title and get_window_tree_by_pid_and_title
    ///
    /// # Arguments
    /// * `pid` - Process ID of the target application
    /// * `title` - Optional window title filter (if None, uses any window from the PID)
    /// * `config` - Configuration for tree building performance and completeness
    ///
    /// # Returns
    /// Complete UI tree starting from the identified window
    fn get_window_tree(
        &self,
        pid: u32,
        title: Option<&str>,
        config: TreeBuildConfig,
    ) -> Result<UINode, AutomationError>;

    /// Enable downcasting to concrete engine types
    fn as_any(&self) -> &dyn std::any::Any;
}

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "macos")]
pub mod tree_search;
#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(all(target_os = "windows", test))]
pub mod windows_tests;

#[cfg(target_os = "windows")]
#[cfg(test)]
pub mod windows_benchmarks;

/// Create the appropriate engine for the current platform
pub fn create_engine(
    use_background_apps: bool,
    activate_app: bool,
) -> Result<Arc<dyn AccessibilityEngine>, AutomationError> {
    #[cfg(target_os = "macos")]
    {
        Ok(Arc::new(macos::MacOSEngine::new(
            use_background_apps,
            activate_app,
        )?))
    }
    #[cfg(target_os = "windows")]
    {
        Ok(Arc::new(windows::WindowsEngine::new(
            use_background_apps,
            activate_app,
        )?))
    }
    #[cfg(target_os = "linux")]
    {
        Ok(Arc::new(linux::LinuxEngine::new(
            use_background_apps,
            activate_app,
        )?))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Err(AutomationError::UnsupportedPlatform(
            "Current platform is not supported".to_string(),
        ))
    }
}
