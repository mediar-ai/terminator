//! Windows UI Element implementation

use super::types::{FontStyle, HighlightHandle, TextPosition, ThreadSafeWinUIElement};
use super::utils::{create_ui_automation_with_com_init, generate_element_id};
use crate::element::UIElementImpl;
use crate::platforms::windows::applications::get_application_by_pid;
use crate::platforms::windows::{highlighting, WindowsEngine};
use crate::{
    AutomationError, ClickResult, Locator, ScreenshotResult, Selector, UIElement,
    UIElementAttributes,
};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};
use uiautomation::controls::ControlType;
use uiautomation::inputs::Mouse;
use uiautomation::patterns;
use uiautomation::types::{Point, TreeScope, UIProperty};
use uiautomation::variants::Variant;
use uiautomation::UIAutomation;

trait ScrollFallback {
    fn scroll_with_fallback(&self, direction: &str, amount: f64) -> Result<(), AutomationError>;
}

impl ScrollFallback for WindowsUIElement {
    fn scroll_with_fallback(&self, direction: &str, amount: f64) -> Result<(), AutomationError> {
        warn!(
            "Using key-press scroll fallback for element: {:?}",
            self.element.0.get_name().unwrap_or_default()
        );
        self.focus().map_err(|e| {
            AutomationError::PlatformError(format!(
                "Failed to focus element for scroll fallback: {e:?}"
            ))
        })?;

        match direction {
            "up" | "down" => {
                let times = amount.abs().round().max(1.0) as usize;
                let key = if direction == "up" {
                    "{page_up}"
                } else {
                    "{page_down}"
                };
                for _ in 0..times {
                    self.press_key(key)?;
                }
            }
            "left" | "right" => {
                let times = amount.abs().round().max(1.0) as usize;
                let key = if direction == "left" {
                    "{left}"
                } else {
                    "{right}"
                };
                for _ in 0..times {
                    self.press_key(key)?;
                }
            }
            _ => {
                return Err(AutomationError::UnsupportedOperation(
                    "Supported scroll directions: 'up', 'down', 'left', 'right'".to_string(),
                ));
            }
        }
        Ok(())
    }
}

const DEFAULT_FIND_TIMEOUT: Duration = Duration::from_millis(5000);

pub struct WindowsUIElement {
    pub(crate) element: ThreadSafeWinUIElement,
}

impl WindowsUIElement {
    /// Get the raw UI element for direct automation
    pub fn get_raw_element(&self) -> &uiautomation::UIElement {
        &self.element.0
    }

    /// Create a new WindowsUIElement from a raw uiautomation element
    pub fn new(element: uiautomation::UIElement) -> Self {
        Self {
            #[allow(clippy::arc_with_non_send_sync)]
            element: ThreadSafeWinUIElement(std::sync::Arc::new(element)),
        }
    }
}

impl Debug for WindowsUIElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowsUIElement").finish()
    }
}

impl UIElementImpl for WindowsUIElement {
    fn object_id(&self) -> usize {
        // Use the common function to generate ID
        generate_element_id(&self.element.0).unwrap_or(0)
    }

    fn id(&self) -> Option<String> {
        Some(self.object_id().to_string().chars().take(6).collect())
    }

    fn role(&self) -> String {
        self.element
            .0
            .get_control_type()
            .map(|ct| ct.to_string())
            .unwrap_or_else(|_| "unknown".to_string())
    }

