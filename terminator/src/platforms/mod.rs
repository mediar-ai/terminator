use crate::{AutomationError, Selector, UIElement, UINode};
use std::sync::Arc;
use std::time::Duration;

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
    fn get_application_by_pid(&self, pid: i32, timeout: Option<Duration>) -> Result<UIElement, AutomationError>;

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
    fn open_url(&self, url: &str, browser: Option<&str>) -> Result<UIElement, AutomationError>;

    /// Open a file with its default application
    fn open_file(&self, file_path: &str) -> Result<(), AutomationError>;

    /// Execute a terminal command, choosing the appropriate command based on the OS.
    async fn run_command(
        &self,
        windows_command: Option<&str>,
        unix_command: Option<&str>,
    ) -> Result<crate::CommandOutput, AutomationError>;

    /// Capture a screenshot of the primary monitor
    async fn capture_screen(&self) -> Result<crate::ScreenshotResult, AutomationError>;

    /// Capture a screenshot of a specific monitor by name
    async fn capture_monitor_by_name(&self, name: &str) -> Result<crate::ScreenshotResult, AutomationError>;

    /// Perform OCR on the provided image file (requires async runtime)
    async fn ocr_image_path(&self, image_path: &str) -> Result<String, AutomationError>;

    /// Perform OCR on the provided screenshot data (requires async runtime)
    async fn ocr_screenshot(&self, screenshot: &crate::ScreenshotResult) -> Result<String, AutomationError>;

    /// Activate a browser window containing a specific title
    fn activate_browser_window_by_title(&self, title: &str) -> Result<(), AutomationError>;

    /// Find a window by criteria
    async fn find_window_by_criteria(
        &self,
        title_contains: Option<&str>,
        timeout: Option<Duration>,
    ) -> Result<UIElement, AutomationError>;

    /// Get the currently focused browser window (async)
    async fn get_current_browser_window(&self) -> Result<UIElement, AutomationError>;

    /// Get the currently focused window
    async fn get_current_window(&self) -> Result<UIElement, AutomationError>;

    /// Get the currently focused application
    async fn get_current_application(&self) -> Result<UIElement, AutomationError>;

    /// Get the UI tree for a window by its title
    fn get_window_tree_by_title(&self, title: &str) -> Result<UINode, AutomationError>;

    /// Get the UI tree for a window by process ID and optional title
    /// If title is provided and matches, use that window
    /// If title is provided but no match found, fall back to any window from the process ID
    /// If title is None, use any window from the process ID
    fn get_window_tree_by_pid_and_title(&self, pid: u32, title: Option<&str>) -> Result<UINode, AutomationError>;

    /// Get the name of the currently active monitor
    async fn get_active_monitor_name(&self) -> Result<String, AutomationError>;

    /// Enable downcasting to concrete engine types
    fn as_any(&self) -> &dyn std::any::Any;

    /// Enable or disable background cache warming for improved performance
    /// 
    /// This spawns a background thread that periodically fetches UI trees for frequently used applications
    /// to keep the platform's native cache warm, improving performance when applications need to be queried.
    /// 
    /// Default implementation returns UnsupportedOperation - platforms should override as needed.
    fn enable_background_cache_warmer(
        &self,
        enable: bool,
        interval_seconds: Option<u64>,
        max_apps_to_cache: Option<usize>,
    ) -> Result<(), AutomationError>;

    /// Check if the background cache warmer is currently running
    /// 
    /// Default implementation returns false - platforms should override as needed.
    fn is_cache_warmer_enabled(&self) -> bool;
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
        Err(AutomationError::UnsupportedPlatform("Linux platform not fully implemented in create_engine".to_string()))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Err(AutomationError::UnsupportedPlatform(
            "Current platform is not supported".to_string(),
        ))
    }
}
