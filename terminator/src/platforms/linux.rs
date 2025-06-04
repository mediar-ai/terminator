use crate::element::UIElementImpl;
use crate::platforms::AccessibilityEngine;
use crate::{ClickResult, CommandOutput, ScreenshotResult};
use crate::{AutomationError, Locator, Selector, UIElement, UIElementAttributes};
use std::fmt::Debug;
use std::time::Duration;
use std::default::Default;

pub struct LinuxEngine;

impl LinuxEngine {
    pub fn new(use_background_apps: bool, activate_app: bool) -> Result<Self, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }
}

#[async_trait::async_trait]
impl AccessibilityEngine for LinuxEngine {
    fn get_root_element(&self) -> UIElement {
        panic!("Linux implementation is not yet available")
    }

    fn get_focused_element(&self) -> Result<UIElement, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn get_applications(&self) -> Result<Vec<UIElement>, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn get_application_by_name(&self, _name: &str) -> Result<UIElement, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn get_application_by_pid(&self, _pid: i32, _timeout: Option<Duration>) -> Result<UIElement, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn find_element(
        &self,
        selector: &Selector,
        root: Option<&UIElement>,
    ) -> Result<UIElement, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn find_elements(
        &self,
        _selector: &Selector,
        _root: Option<&UIElement>,
    ) -> Result<Vec<UIElement>, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn open_application(&self, _app_name: &str) -> Result<UIElement, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn open_url(&self, _url: &str, _browser: Option<&str>) -> Result<UIElement, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn open_file(&self, _file_path: &str) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn run_command(
        &self,
        _windows_command: Option<&str>,
        _unix_command: Option<&str>,
    ) -> Result<crate::CommandOutput, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn capture_screen(&self) -> Result<ScreenshotResult, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn capture_monitor_by_name(&self, _name: &str) -> Result<ScreenshotResult, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    async fn ocr_image_path(&self, _image_path: &str) -> Result<String, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    async fn ocr_screenshot(&self, _screenshot: &ScreenshotResult) -> Result<String, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    async fn find_window_by_criteria(
        &self,
        title_contains: Option<&str>,
        timeout: Option<Duration>,
    ) -> Result<UIElement, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "find_window_by_criteria not yet implemented for Linux".to_string(),
        ))
    }

    async fn get_current_browser_window(&self) -> Result<UIElement, AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "get_current_browser_window not yet implemented for Linux".to_string(),
        ))
    }

    async fn get_current_window(&self) -> Result<UIElement, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "get_current_window not yet implemented for Linux".to_string(),
        ))
    }

    async fn get_current_application(&self) -> Result<UIElement, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "get_current_application not yet implemented for Linux".to_string(),
        ))
    }

    fn get_window_tree_by_title(&self, title: &str) -> Result<crate::UINode, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            format!("get_window_tree_by_title for '{}' not yet implemented for Linux", title)
        ))
    }

    async fn get_active_monitor_name(&self) -> Result<String, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn get_element_by_id(&self, _id: i32) -> Result<UIElement, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn find_element(
        &self,
        _selector: &Selector,
        _root: Option<&UIElement>,
        _timeout: Option<Duration>,
    ) -> Result<UIElement, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn find_elements(
        &self,
        _selector: &Selector,
        _root: Option<&UIElement>,
        _timeout: Option<Duration>,
        _depth: Option<usize>,
    ) -> Result<Vec<UIElement>, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn get_window_tree_by_pid_and_title(&self, _pid: u32, _title: Option<&str>) -> Result<crate::UINode, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn activate_browser_window_by_title(&self, _title: &str) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn activate_application(&self, _app_name: &str) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    async fn run_command(
        &self,
        _windows_command: Option<&str>,
        _unix_command: Option<&str>,
    ) -> Result<crate::CommandOutput, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    async fn capture_screen(&self) -> Result<ScreenshotResult, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    async fn capture_monitor_by_name(&self, _name: &str) -> Result<ScreenshotResult, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn enable_background_cache_warmer(
        &self,
        _enable: bool,
        _interval_seconds: Option<u64>,
        _max_apps_to_cache: Option<usize>,
    ) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "Background cache warming is not yet implemented for Linux".to_string()
        ))
    }

    fn is_cache_warmer_enabled(&self) -> bool {
        false
    }
}

// Placeholder LinuxUIElement that implements UIElementImpl
pub struct LinuxUIElement;

impl Debug for LinuxUIElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinuxUIElement").finish()
    }
}

impl UIElementImpl for LinuxUIElement {
    fn object_id(&self) -> usize {
        0
    }

    fn id(&self) -> Option<String> {
        None
    }

    fn role(&self) -> String {
        "".to_string()
    }

    fn attributes(&self) -> UIElementAttributes {
        UIElementAttributes {
            role: "".to_string(),
            ..Default::default()
        }
    }

    fn name(&self) -> Option<String> {
        None
    }

    fn children(&self) -> Result<Vec<UIElement>, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn parent(&self) -> Result<Option<UIElement>, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn bounds(&self) -> Result<(f64, f64, f64, f64), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn click(&self) -> Result<ClickResult, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn double_click(&self) -> Result<ClickResult, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn right_click(&self) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn hover(&self) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn focus(&self) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn type_text(&self, _text: &str, _use_clipboard: bool) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn press_key(&self, _key: &str) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn get_text(&self, max_depth: usize) -> Result<String, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn set_value(&self, _value: &str) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn is_enabled(&self) -> Result<bool, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn is_visible(&self) -> Result<bool, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn is_focused(&self) -> Result<bool, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn perform_action(&self, _action: &str) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn create_locator(&self, _selector: Selector) -> Result<Locator, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn clone_box(&self) -> Box<dyn UIElementImpl> {
        Box::new(LinuxUIElement)
    }

    fn scroll(&self, _direction: &str, _amount: f64) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn application(&self) -> Result<Option<UIElement>, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn window(&self) -> Result<Option<UIElement>, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn highlight(&self, color: Option<u32>, duration: Option<std::time::Duration>) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn capture(&self) -> Result<ScreenshotResult, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }
}