    fn attributes(&self) -> UIElementAttributes {
        // OPTIMIZATION: Use cached properties first to avoid expensive UI automation calls
        // This significantly reduces the number of cross-process calls to the UI automation system

        let mut properties = HashMap::new();

        // Helper function to filter empty strings
        fn filter_empty_string(s: Option<String>) -> Option<String> {
            s.filter(|s| !s.is_empty())
        }

        // OPTIMIZATION: Try cached properties first, fallback to live properties only if needed
        let role = self
            .element
            .0
            .get_cached_control_type()
            .or_else(|_| self.element.0.get_control_type())
            .map(|ct| ct.to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        // OPTIMIZATION: Use cached name first
        let name = filter_empty_string(
            self.element
                .0
                .get_cached_name()
                .or_else(|_| self.element.0.get_name())
                .ok(),
        );

        // OPTIMIZATION: Only load automation ID if name is empty (fallback identifier)
        // This reduces unnecessary property lookups for most elements
        let automation_id_for_properties = if name.is_none() {
            self.element
                .0
                .get_cached_automation_id()
                .or_else(|_| self.element.0.get_automation_id())
                .ok()
                .and_then(|aid| {
                    if !aid.is_empty() {
                        Some(serde_json::Value::String(aid.clone()))
                    } else {
                        None
                    }
                })
        } else {
            None
        };

        if let Some(aid_value) = automation_id_for_properties {
            properties.insert("AutomationId".to_string(), Some(aid_value));
        }

        // OPTIMIZATION: Defer all other expensive properties:
        // - Skip label lookup (get_labeled_by + get_name chain)
        // - Skip value lookup (UIProperty::ValueValue)
        // - Skip description lookup (get_help_text)
        // - Skip keyboard focusable lookup (UIProperty::IsKeyboardFocusable)
        // - Skip additional property enumeration
        // These can be loaded on-demand when specifically requested

        // Return minimal attribute set for maximum performance
        UIElementAttributes {
            role,
            name,
            label: None,                 // Deferred - load on demand
            value: None,                 // Deferred - load on demand
            description: None,           // Deferred - load on demand
            properties,                  // Minimal properties only
            is_keyboard_focusable: None, // Deferred - load on demand
            is_focused: None,            // Deferred - load on demand
            bounds: None, // Will be populated by get_configurable_attributes if focusable
            text: None,
            enabled: None,
            is_toggled: None,
            is_selected: None,
            child_count: None,
            index_in_parent: None,
        }
    }

    fn children(&self) -> Result<Vec<UIElement>, AutomationError> {
        // Try getting cached children first
        let children_result = self.element.0.get_cached_children();

        let children = match children_result {
            Ok(cached_children) => {
                info!("Found {} cached children.", cached_children.len());
                cached_children
            }
            Err(_) => {
                let temp_automation = create_ui_automation_with_com_init()?;
                let true_condition = temp_automation.create_true_condition().map_err(|e| {
                    AutomationError::PlatformError(format!(
                        "Failed to create true condition for child fallback: {e}"
                    ))
                })?;
                self.element
                    .0
                    .find_all(uiautomation::types::TreeScope::Children, &true_condition)
                    .map_err(|find_err| {
                        AutomationError::PlatformError(format!(
                            "Failed to get children (cached and non-cached): {find_err}"
                        ))
                    })? // Propagate error
            }
        };

        // Wrap the platform elements into our UIElement trait objects
        Ok(children
            .into_iter()
            .map(|ele| {
                #[allow(clippy::arc_with_non_send_sync)]
                UIElement::new(Box::new(WindowsUIElement {
                    element: ThreadSafeWinUIElement(Arc::new(ele)),
                }))
            })
            .collect())
    }

    fn parent(&self) -> Result<Option<UIElement>, AutomationError> {
        // Use TreeWalker instead of cached parent - this avoids caching setup requirements
        let temp_automation = create_ui_automation_with_com_init().map_err(|e| {
            AutomationError::PlatformError(format!(
                "Failed to create UI automation for parent navigation: {e}"
            ))
        })?;

        let walker = temp_automation.get_raw_view_walker().map_err(|e| {
            AutomationError::PlatformError(format!(
                "Failed to get tree walker for parent navigation: {e}"
            ))
        })?;

        match walker.get_parent(&self.element.0) {
            Ok(parent_element) => {
                #[allow(clippy::arc_with_non_send_sync)]
                let par_ele = UIElement::new(Box::new(WindowsUIElement {
                    element: ThreadSafeWinUIElement(Arc::new(parent_element)),
                }));
                Ok(Some(par_ele))
            }
            Err(e) => {
                // TreeWalker parent navigation failed - this usually means no parent exists (root element)
                tracing::debug!("TreeWalker get_parent failed: {}", e);
                Ok(None)
            }
        }
    }

    fn bounds(&self) -> Result<(f64, f64, f64, f64), AutomationError> {
        let rect = self
            .element
            .0
            .get_bounding_rectangle()
            .map_err(|e| AutomationError::ElementNotFound(e.to_string()))?;
        Ok((
            rect.get_left() as f64,
            rect.get_top() as f64,
            rect.get_width() as f64,
            rect.get_height() as f64,
        ))
    }

    fn click(&self) -> Result<ClickResult, AutomationError> {
        self.element.0.try_focus();
        debug!("attempting to click element: {:?}", self.element.0);

        let click_result = self.element.0.click();

        if click_result.is_ok() {
            return Ok(ClickResult {
                method: "Single Click".to_string(),
                coordinates: None,
                details: "Clicked by Mouse".to_string(),
            });
        }
        // First try using the standard clickable point
        let click_result = self
            .element
            .0
            .get_clickable_point()
            .and_then(|maybe_point| {
                if let Some(point) = maybe_point {
                    debug!("using clickable point: {:?}", point);
                    let mouse = Mouse::default();
                    mouse.click(point).map(|_| ClickResult {
                        method: "Single Click (Clickable Point)".to_string(),
                        coordinates: Some((point.get_x() as f64, point.get_y() as f64)),
                        details: "Clicked by Mouse using element's clickable point".to_string(),
                    })
                } else {
                    Err(
                        AutomationError::PlatformError("No clickable point found".to_string())
                            .to_string()
                            .into(),
                    )
                }
            });

        // If first method fails, try using the bounding rectangle
        if click_result.is_err() {
            debug!("clickable point unavailable, falling back to bounding rectangle");
            if let Ok(rect) = self.element.0.get_bounding_rectangle() {
                println!("bounding rectangle: {rect:?}");
                // Calculate center point of the element
                let center_x = rect.get_left() + rect.get_width() / 2;
                let center_y = rect.get_top() + rect.get_height() / 2;

                let point = Point::new(center_x, center_y);
                let mouse = Mouse::default();

                debug!("clicking at center point: ({}, {})", center_x, center_y);
                mouse
                    .click(point)
                    .map_err(|e| AutomationError::PlatformError(e.to_string()))?;

                return Ok(ClickResult {
                    method: "Single Click (Fallback)".to_string(),
                    coordinates: Some((center_x as f64, center_y as f64)),
                    details: "Clicked by Mouse using element's center coordinates".to_string(),
                });
            }
        }

        // Return the result of the first attempt or propagate the error
        click_result.map_err(|e| AutomationError::PlatformError(e.to_string()))
    }

    fn double_click(&self) -> Result<ClickResult, AutomationError> {
        self.element.0.try_focus();
        let point = self
            .element
            .0
            .get_clickable_point()
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?
            .ok_or_else(|| {
                AutomationError::PlatformError("No clickable point found".to_string())
            })?;
        let mouse = Mouse::default();
        mouse
            .double_click(point)
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?;
        Ok(ClickResult {
            method: "Double Click".to_string(),
            coordinates: Some((point.get_x() as f64, point.get_y() as f64)),
            details: "Clicked by Mouse".to_string(),
        })
    }

    fn right_click(&self) -> Result<(), AutomationError> {
        self.element.0.try_focus();
        let point = self
            .element
            .0
            .get_clickable_point()
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?
            .ok_or_else(|| {
                AutomationError::PlatformError("No clickable point found".to_string())
            })?;
        let mouse = Mouse::default();
        mouse
            .right_click(point)
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?;
        Ok(())
    }

    fn hover(&self) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "`hover` doesn't not support".to_string(),
        ))
    }

    fn focus(&self) -> Result<(), AutomationError> {
        self.element
            .0
            .set_focus()
            .map_err(|e| AutomationError::PlatformError(e.to_string()))
    }

    fn invoke(&self) -> Result<(), AutomationError> {
        let invoke_pat = self
            .element
            .0
            .get_pattern::<patterns::UIInvokePattern>()
            .map_err(|e| {
                let error_str = e.to_string();
                if error_str.contains("not support") || error_str.contains("UIA_E_ELEMENTNOTAVAILABLE") {
                    AutomationError::UnsupportedOperation(format!(
                        "Element does not support InvokePattern. This typically happens with custom controls, groups, or non-standard buttons. Try using 'click_element' instead. Error: {error_str}"
                    ))
                } else {
                    AutomationError::PlatformError(format!("Failed to get InvokePattern: {e}"))
                }
            })?;
        invoke_pat
            .invoke()
            .map_err(|e| AutomationError::PlatformError(e.to_string()))
    }

    fn activate_window(&self) -> Result<(), AutomationError> {
        use windows::Win32::UI::WindowsAndMessaging::{
            BringWindowToTop, IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE,
        };

        debug!(
            "Activating window by focusing element: {:?}",
            self.element.0
        );

        // First try to get the native window handle
        let hwnd = match self.element.0.get_native_window_handle() {
            Ok(handle) => handle,
            Err(_) => {
                // Fallback to just setting focus if we can't get the window handle
                debug!("Could not get native window handle, falling back to set_focus");
                return self.focus();
            }
        };

        unsafe {
            let hwnd_param: windows::Win32::Foundation::HWND = hwnd.into();

            // Check if the window is minimized and restore it if needed
            if IsIconic(hwnd_param).as_bool() {
                debug!("Window is minimized, restoring it");
                let _ = ShowWindow(hwnd_param, SW_RESTORE);
            }

            // Bring the window to the top of the Z order
            let _ = BringWindowToTop(hwnd_param);

            // Set as the foreground window (this is the key method for activation)
            let result = SetForegroundWindow(hwnd_param);

            if !result.as_bool() {
                debug!("SetForegroundWindow failed, but continuing");
                // Note: SetActiveWindow is not available in the current Windows crate version
                // The SetForegroundWindow should be sufficient for most cases
            }

            // Finally, set focus to the specific element
            let _ = self.element.0.set_focus();
        }

        debug!("Window activation completed");
        Ok(())
    }

    fn minimize_window(&self) -> Result<(), AutomationError> {
        use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_MINIMIZE};

        debug!("Minimizing window for element: {:?}", self.element.0);

        // First try to get the native window handle
        let hwnd = match self.element.0.get_native_window_handle() {
            Ok(handle) => handle,
            Err(_) => {
                return Err(AutomationError::PlatformError(
                    "Could not get native window handle for minimize operation".to_string(),
                ));
            }
        };

        unsafe {
            let hwnd_param: windows::Win32::Foundation::HWND = hwnd.into();

            // Minimize the window
            let result = ShowWindow(hwnd_param, SW_MINIMIZE);

            if result.as_bool() {
                debug!("Window minimized successfully");
            } else {
                debug!("Window was already minimized or minimize operation had no effect");
            }
        }

        debug!("Window minimize operation completed");
        Ok(())
    }

    fn maximize_window(&self) -> Result<(), AutomationError> {
        debug!("Maximizing window for element: {:?}", self.element.0);

        // First try using the WindowPattern which is the preferred method
        if let Ok(window_pattern) = self.element.0.get_pattern::<patterns::UIWindowPattern>() {
            debug!("Using WindowPattern to maximize window");
            window_pattern
                .set_window_visual_state(uiautomation::types::WindowVisualState::Maximized)
                .map_err(|e| {
                    AutomationError::PlatformError(format!(
                        "Failed to maximize window using WindowPattern: {e}"
                    ))
                })?;
            debug!("Window maximized successfully using WindowPattern");
            return Ok(());
        }

        // Fallback to native Windows API if WindowPattern is not available
        debug!("WindowPattern not available, falling back to native Windows API");
        let hwnd = match self.element.0.get_native_window_handle() {
            Ok(handle) => handle,
            Err(_) => {
                return Err(AutomationError::PlatformError(
                    "Could not get native window handle for maximize operation".to_string(),
                ));
            }
        };

        use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_MAXIMIZE};

        unsafe {
            let hwnd_param: windows::Win32::Foundation::HWND = hwnd.into();

            // Maximize the window
            let result = ShowWindow(hwnd_param, SW_MAXIMIZE);

            if result.as_bool() {
                debug!("Window maximized successfully using native API");
            } else {
                debug!("Window was already maximized or maximize operation had no effect");
            }
        }

        debug!("Window maximize operation completed");
        Ok(())
    }

    fn type_text(&self, text: &str, use_clipboard: bool) -> Result<(), AutomationError> {
        let control_type = self
            .element
            .0
            .get_control_type()
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?;

        debug!(
            "typing text with control_type: {:#?}, use_clipboard: {}",
            control_type, use_clipboard
        );

        if use_clipboard {
            // Try clipboard typing first
            match self.element.0.send_text_by_clipboard(text) {
                Ok(()) => Ok(()),
                Err(e) => {
                    // Clipboard method failed, fall back to key-by-key typing
                    debug!(
                        "Clipboard typing returned error: {:?}. Using key-by-key input instead.",
                        e
                    );
                    self.element
                        .0
                        .send_text(text, 10)
                        .map_err(|e| AutomationError::PlatformError(e.to_string()))
                }
            }
        } else {
            // Use standard typing method
            self.element
                .0
                .send_text(text, 10)
                .map_err(|e| AutomationError::PlatformError(e.to_string()))
        }
    }

    fn press_key(&self, key: &str) -> Result<(), AutomationError> {
        let control_type = self.element.0.get_control_type().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get control type: {e:?}"))
        })?;
        // check if element accepts input, similar :D
        debug!("pressing key with control_type: {:#?}", control_type);
        self.element
            .0
            .send_keys(key, 10)
            .map_err(|e| AutomationError::PlatformError(format!("Failed to press key: {e:?}")))
    }

    fn get_text(&self, max_depth: usize) -> Result<String, AutomationError> {
        let mut all_texts = Vec::new();
        let automation = create_ui_automation_with_com_init()?;

        // Create a function to extract text recursively
        fn extract_text_from_element(
            automation: &UIAutomation,
            element: &uiautomation::UIElement,
            texts: &mut Vec<String>,
            current_depth: usize,
            max_depth: usize,
        ) -> Result<(), AutomationError> {
            if current_depth > max_depth {
                return Ok(());
            }

            // Check Value property
            if let Ok(value) = element.get_property_value(UIProperty::ValueValue) {
                if let Ok(value_text) = value.get_string() {
                    if !value_text.is_empty() {
                        debug!("found text in value property: {:?}", &value_text);
                        texts.push(value_text);
                    }
                }
            }

            // Recursively process children
            let children_result = element.get_cached_children();

            let children_to_process = match children_result {
                Ok(cached_children) => {
                    info!(
                        "Found {} cached children for text extraction.",
                        cached_children.len()
                    );
                    cached_children
                }
                Err(_) => {
                    match automation.create_true_condition() {
                        Ok(true_condition) => {
                            // Perform the non-cached search for direct children
                            element
                                .find_all(uiautomation::types::TreeScope::Children, &true_condition)
                                .unwrap_or_default()
                        }
                        Err(cond_err) => {
                            error!(
                                "Failed to create true condition for child fallback in text extraction: {}",
                                cond_err
                            );
                            vec![] // Return empty vec on condition creation error
                        }
                    }
                }
            };

            // Process the children (either cached or found via fallback)
            for child in children_to_process {
                let _ = extract_text_from_element(
                    automation,
                    &child,
                    texts,
                    current_depth + 1,
                    max_depth,
                );
            }

            Ok(())
        }

        // Extract text from the element and its descendants
        extract_text_from_element(&automation, &self.element.0, &mut all_texts, 0, max_depth)?;

        // Join the texts with spaces
        Ok(all_texts.join(" "))
    }

    fn set_value(&self, value: &str) -> Result<(), AutomationError> {
        debug!(
            "setting value: {:#?} to ui element {:#?}",
            &value, &self.element.0
        );

        let value_par = self
            .element
            .0
            .get_pattern::<patterns::UIValuePattern>()
            .map_err(|e| {
                let error_str = e.to_string();
                if error_str.contains("not support") || error_str.contains("UIA_E_ELEMENTNOTAVAILABLE") {
                    AutomationError::UnsupportedOperation(format!(
                        "Element does not support ValuePattern. This control cannot have its value set directly. Try using 'type_into_element' for text input, or 'select_option' for dropdowns. Error: {error_str}"
                    ))
                } else {
                    AutomationError::PlatformError(format!("Failed to get ValuePattern: {e}"))
                }
            })?;

        value_par
            .set_value(value)
            .map_err(|e| AutomationError::PlatformError(e.to_string()))
    }

    fn is_enabled(&self) -> Result<bool, AutomationError> {
        self.element
            .0
            .is_enabled()
            .map_err(|e| AutomationError::ElementNotFound(e.to_string()))
    }

    fn is_visible(&self) -> Result<bool, AutomationError> {
        self.element
            .0
            .is_offscreen()
            .map(|is_offscreen| !is_offscreen)
            .map_err(|e| AutomationError::ElementNotFound(e.to_string()))
    }

    fn is_focused(&self) -> Result<bool, AutomationError> {
        self.element.0.has_keyboard_focus().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get keyboard focus state: {e}"))
        })
    }

    fn perform_action(&self, action: &str) -> Result<(), AutomationError> {
        // actions those don't take args
        match action {
            "focus" => self.focus(),
            "invoke" => self.invoke(),
            "click" => self.click().map(|_| ()),
            "double_click" => self.double_click().map(|_| ()),
            "right_click" => self.right_click().map(|_| ()),
            "toggle" => {
                let toggle_pattern = self
                    .element
                    .0
                    .get_pattern::<patterns::UITogglePattern>()
                    .map_err(|e| {
                        let error_str = e.to_string();
                        if error_str.contains("not support") || error_str.contains("UIA_E_ELEMENTNOTAVAILABLE") {
                            AutomationError::UnsupportedOperation(format!(
                                "Element does not support TogglePattern. This is not a toggleable control (checkbox, switch, etc.). Try using 'click' instead. Error: {error_str}"
                            ))
                        } else {
                            AutomationError::PlatformError(format!("Failed to get TogglePattern: {e}"))
                        }
                    })?;
                toggle_pattern
                    .toggle()
                    .map_err(|e| AutomationError::PlatformError(e.to_string()))
            }
            "expand_collapse" => {
                let expand_collapse_pattern = self
                    .element
                    .0
                    .get_pattern::<patterns::UIExpandCollapsePattern>()
                    .map_err(|e| {
                        let error_str = e.to_string();
                        if error_str.contains("not support") || error_str.contains("UIA_E_ELEMENTNOTAVAILABLE") {
                            AutomationError::UnsupportedOperation(format!(
                                "Element does not support ExpandCollapsePattern. This is not an expandable control (tree item, dropdown, etc.). Try using 'click' to interact with it. Error: {error_str}"
                            ))
                        } else {
                            AutomationError::PlatformError(format!("Failed to get ExpandCollapsePattern: {e}"))
                        }
                    })?;
                expand_collapse_pattern
                    .expand()
                    .map_err(|e| AutomationError::PlatformError(e.to_string()))
            }
            _ => Err(AutomationError::UnsupportedOperation(format!(
                "action '{action}' not supported"
            ))),
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn create_locator(&self, selector: Selector) -> Result<Locator, AutomationError> {
        let automation = WindowsEngine::new(false, false)
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?;

        let attrs = self.attributes();
        debug!(
            "creating locator for element: control_type={:#?}, label={:#?}",
            attrs.role, attrs.label
        );

        let self_element = UIElement::new(Box::new(WindowsUIElement {
            element: self.element.clone(),
        }));

        Ok(Locator::new(std::sync::Arc::new(automation), selector).within(self_element))
    }

    fn clone_box(&self) -> Box<dyn UIElementImpl> {
        Box::new(WindowsUIElement {
            element: self.element.clone(),
        })
    }

    #[allow(clippy::arc_with_non_send_sync)]
    fn scroll(&self, direction: &str, amount: f64) -> Result<(), AutomationError> {
        // 1. Find a scrollable parent (or self)
        let mut scrollable_element: Option<uiautomation::UIElement> = None;
        let mut current_element_arc = self.element.0.clone();

        for _ in 0..7 {
            // Search up to 7 levels up the tree
            if let Ok(_pattern) = current_element_arc.get_pattern::<patterns::UIScrollPattern>() {
                // Element supports scrolling, we found our target
                scrollable_element = Some(current_element_arc.as_ref().clone());
                break;
            }

            // Move to parent
            match current_element_arc.get_cached_parent() {
                Ok(parent) => {
                    // Check if we've hit the root or a cycle
                    if let (Ok(cur_id), Ok(par_id)) = (
                        current_element_arc.get_runtime_id(),
                        parent.get_runtime_id(),
                    ) {
                        if cur_id == par_id {
                            break;
                        }
                    }
                    current_element_arc = Arc::new(parent);
                }
                Err(_) => {
                    break;
                }
            }
        }

        if let Some(target_element) = scrollable_element {
            // 2. Use ScrollPattern to scroll with enhanced direction support
            if let Ok(scroll_pattern) = target_element.get_pattern::<patterns::UIScrollPattern>() {
                let (h_amount, v_amount) =
                    match direction {
                        "up" => (
                            uiautomation::types::ScrollAmount::NoAmount,
                            uiautomation::types::ScrollAmount::LargeDecrement,
                        ),
                        "down" => (
                            uiautomation::types::ScrollAmount::NoAmount,
                            uiautomation::types::ScrollAmount::LargeIncrement,
                        ),
                        "left" => (
                            uiautomation::types::ScrollAmount::LargeDecrement,
                            uiautomation::types::ScrollAmount::NoAmount,
                        ),
                        "right" => (
                            uiautomation::types::ScrollAmount::LargeIncrement,
                            uiautomation::types::ScrollAmount::NoAmount,
                        ),
                        _ => return Err(AutomationError::InvalidArgument(
                            "Invalid scroll direction. Supported: 'up', 'down', 'left', 'right'"
                                .to_string(),
                        )),
                    };

                let num_scrolls = amount.round().max(1.0) as usize;
                for i in 0..num_scrolls {
                    if scroll_pattern.scroll(h_amount, v_amount).is_err() {
                        // If pattern fails, break and try the key press fallback
                        warn!(
                            "ScrollPattern failed on iteration {}. Attempting key-press fallback.",
                            i
                        );
                        return self.scroll_with_fallback(direction, amount);
                    }
                    // Small delay between programmatic scrolls to allow UI to catch up
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                return Ok(());
            }
        }

        // 3. If ScrollPattern fails or no scrollable element found, fall back to key presses on the original element
        self.scroll_with_fallback(direction, amount)
    }

    fn is_keyboard_focusable(&self) -> Result<bool, AutomationError> {
        let variant = self
            .element
            .0
            .get_property_value(UIProperty::IsKeyboardFocusable)
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?;
        variant.try_into().map_err(|e| {
            AutomationError::PlatformError(format!(
                "Failed to convert IsKeyboardFocusable to bool: {e:?}"
            ))
        })
    }

    // New method for mouse drag
    fn mouse_drag(
        &self,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
    ) -> Result<(), AutomationError> {
        use std::thread::sleep;
        use std::time::Duration;
        self.mouse_click_and_hold(start_x, start_y)?;
        sleep(Duration::from_millis(20));
        self.mouse_move(end_x, end_y)?;
        sleep(Duration::from_millis(20));
        self.mouse_release()?;
        Ok(())
    }

    // New mouse control methods
    fn mouse_click_and_hold(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN,
            MOUSEEVENTF_MOVE, MOUSEINPUT,
        };
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
        fn to_absolute(x: f64, y: f64) -> (i32, i32) {
            let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
            let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
            let abs_x = ((x / screen_w as f64) * 65535.0).round() as i32;
            let abs_y = ((y / screen_h as f64) * 65535.0).round() as i32;
            (abs_x, abs_y)
        }
        let (abs_x, abs_y) = to_absolute(x, y);
        let move_input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: abs_x,
                    dy: abs_y,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let down_input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_LEFTDOWN,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            SendInput(&[move_input], std::mem::size_of::<INPUT>() as i32);
            SendInput(&[down_input], std::mem::size_of::<INPUT>() as i32);
        }
        Ok(())
    }
    fn mouse_move(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_MOVE,
            MOUSEINPUT,
        };
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
        fn to_absolute(x: f64, y: f64) -> (i32, i32) {
            let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
            let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
            let abs_x = ((x / screen_w as f64) * 65535.0).round() as i32;
            let abs_y = ((y / screen_h as f64) * 65535.0).round() as i32;
            (abs_x, abs_y)
        }
        let (abs_x, abs_y) = to_absolute(x, y);
        let move_input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: abs_x,
                    dy: abs_y,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            SendInput(&[move_input], std::mem::size_of::<INPUT>() as i32);
        }
        Ok(())
    }
    fn mouse_release(&self) -> Result<(), AutomationError> {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTUP, MOUSEINPUT,
        };
        let up_input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_LEFTUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            SendInput(&[up_input], std::mem::size_of::<INPUT>() as i32);
        }
        Ok(())
    }

    fn application(&self) -> Result<Option<UIElement>, AutomationError> {
        // Get the process ID of the current element
        let pid = self.element.0.get_process_id().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get process ID for element: {e}"))
        })?;

        // Create a WindowsEngine instance to use its methods.
        // This follows the pattern in `create_locator` but might be inefficient if called frequently.
        let engine = WindowsEngine::new(false, false).map_err(|e| {
            AutomationError::PlatformError(format!("Failed to create WindowsEngine: {e}"))
        })?;

        // Get the application element by PID
        match get_application_by_pid(&engine, pid as i32, Some(DEFAULT_FIND_TIMEOUT)) {
            // Cast pid to i32
            Ok(app_element) => Ok(Some(app_element)),
            Err(AutomationError::ElementNotFound(_)) => {
                // If the specific application element is not found by PID, return None.
                debug!("Application element not found for PID {}", pid);
                Ok(None)
            }
            Err(e) => Err(e), // Propagate other errors
        }
    }

    #[allow(clippy::arc_with_non_send_sync)]
    fn window(&self) -> Result<Option<UIElement>, AutomationError> {
        let mut current_element_arc = Arc::clone(&self.element.0); // Start with the current element's Arc<uiautomation::UIElement>
        const MAX_DEPTH: usize = 20; // Safety break for parent traversal

        // Strategy: Find the FIRST Pane, or fall back to the FIRST Window
        // This prioritizes finding the closest application container (Pane) over system containers (Window)
        let mut first_pane: Option<Arc<uiautomation::UIElement>> = None;
        let mut first_window: Option<Arc<uiautomation::UIElement>> = None;

        for i in 0..MAX_DEPTH {
            // Check current element's control type
            match current_element_arc.get_control_type() {
                Ok(control_type) => {
                    match control_type {
                        ControlType::Pane => {
                            if first_pane.is_none() {
                                first_pane = Some(Arc::clone(&current_element_arc));
                                // Found a Pane - this is what we want for Chrome, stop here
                                break;
                            }
                        }
                        ControlType::Window => {
                            if first_window.is_none() {
                                first_window = Some(Arc::clone(&current_element_arc));
                                // Don't break - keep looking for a Pane
                            }
                        }
                        _ => {} // Continue traversing for other control types
                    }
                }
                Err(e) => {
                    return Err(AutomationError::PlatformError(format!(
                        "Failed to get control type for element during window search (iteration {i}): {e}"
                    )));
                }
            }

            // Try to get the parent
            match current_element_arc.get_cached_parent() {
                Ok(parent_uia_element) => {
                    // Check if parent is same as current (e.g. desktop root's parent is itself)
                    // This requires getting runtime IDs, which can also fail.
                    let current_runtime_id = current_element_arc.get_runtime_id().map_err(|e| {
                        AutomationError::PlatformError(format!(
                            "Failed to get runtime_id for current element: {e}"
                        ))
                    })?;
                    let parent_runtime_id = parent_uia_element.get_runtime_id().map_err(|e| {
                        AutomationError::PlatformError(format!(
                            "Failed to get runtime_id for parent element: {e}"
                        ))
                    })?;

                    if parent_runtime_id == current_runtime_id {
                        debug!(
                            "Parent element has same runtime ID as current, stopping window search."
                        );
                        break; // Reached the top or a cycle.
                    }
                    current_element_arc = Arc::new(parent_uia_element); // Move to the parent
                }
                Err(_) => {
                    break;
                }
            }
        }

        // Return the best candidate we found (prefer first Pane over first Window)
        let chosen_element = first_pane.or(first_window);

        if let Some(element) = chosen_element {
            let window_ui_element = WindowsUIElement {
                element: ThreadSafeWinUIElement(element),
            };
            Ok(Some(UIElement::new(Box::new(window_ui_element))))
        } else {
            // If loop finishes, no element with ControlType::Window or Pane was found.
            Ok(None)
        }
    }

    fn highlight(
        &self,
        color: Option<u32>,
        duration: Option<std::time::Duration>,
        text: Option<&str>,
        text_position: Option<TextPosition>,
        font_style: Option<FontStyle>,
    ) -> Result<HighlightHandle, AutomationError> {
        highlighting::highlight(
            self.element.0.clone(),
            color,
            duration,
            text,
            text_position,
            font_style,
        )
    }
    fn process_id(&self) -> Result<u32, AutomationError> {
        self.element.0.get_process_id().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get process ID for element: {e}"))
        })
    }

    fn close(&self) -> Result<(), AutomationError> {
        // Check the control type to determine if this element is closable
        let control_type = self.element.0.get_control_type().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get control type: {e}"))
        })?;

        match control_type {
            ControlType::Window | ControlType::Pane => {
                // For windows and panes, try to close them

                // First try using the WindowPattern to close the window
                if let Ok(window_pattern) =
                    self.element.0.get_pattern::<patterns::UIWindowPattern>()
                {
                    debug!("Attempting to close window using WindowPattern");
                    let close_result = window_pattern.close();
                    match close_result {
                        Ok(()) => return Ok(()),
                        Err(e) => {
                            let error_str = e.to_string();
                            if error_str.contains("not support")
                                || error_str.contains("UIA_E_ELEMENTNOTAVAILABLE")
                            {
                                // Window doesn't support WindowPattern, try Alt+F4
                                debug!("WindowPattern not supported, falling back to Alt+F4");
                                self.element.0.try_focus();
                                return self.element
                                    .0
                                    .send_keys("%{F4}", 10) // Alt+F4
                                    .map_err(|e2| {
                                        AutomationError::PlatformError(format!(
                                            "Failed to close window: WindowPattern not supported and Alt+F4 failed: {e2}"
                                        ))
                                    });
                            } else {
                                return Err(AutomationError::PlatformError(format!(
                                    "Failed to close window: {e}"
                                )));
                            }
                        }
                    }
                }

                // Fallback: try to send Alt+F4 to close the window
                debug!("WindowPattern not available, trying Alt+F4 as fallback");
                self.element.0.try_focus(); // Focus first
                match self.element.0.send_keys("%{F4}", 10) {
                    Ok(()) => Ok(()),
                    Err(alt_err) => {
                        debug!("Alt+F4 failed: {alt_err}. Attempting process termination fallback");

                        // Try to get the process ID so we can force-terminate it
                        match self.element.0.get_process_id() {
                            Ok(pid) => {
                                // First, try taskkill (built-in)
                                let taskkill_status = std::process::Command::new("taskkill")
                                    .args(["/PID", &pid.to_string(), "/T", "/F"])
                                    .status();

                                if let Ok(status) = taskkill_status {
                                    if status.success() {
                                        debug!("Successfully terminated process {pid} using taskkill");
                                        return Ok(());
                                    }
                                }

                                // If taskkill failed, fall back to PowerShell Stop-Process
                                let ps_status = std::process::Command::new("powershell")
                                    .args([
                                        "-NoProfile",
                                        "-WindowStyle",
                                        "hidden",
                                        "-Command",
                                        &format!("Stop-Process -Id {pid} -Force"),
                                    ])
                                    .status();

                                if let Ok(status) = ps_status {
                                    if status.success() {
                                        debug!("Successfully terminated process {pid} using PowerShell Stop-Process");
                                        return Ok(());
                                    }
                                }

                                Err(AutomationError::PlatformError(format!(
                                    "Failed to close window: WindowPattern/Alt+F4 failed, and both taskkill and Stop-Process were unsuccessful (Alt+F4 error: {alt_err})"
                                )))
                            }
                            Err(pid_err) => Err(AutomationError::PlatformError(format!(
                                "Failed to close window: Alt+F4 failed ({alt_err}) and could not determine PID: {pid_err}"
                            ))),
                        }
                    }
                }
            }
            ControlType::Button => {
                // For buttons, check if it's a close button by name/text
                let name = self.element.0.get_name().unwrap_or_default().to_lowercase();
                if name.contains("close")
                    || name.contains("×")
                    || name.contains("✕")
                    || name.contains("x")
                {
                    debug!("Clicking close button: {}", name);
                    self.click().map(|_| ())
                } else {
                    // Regular button - not a close action
                    debug!("Button '{}' is not a close button", name);
                    Err(AutomationError::UnsupportedOperation(format!(
                        "Button '{name}' is not a close button. Only windows, dialogs, and close buttons can be closed."
                    )))
                }
            }
            _ => {
                // For other control types (text, edit, etc.), closing is not supported
                debug!("Element type {:?} is not closable", control_type);
                Err(AutomationError::UnsupportedOperation(format!(
                    "Element of type '{control_type}' cannot be closed. Only windows, dialogs, and close buttons support the close operation."
                )))
            }
        }
    }

    fn capture(&self) -> Result<ScreenshotResult, AutomationError> {
        // Get the raw UIAutomation bounds
        let rect = self.element.0.get_bounding_rectangle().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get bounding rectangle: {e}"))
        })?;

        // Get all monitors that intersect with the element
        let mut intersected_monitors = Vec::new();
        let monitors = xcap::Monitor::all()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get monitors: {e}")))?;

        for monitor in monitors {
            let monitor_x = monitor.x().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor x: {e}"))
            })?;
            let monitor_y = monitor.y().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor y: {e}"))
            })?;
            let monitor_width = monitor.width().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor width: {e}"))
            })? as i32;
            let monitor_height = monitor.height().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor height: {e}"))
            })? as i32;

            // Check if element intersects with this monitor
            if rect.get_left() < monitor_x + monitor_width
                && rect.get_left() + rect.get_width() > monitor_x
                && rect.get_top() < monitor_y + monitor_height
                && rect.get_top() + rect.get_height() > monitor_y
            {
                intersected_monitors.push(monitor);
            }
        }

        if intersected_monitors.is_empty() {
            return Err(AutomationError::PlatformError(
                "Element is not visible on any monitor".to_string(),
            ));
        }

        // If element spans multiple monitors, capture from the primary monitor
        let monitor = &intersected_monitors[0];
        let scale_factor = monitor.scale_factor().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get scale factor: {e}"))
        })?;

        // Get monitor bounds
        let monitor_x = monitor
            .x()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get monitor x: {e}")))?
            as u32;
        let monitor_y = monitor
            .y()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get monitor y: {e}")))?
            as u32;
        let monitor_width = monitor.width().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitor width: {e}"))
        })?;
        let monitor_height = monitor.height().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitor height: {e}"))
        })?;

        // Calculate scaled coordinates
        let scaled_x = (rect.get_left() as f64 * scale_factor as f64) as u32;
        let scaled_y = (rect.get_top() as f64 * scale_factor as f64) as u32;
        let scaled_width = (rect.get_width() as f64 * scale_factor as f64) as u32;
        let scaled_height = (rect.get_height() as f64 * scale_factor as f64) as u32;

        // Convert to relative coordinates for capture_region
        let rel_x = scaled_x.saturating_sub(monitor_x);
        let rel_y = scaled_y.saturating_sub(monitor_y);

        // Ensure width and height don't exceed monitor bounds
        let rel_width = std::cmp::min(scaled_width, monitor_width - rel_x);
        let rel_height = std::cmp::min(scaled_height, monitor_height - rel_y);

        // Capture the screen region
        let capture = monitor
            .capture_region(rel_x, rel_y, rel_width, rel_height)
            .map_err(|e| {
                AutomationError::PlatformError(format!("Failed to capture region: {e}"))
            })?;

        Ok(ScreenshotResult {
            image_data: capture.to_vec(),
            width: rel_width,
            height: rel_height,
            monitor: None,
        })
    }

    fn set_transparency(&self, percentage: u8) -> Result<(), AutomationError> {
        // Convert percentage (0-100) to alpha (0-255)
        let alpha = ((percentage as f32 / 100.0) * 255.0) as u8;

        // Get the window handle
        let hwnd = self.element.0.get_native_window_handle().map_err(|e| {
            AutomationError::PlatformError(format!(
                "Failed to get native window handle of element: {e}"
            ))
        })?;

        // Set the window to be layered
        unsafe {
            let style = windows::Win32::UI::WindowsAndMessaging::GetWindowLongW(
                hwnd.into(),
                windows::Win32::UI::WindowsAndMessaging::WINDOW_LONG_PTR_INDEX(-20), // GWL_EXSTYLE
            );
            if style == 0 {
                return Err(AutomationError::PlatformError(
                    "Failed to get window style".to_string(),
                ));
            }
            let new_style = style | 0x00080000; // WS_EX_LAYERED
            if windows::Win32::UI::WindowsAndMessaging::SetWindowLongW(
                hwnd.into(),
                windows::Win32::UI::WindowsAndMessaging::WINDOW_LONG_PTR_INDEX(-20), // GWL_EXSTYLE
                new_style,
            ) == 0
            {
                return Err(AutomationError::PlatformError(
                    "Failed to set window style".to_string(),
                ));
            }
        }

        // Set the transparency
        unsafe {
            let result = windows::Win32::UI::WindowsAndMessaging::SetLayeredWindowAttributes(
                hwnd.into(),
                windows::Win32::Foundation::COLORREF(0), // crKey - not used with LWA_ALPHA
                alpha,
                windows::Win32::UI::WindowsAndMessaging::LAYERED_WINDOW_ATTRIBUTES_FLAGS(
                    0x00000002,
                ), // LWA_ALPHA
            );
            if result.is_err() {
                return Err(AutomationError::PlatformError(
                    "Failed to set window transparency".to_string(),
                ));
            }
        }

        Ok(())
    }

    fn url(&self) -> Option<String> {
        let automation = match create_ui_automation_with_com_init() {
            Ok(a) => a,
            Err(e) => {
                debug!(
                    "Failed to create UIAutomation instance for URL detection: {}",
                    e
                );
                return None;
            }
        };

        // Find the root window for the element.
        let search_root = if let Ok(Some(window)) = self.window() {
            window
                .as_any()
                .downcast_ref::<WindowsUIElement>()
                .map(|win_el| win_el.element.0.clone())
                .unwrap_or_else(|| self.element.0.clone())
        } else {
            self.element.0.clone()
        };

        debug!(
            "URL search root: {}",
            search_root.get_name().unwrap_or_default()
        );

        // Try to find address bar using a more flexible filter function.
        let address_bar_keywords = ["address", "location", "url", "website", "search", "go to"];

        let matcher = automation
            .create_matcher()
            .from_ref(&search_root)
            .control_type(ControlType::Edit)
            .filter_fn(Box::new(move |e: &uiautomation::UIElement| {
                if let Ok(name) = e.get_name() {
                    let name_lower = name.to_lowercase();
                    if address_bar_keywords
                        .iter()
                        .any(|&keyword| name_lower.contains(keyword))
                    {
                        return Ok(true);
                    }
                }
                Ok(false)
            }))
            .timeout(200) // Quick search for the best case
            .depth(10);

        if let Ok(element) = matcher.find_first() {
            if let Ok(value_pattern) = element.get_pattern::<patterns::UIValuePattern>() {
                if let Ok(value) = value_pattern.get_value() {
                    debug!("Found URL via keyword search for address bar: {}", value);
                    return Some(value);
                }
            }
        }

        // Fallback: If no specifically named address bar is found,
        // search for ANY edit control with a URL in it, as a broader but still constrained search.
        // This can help with non-standard browsers or updated UI.
        let edit_condition = automation
            .create_property_condition(
                UIProperty::ControlType,
                Variant::from(ControlType::Edit as i32),
                None,
            )
            .unwrap();
        if let Ok(candidates) = search_root.find_all(TreeScope::Descendants, &edit_condition) {
            for candidate in candidates {
                if let Ok(value_pattern) = candidate.get_pattern::<patterns::UIValuePattern>() {
                    if let Ok(url) = value_pattern.get_value() {
                        if url.starts_with("http") {
                            debug!("Found URL in fallback search of Edit controls: {}", url);
                            return Some(url);
                        }
                    }
                }
            }
        }

        debug!("Could not find URL in any address bar candidate.");
        None
    }

    fn select_option(&self, option_name: &str) -> Result<(), AutomationError> {
        // Expand the dropdown/combobox first
        if let Ok(expand_collapse_pattern) = self
            .element
            .0
            .get_pattern::<patterns::UIExpandCollapsePattern>()
        {
            expand_collapse_pattern.expand().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to expand element: {e}"))
            })?;
        }

        // Wait a moment for options to appear
        std::thread::sleep(std::time::Duration::from_millis(200));

        // Find the specific option by name
        let automation = UIAutomation::new_direct()
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?;
        let option_element = self
            .element
            .0
            .find_first(
                TreeScope::Descendants,
                &automation
                    .create_property_condition(
                        uiautomation::types::UIProperty::Name,
                        option_name.into(),
                        None,
                    )
                    .unwrap(),
            )
            .map_err(|e| {
                AutomationError::ElementNotFound(format!(
                    "Option '{option_name}' not found in dropdown. Make sure the dropdown is expanded and the option name is exact. Error: {e}"
                ))
            })?;

        // Select the option
        if let Ok(selection_item_pattern) =
            option_element.get_pattern::<patterns::UISelectionItemPattern>()
        {
            selection_item_pattern.select().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to select option: {e}"))
            })?;
        } else {
            // Fallback to click if selection pattern is not available
            debug!(
                "SelectionItemPattern not available for option '{}', falling back to click",
                option_name
            );
            option_element.click().map_err(|e| {
                AutomationError::PlatformError(format!(
                    "Failed to click option '{option_name}': {e}"
                ))
            })?;
        }

        // Try to collapse the dropdown again
        if let Ok(expand_collapse_pattern) = self
            .element
            .0
            .get_pattern::<patterns::UIExpandCollapsePattern>()
        {
            let _ = expand_collapse_pattern.collapse();
        }

        Ok(())
    }

    fn list_options(&self) -> Result<Vec<String>, AutomationError> {
        let mut options = Vec::new();
        // Ensure the element is expanded to reveal options
        if let Ok(expand_collapse_pattern) = self
            .element
            .0
            .get_pattern::<patterns::UIExpandCollapsePattern>()
        {
            let state_variant = self
                .element
                .0
                .get_property_value(UIProperty::ExpandCollapseExpandCollapseState)
                .map_err(|e| AutomationError::PlatformError(e.to_string()))?;

            let state_val: i32 = state_variant.try_into().map_err(|_| {
                AutomationError::PlatformError(
                    "Failed to convert expand/collapse state variant to i32".to_string(),
                )
            })?;
            let state = match state_val {
                0 => uiautomation::types::ExpandCollapseState::Collapsed,
                1 => uiautomation::types::ExpandCollapseState::Expanded,
                2 => uiautomation::types::ExpandCollapseState::PartiallyExpanded,
                3 => uiautomation::types::ExpandCollapseState::LeafNode,
                _ => uiautomation::types::ExpandCollapseState::Collapsed, // Default case
            };

            if state != uiautomation::types::ExpandCollapseState::Expanded {
                expand_collapse_pattern.expand().map_err(|e| {
                    AutomationError::PlatformError(format!(
                        "Failed to expand element to list options: {e}"
                    ))
                })?;
                std::thread::sleep(std::time::Duration::from_millis(200)); // Wait for animation
            }
        } else {
            debug!("Element does not support ExpandCollapsePattern, attempting to list visible children directly");
        }

        // Search for ListItem children
        let children = self.children()?;
        for child in children {
            let role = child.role();
            if role == "ListItem" || role == "MenuItem" || role == "Option" {
                if let Some(name) = child.name() {
                    options.push(name);
                }
            }
        }

        if options.is_empty() {
            debug!("No options found. The element might not be a dropdown/list, or options might have different roles");
        }

        Ok(options)
    }

    fn is_toggled(&self) -> Result<bool, AutomationError> {
        // let toggle_pattern = self.element.0.get_pattern::<patterns::UITogglePattern>();

        // if let Ok(pattern) = toggle_pattern {
        // let state = pattern.get_toggle_state().map_err(|e| {
        //     AutomationError::PlatformError(format!("Failed to get toggle state: {e}"))
        // })?;
        // return Ok(state == uiautomation::types::ToggleState::On);

        let current_state = self.element.0.get_name().unwrap_or_default().contains("");

        Ok(current_state)
        // }

        // Fallback: Check SelectionItemPattern as some controls might use it
        // if let Ok(selection_pattern) = self
        //     .element
        //     .0
        //     .get_pattern::<patterns::UISelectionItemPattern>()
        // {
        //     if let Ok(is_selected) = selection_pattern.is_selected() {
        //         return Ok(is_selected);
        //     }
        // }

        // Fallback: Check name for keywords if no pattern is definitive
        // if let Ok(name) = self.element.0.get_name() {
        //     let name_lower = name.to_lowercase();
        //     if name_lower.contains("checked")
        //         || name_lower.contains("selected")
        //         || name_lower.contains("toggled")
        //     {
        //         return Ok(true);
        //     }
        //     if name_lower.contains("unchecked") || name_lower.contains("not selected") {
        //         return Ok(false);
        //     }
        // }

        // Err(AutomationError::UnsupportedOperation(format!(
        //     "Element '{}' does not support TogglePattern or provide state information. This element is not a toggleable control. Use 'is_selected' for selection states.",
        //     self.element.0.get_name().unwrap_or_default()
        // )))
    }

    fn set_toggled(&self, state: bool) -> Result<(), AutomationError> {
        // First, try to use the TogglePattern, which is the primary pattern for toggleable controls.
        if let Ok(toggle_pattern) = self.element.0.get_pattern::<patterns::UITogglePattern>() {
            if let Ok(current_state_enum) = toggle_pattern.get_toggle_state() {
                // let current_state = current_state_enum == uiautomation::types::ToggleState::On;

                // VERY DIRTY HACK BECAUSE TOGGLE STATE DOES NOT WORK
                // CHECK IF THERE IS [] IN THE NAME OF THE CONTROL
                let current_state = self.element.0.get_name().unwrap_or_default().contains("");
                debug!("Current state: {current_state}, desired state: {state}, enum: {current_state_enum} name: {}", self.element.0.get_name().unwrap_or_default());

                if current_state != state {
                    // Only toggle if the state is different.
                    return toggle_pattern.toggle().map_err(|e| {
                        AutomationError::PlatformError(format!("Failed to toggle: {e}"))
                    });
                } else {
                    // Already in the desired state.
                    return Ok(());
                }
            }
        }

        // As a fallback, try to use SelectionItemPattern, as some controls report toggle state via selection.
        debug!("Element does not support TogglePattern or failed to get state, falling back to SelectionItemPattern for set_toggled");
        if self
            .element
            .0
            .get_pattern::<patterns::UISelectionItemPattern>()
            .is_ok()
        {
            return self.set_selected(state);
        }

        Err(AutomationError::UnsupportedOperation(format!(
            "Element '{}' supports neither TogglePattern nor SelectionItemPattern for setting toggle state. This element may not be a standard toggleable control.",
            self.element.0.get_name().unwrap_or_default()
        )))
    }

    fn get_range_value(&self) -> Result<f64, AutomationError> {
        let range_pattern = self
            .element
            .0
            .get_pattern::<patterns::UIRangeValuePattern>()
            .map_err(|e| {
                let error_str = e.to_string();
                if error_str.contains("not support") || error_str.contains("UIA_E_ELEMENTNOTAVAILABLE") {
                    AutomationError::UnsupportedOperation(format!(
                        "Element does not support RangeValuePattern. This is not a range control (slider, progress bar, etc.). Error: {error_str}"
                    ))
                } else {
                    AutomationError::PlatformError(format!("Failed to get RangeValuePattern: {e}"))
                }
            })?;
        range_pattern
            .get_value()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get range value: {e}")))
    }

    fn set_range_value(&self, value: f64) -> Result<(), AutomationError> {
        self.focus()?; // Always focus first for keyboard interaction

        let range_pattern = self
            .element
            .0
            .get_pattern::<patterns::UIRangeValuePattern>()
            .map_err(|e| {
                let error_str = e.to_string();
                if error_str.contains("not support") || error_str.contains("UIA_E_ELEMENTNOTAVAILABLE") {
                    AutomationError::UnsupportedOperation(format!(
                        "Element does not support RangeValuePattern. This is not a range control (slider, progress bar, etc.). Try using keyboard arrows or mouse drag for custom sliders. Error: {error_str}"
                    ))
                } else {
                    AutomationError::PlatformError(format!("Failed to get RangeValuePattern: {e}"))
                }
            })?;

        // Try setting value directly first, as it's the most efficient method.
        if range_pattern.set_value(value).is_ok() {
            // Optional: Short sleep to allow UI to update.
            std::thread::sleep(std::time::Duration::from_millis(100));
            if let Ok(new_value) = range_pattern.get_value() {
                // Use a tolerance for floating-point comparison.
                if (new_value - value).abs() < 1.0 {
                    debug!("Direct set_value for RangeValuePattern succeeded.");
                    return Ok(());
                }
                debug!(
                    "Direct set_value was inaccurate, new value: {}. Expected: {}",
                    new_value, value
                );
            }
        }

        // Fallback to keyboard simulation.
        debug!("Direct set_value for RangeValuePattern failed or was inaccurate, falling back to keyboard simulation.");

        let min_value = range_pattern
            .get_minimum()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get min value: {e}")))?;
        let max_value = range_pattern
            .get_maximum()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get max value: {e}")))?;

        let mut small_change = range_pattern.get_small_change().unwrap_or(0.0);

        if small_change <= 0.0 {
            debug!("Slider small_change is not positive, calculating fallback step.");
            let range = max_value - min_value;
            if range > 0.0 {
                // Use 1% of the range as a reasonable step, or a minimum of 1.0
                small_change = (range / 100.0).max(1.0);
            } else {
                // If range is zero or negative, we can't do much.
                return Err(AutomationError::PlatformError(
                    "Slider range is zero or negative, cannot use keyboard fallback.".to_string(),
                ));
            }
        }

        // Clamp the target value to be within the allowed range.
        let target_value = value.clamp(min_value, max_value);

        debug!(
            "Slider properties: min={}, max={}, small_change={}, target={}",
            min_value, max_value, small_change, target_value
        );

        // Decide whether to move from min or max.
        let from_min_dist = (target_value - min_value).abs();
        let from_max_dist = (max_value - target_value).abs();

        if from_min_dist <= from_max_dist {
            // Go to min and step up.
            debug!("Moving from min. Resetting to HOME.");
            self.press_key("{home}")?;
            std::thread::sleep(std::time::Duration::from_millis(50));
            let num_steps = (from_min_dist / small_change).round() as u32;
            debug!(
                "Pressing RIGHT {} times to reach {}",
                num_steps, target_value
            );
            for i in 0..num_steps {
                self.press_key("{right}")?;
                std::thread::sleep(std::time::Duration::from_millis(10));
                debug!("Step {}/{}: Pressed RIGHT", i + 1, num_steps);
            }
        } else {
            // Go to max and step down.
            debug!("Moving from max. Resetting to END.");
            self.press_key("{end}")?;
            std::thread::sleep(std::time::Duration::from_millis(50));
            let num_steps = (from_max_dist / small_change).round() as u32;
            debug!(
                "Pressing LEFT {} times to reach {}",
                num_steps, target_value
            );
            for i in 0..num_steps {
                self.press_key("{left}")?;
                std::thread::sleep(std::time::Duration::from_millis(10));
                debug!("Step {}/{}: Pressed LEFT", i + 1, num_steps);
            }
        }

        Ok(())
    }

    fn is_selected(&self) -> Result<bool, AutomationError> {
        // First, try SelectionItemPattern, which is the primary meaning of "selected".
        if let Ok(selection_item_pattern) = self
            .element
            .0
            .get_pattern::<patterns::UISelectionItemPattern>()
        {
            if selection_item_pattern.is_selected().unwrap_or(false) {
                return Ok(true);
            }
        }

        // As a fallback for convenience, check if it's a "toggled" control like a checkbox.
        if let Ok(toggle_pattern) = self.element.0.get_pattern::<patterns::UITogglePattern>() {
            if let Ok(state) = toggle_pattern.get_toggle_state() {
                if state == uiautomation::types::ToggleState::On {
                    return Ok(true);
                }
            }
        }

        // Final fallback: for some controls (like calendar dates), selection is indicated by focus.
        if self.is_focused().unwrap_or(false) {
            return Ok(true);
        }

        // If we've reached here, none of the positive checks passed.
        // Return false if any of the patterns were supported, otherwise error.
        if self
            .element
            .0
            .get_pattern::<patterns::UISelectionItemPattern>()
            .is_ok()
            || self
                .element
                .0
                .get_pattern::<patterns::UITogglePattern>()
                .is_ok()
        {
            Ok(false)
        } else {
            // Fallback: Check name for keywords if no pattern is definitive
            if let Ok(name) = self.element.0.get_name() {
                let name_lower = name.to_lowercase();
                if name_lower.contains("checked") || name_lower.contains("selected") {
                    return Ok(true);
                }
                if name_lower.contains("unchecked") || name_lower.contains("not selected") {
                    return Ok(false);
                }
            }
            Err(AutomationError::UnsupportedOperation(
                "Element supports neither SelectionItemPattern nor TogglePattern, and is not focused."
                    .to_string(),
            ))
        }
    }

    fn set_selected(&self, state: bool) -> Result<(), AutomationError> {
        // First, try SelectionItemPattern, which is the primary meaning of "selected".
        if let Ok(selection_item_pattern) = self
            .element
            .0
            .get_pattern::<patterns::UISelectionItemPattern>()
        {
            let is_currently_selected = selection_item_pattern.is_selected().unwrap_or(false);

            if state && !is_currently_selected {
                // If we need to select it, and it's not selected yet.
                return selection_item_pattern.select().map_err(|e| {
                    AutomationError::PlatformError(format!("Failed to select item: {e}"))
                });
            } else if !state && is_currently_selected {
                // If we need to deselect it, and it's currently selected.
                // This is for multi-select controls; for single-select this may fail.
                return selection_item_pattern.remove_from_selection().map_err(|e| {
                    AutomationError::PlatformError(format!(
                        "Failed to remove item from selection. This might be a single-select control that doesn't support deselection: {e}"
                    ))
                });
            }
            return Ok(()); // Already in the desired state.
        }

        // As a fallback for convenience, check if it's a "toggled" control like a checkbox.
        if self
            .element
            .0
            .get_pattern::<patterns::UITogglePattern>()
            .is_ok()
        {
            debug!("Element doesn't support SelectionItemPattern, falling back to TogglePattern");
            return self.set_toggled(state);
        }

        // Final fallback: if we want to select, try clicking.
        if state {
            debug!("Element supports neither SelectionItemPattern nor TogglePattern, falling back to click");
            return self.click().map(|_| ());
        }

        Err(AutomationError::UnsupportedOperation(
            "Element cannot be deselected as it supports neither SelectionItemPattern nor TogglePattern. For radio buttons and list items, deselection typically happens by selecting another item.".to_string(),
        ))
    }
}

impl WindowsUIElement {
    // No more CDP stuff - using direct browser automation now
}
