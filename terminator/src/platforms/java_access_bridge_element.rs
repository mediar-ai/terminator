//! UIElement implementation for Java Access Bridge elements
//! 
//! This module provides a UIElement implementation that wraps Java Access Bridge
//! accessible contexts, allowing them to be used seamlessly with the terminator API.

use std::collections::HashMap;
use std::sync::Arc;
use std::ffi::c_void;
use crate::element::UIElementImpl;
use crate::platforms::java_access_bridge::{
    JavaAccessBridge, AccessibleContext, VmId, AccessibleContextInfo, wide_string_to_string
};
use crate::{AutomationError, ClickResult, UIElementAttributes, ScreenshotResult};
use image::DynamicImage;
use tracing::{debug, error, warn};
use windows::Win32::Foundation::HWND;

/// Java Access Bridge element implementation
pub struct JavaAccessBridgeElement {
    /// The Java VM ID
    vm_id: VmId,
    /// The accessible context handle
    accessible_context: AccessibleContext,
    /// Window handle (if available)
    hwnd: Option<HWND>,
    /// Cached context info to avoid repeated API calls
    cached_info: Option<AccessibleContextInfo>,
    /// Reference to the Java Access Bridge instance
    jab: Arc<std::sync::Mutex<JavaAccessBridge>>,
}

impl JavaAccessBridgeElement {
    /// Create a new Java Access Bridge element
    pub fn new(
        vm_id: VmId,
        accessible_context: AccessibleContext,
        hwnd: Option<HWND>,
    ) -> Result<Self, AutomationError> {
        let jab = JavaAccessBridge::get_instance()?;
        
        Ok(Self {
            vm_id,
            accessible_context,
            hwnd,
            cached_info: None,
            jab,
        })
    }
    
    /// Create from window handle
    pub fn from_hwnd(hwnd: HWND) -> Result<Self, AutomationError> {
        let jab = JavaAccessBridge::get_instance()?;
        let jab_locked = jab.lock().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to lock JAB: {}", e))
        })?;
        
        // Check if this is a Java window
        let hwnd_ptr = hwnd.0 as *mut c_void;
        if !jab_locked.is_java_window(hwnd_ptr) {
            return Err(AutomationError::PlatformError(
                "Window is not a Java application".to_string(),
            ));
        }
        
        // Get accessible context from window handle
        let (vm_id, accessible_context) = jab_locked.get_accessible_context_from_hwnd(hwnd_ptr)?;
        
        drop(jab_locked);
        
        Self::new(vm_id, accessible_context, Some(hwnd))
    }
    
    /// Get or cache the accessible context info
    fn get_context_info(&mut self) -> Result<&AccessibleContextInfo, AutomationError> {
        if self.cached_info.is_none() {
            let jab_locked = self.jab.lock().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to lock JAB: {}", e))
            })?;
            
            let info = jab_locked.get_accessible_context_info(self.vm_id, self.accessible_context)?;
            self.cached_info = Some(info);
        }
        
        Ok(self.cached_info.as_ref().unwrap())
    }
    
    /// Get child elements
    pub fn get_children(&self) -> Result<Vec<JavaAccessBridgeElement>, AutomationError> {
        let jab_locked = self.jab.lock().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to lock JAB: {}", e))
        })?;
        
        let info = jab_locked.get_accessible_context_info(self.vm_id, self.accessible_context)?;
        let mut children = Vec::new();
        
        for i in 0..info.children_count {
            match jab_locked.get_accessible_child_from_context(self.vm_id, self.accessible_context, i) {
                Ok(child_ac) => {
                    match Self::new(self.vm_id, child_ac, self.hwnd) {
                        Ok(child_element) => children.push(child_element),
                        Err(e) => {
                            warn!("Failed to create child element at index {}: {}", i, e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to get child at index {}: {}", i, e);
                }
            }
        }
        
        Ok(children)
    }
    
    /// Get parent element
    pub fn get_parent(&self) -> Result<JavaAccessBridgeElement, AutomationError> {
        let jab_locked = self.jab.lock().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to lock JAB: {}", e))
        })?;
        
        let parent_ac = jab_locked.get_accessible_parent_from_context(self.vm_id, self.accessible_context)?;
        
        drop(jab_locked);
        
        Self::new(self.vm_id, parent_ac, self.hwnd)
    }
}

impl UIElementImpl for JavaAccessBridgeElement {
    fn get_name(&self) -> Result<String, AutomationError> {
        let jab_locked = self.jab.lock().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to lock JAB: {}", e))
        })?;
        
        let info = jab_locked.get_accessible_context_info(self.vm_id, self.accessible_context)?;
        Ok(wide_string_to_string(&info.name))
    }

    fn get_role(&self) -> Result<String, AutomationError> {
        let jab_locked = self.jab.lock().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to lock JAB: {}", e))
        })?;
        
        let info = jab_locked.get_accessible_context_info(self.vm_id, self.accessible_context)?;
        Ok(wide_string_to_string(&info.role))
    }

    fn get_value(&self) -> Result<String, AutomationError> {
        // For Java Access Bridge, we use the description as the value
        let jab_locked = self.jab.lock().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to lock JAB: {}", e))
        })?;
        
        let info = jab_locked.get_accessible_context_info(self.vm_id, self.accessible_context)?;
        Ok(wide_string_to_string(&info.description))
    }

    fn get_help_text(&self) -> Result<String, AutomationError> {
        // Java Access Bridge doesn't have separate help text, use description
        self.get_value()
    }

    fn get_control_type(&self) -> Result<String, AutomationError> {
        self.get_role()
    }

    fn get_class_name(&self) -> Result<String, AutomationError> {
        // Java Access Bridge doesn't expose class names directly
        Ok("JavaComponent".to_string())
    }

    fn get_automation_id(&self) -> Result<String, AutomationError> {
        // Use the accessible context as a unique identifier
        Ok(format!("jab_{}_{}", self.vm_id, self.accessible_context))
    }

    fn get_process_id(&self) -> Result<i32, AutomationError> {
        // Return the VM ID as process identifier
        Ok(self.vm_id)
    }

    fn get_bounding_rectangle(&self) -> Result<(i32, i32, i32, i32), AutomationError> {
        let jab_locked = self.jab.lock().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to lock JAB: {}", e))
        })?;
        
        let info = jab_locked.get_accessible_context_info(self.vm_id, self.accessible_context)?;
        Ok((info.x, info.y, info.width, info.height))
    }

    fn get_children_count(&self) -> Result<i32, AutomationError> {
        let jab_locked = self.jab.lock().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to lock JAB: {}", e))
        })?;
        
        let info = jab_locked.get_accessible_context_info(self.vm_id, self.accessible_context)?;
        Ok(info.children_count)
    }

    fn click(&self) -> Result<ClickResult, AutomationError> {
        // Get the center point of the element
        let (x, y, width, height) = self.get_bounding_rectangle()?;
        let center_x = x + width / 2;
        let center_y = y + height / 2;
        
        // Perform click using Windows API
        self.click_at_point(center_x, center_y)
    }

    fn click_at_point(&self, x: i32, y: i32) -> Result<ClickResult, AutomationError> {
        use windows::Win32::UI::WindowsAndMessaging::{SetCursorPos, SendInput, INPUT, INPUT_MOUSE};
        use windows::Win32::UI::WindowsAndMessaging::{MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP};
        
        debug!("Clicking at point ({}, {}) on Java element", x, y);
        
        // Move cursor to position
        unsafe {
            SetCursorPos(x, y).map_err(|e| {
                AutomationError::ActionFailed(format!("Failed to set cursor position: {}", e))
            })?;
        }
        
        // Perform mouse click
        let mut inputs = [
            INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: windows::Win32::UI::WindowsAndMessaging::INPUT_0 {
                    mi: windows::Win32::UI::WindowsAndMessaging::MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_LEFTDOWN,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: windows::Win32::UI::WindowsAndMessaging::INPUT_0 {
                    mi: windows::Win32::UI::WindowsAndMessaging::MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_LEFTUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
        ];
        
        unsafe {
            let result = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
            if result != 2 {
                return Err(AutomationError::ActionFailed(
                    "Failed to send mouse input".to_string(),
                ));
            }
        }
        
        Ok(ClickResult {
            success: true,
            error_message: None,
        })
    }

    fn double_click(&self) -> Result<ClickResult, AutomationError> {
        // Perform two clicks in quick succession
        self.click()?;
        std::thread::sleep(std::time::Duration::from_millis(50));
        self.click()
    }

    fn right_click(&self) -> Result<ClickResult, AutomationError> {
        use windows::Win32::UI::WindowsAndMessaging::{SetCursorPos, SendInput, INPUT, INPUT_MOUSE};
        use windows::Win32::UI::WindowsAndMessaging::{MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP};
        
        let (x, y, width, height) = self.get_bounding_rectangle()?;
        let center_x = x + width / 2;
        let center_y = y + height / 2;
        
        debug!("Right clicking at point ({}, {}) on Java element", center_x, center_y);
        
        // Move cursor to position
        unsafe {
            SetCursorPos(center_x, center_y).map_err(|e| {
                AutomationError::ActionFailed(format!("Failed to set cursor position: {}", e))
            })?;
        }
        
        // Perform right mouse click
        let mut inputs = [
            INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: windows::Win32::UI::WindowsAndMessaging::INPUT_0 {
                    mi: windows::Win32::UI::WindowsAndMessaging::MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_RIGHTDOWN,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: windows::Win32::UI::WindowsAndMessaging::INPUT_0 {
                    mi: windows::Win32::UI::WindowsAndMessaging::MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_RIGHTUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
        ];
        
        unsafe {
            let result = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
            if result != 2 {
                return Err(AutomationError::ActionFailed(
                    "Failed to send right mouse input".to_string(),
                ));
            }
        }
        
        Ok(ClickResult {
            success: true,
            error_message: None,
        })
    }

    fn scroll_into_view(&self) -> Result<(), AutomationError> {
        // Java Access Bridge doesn't have direct scroll support
        // We could implement this by finding scrollable parent and using scroll patterns
        debug!("Scroll into view not fully implemented for Java Access Bridge elements");
        Ok(())
    }

    fn type_text(&self, text: &str) -> Result<(), AutomationError> {
        use windows::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_KEYBOARD};
        use windows::Win32::UI::Input::KeyboardAndMouse::{KEYEVENTF_UNICODE, VIRTUAL_KEY};
        
        debug!("Typing text '{}' to Java element", text);
        
        // First click to focus the element
        self.click()?;
        
        // Small delay to ensure focus
        std::thread::sleep(std::time::Duration::from_millis(100));
        
        // Type each character
        for ch in text.chars() {
            let input = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    ki: windows::Win32::UI::Input::KeyboardAndMouse::KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: ch as u16,
                        dwFlags: KEYEVENTF_UNICODE,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            
            unsafe {
                SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
            }
            
            // Small delay between characters
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        
        Ok(())
    }

    fn clear_text(&self) -> Result<(), AutomationError> {
        // Select all and delete
        self.click()?;
        std::thread::sleep(std::time::Duration::from_millis(50));
        
        // Ctrl+A to select all
        use windows::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_KEYBOARD};
        use windows::Win32::UI::Input::KeyboardAndMouse::{KEYEVENTF_KEYUP, VK_CONTROL, VK_A, VK_DELETE};
        
        let inputs = [
            // Press Ctrl
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    ki: windows::Win32::UI::Input::KeyboardAndMouse::KEYBDINPUT {
                        wVk: VK_CONTROL,
                        wScan: 0,
                        dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::KEYEVENTF_KEYUP.into(),
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            // Press A
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    ki: windows::Win32::UI::Input::KeyboardAndMouse::KEYBDINPUT {
                        wVk: VK_A,
                        wScan: 0,
                        dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::KEYEVENTF_KEYUP.into(),
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            // Release A
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    ki: windows::Win32::UI::Input::KeyboardAndMouse::KEYBDINPUT {
                        wVk: VK_A,
                        wScan: 0,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            // Release Ctrl
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    ki: windows::Win32::UI::Input::KeyboardAndMouse::KEYBDINPUT {
                        wVk: VK_CONTROL,
                        wScan: 0,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            // Press Delete
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    ki: windows::Win32::UI::Input::KeyboardAndMouse::KEYBDINPUT {
                        wVk: VK_DELETE,
                        wScan: 0,
                        dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::KEYEVENTF_KEYUP.into(),
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            // Release Delete
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    ki: windows::Win32::UI::Input::KeyboardAndMouse::KEYBDINPUT {
                        wVk: VK_DELETE,
                        wScan: 0,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
        ];
        
        unsafe {
            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }
        
        Ok(())
    }

    fn get_text(&self) -> Result<String, AutomationError> {
        // For Java Access Bridge, try to get text from name or description
        let name = self.get_name().unwrap_or_default();
        if !name.is_empty() {
            return Ok(name);
        }
        
        self.get_value()
    }

    fn is_enabled(&self) -> Result<bool, AutomationError> {
        let jab_locked = self.jab.lock().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to lock JAB: {}", e))
        })?;
        
        let info = jab_locked.get_accessible_context_info(self.vm_id, self.accessible_context)?;
        let states = wide_string_to_string(&info.states);
        
        // Check if "enabled" is in the states string
        Ok(!states.to_lowercase().contains("disabled"))
    }

    fn is_visible(&self) -> Result<bool, AutomationError> {
        let jab_locked = self.jab.lock().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to lock JAB: {}", e))
        })?;
        
        let info = jab_locked.get_accessible_context_info(self.vm_id, self.accessible_context)?;
        let states = wide_string_to_string(&info.states);
        
        // Check if "visible" is in the states string
        Ok(states.to_lowercase().contains("visible") || states.to_lowercase().contains("showing"))
    }

    fn focus(&self) -> Result<(), AutomationError> {
        // For Java Access Bridge, we can try to focus by clicking
        self.click()?;
        Ok(())
    }

    fn set_value(&self, value: &str) -> Result<(), AutomationError> {
        // Clear existing text and type new value
        self.clear_text()?;
        self.type_text(value)
    }

    fn expand(&self) -> Result<(), AutomationError> {
        // Java Access Bridge supports expand/collapse patterns
        // This would need to be implemented with proper pattern support
        Err(AutomationError::UnsupportedOperation(
            "Expand not implemented for Java Access Bridge elements".to_string(),
        ))
    }

    fn collapse(&self) -> Result<(), AutomationError> {
        // Java Access Bridge supports expand/collapse patterns
        // This would need to be implemented with proper pattern support
        Err(AutomationError::UnsupportedOperation(
            "Collapse not implemented for Java Access Bridge elements".to_string(),
        ))
    }

    fn select(&self) -> Result<(), AutomationError> {
        // For selection, try clicking the element
        self.click()?;
        Ok(())
    }

    fn get_selected(&self) -> Result<bool, AutomationError> {
        let jab_locked = self.jab.lock().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to lock JAB: {}", e))
        })?;
        
        let info = jab_locked.get_accessible_context_info(self.vm_id, self.accessible_context)?;
        let states = wide_string_to_string(&info.states);
        
        Ok(states.to_lowercase().contains("selected"))
    }

    fn capture_screenshot(&self) -> Result<ScreenshotResult, AutomationError> {
        // Get element bounds
        let (x, y, width, height) = self.get_bounding_rectangle()?;
        
        // Use xcap to capture the screen region
        use xcap::Window;
        
        // For now, capture the entire screen and crop to element bounds
        // This is a basic implementation - could be optimized
        let windows = Window::all().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get windows: {}", e))
        })?;
        
        if let Some(window) = windows.first() {
            let image = window.capture_image().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to capture window: {}", e))
            })?;
            
            // Crop to element bounds
            let cropped = image.crop_imm(
                x.max(0) as u32,
                y.max(0) as u32,
                width.max(1) as u32,
                height.max(1) as u32,
            );
            
            Ok(ScreenshotResult {
                image: DynamicImage::ImageRgba8(cropped),
                width: width as u32,
                height: height as u32,
            })
        } else {
            Err(AutomationError::PlatformError(
                "No windows available for screenshot".to_string(),
            ))
        }
    }

    fn get_all_attributes(&self) -> Result<UIElementAttributes, AutomationError> {
        let mut attributes = HashMap::new();
        
        // Add Java Access Bridge specific attributes
        attributes.insert("vm_id".to_string(), self.vm_id.to_string());
        attributes.insert("accessible_context".to_string(), self.accessible_context.to_string());
        
        let jab_locked = self.jab.lock().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to lock JAB: {}", e))
        })?;
        
        let info = jab_locked.get_accessible_context_info(self.vm_id, self.accessible_context)?;
        
        attributes.insert("name".to_string(), wide_string_to_string(&info.name));
        attributes.insert("description".to_string(), wide_string_to_string(&info.description));
        attributes.insert("role".to_string(), wide_string_to_string(&info.role));
        attributes.insert("states".to_string(), wide_string_to_string(&info.states));
        attributes.insert("index_in_parent".to_string(), info.index_in_parent.to_string());
        attributes.insert("children_count".to_string(), info.children_count.to_string());
        attributes.insert("x".to_string(), info.x.to_string());
        attributes.insert("y".to_string(), info.y.to_string());
        attributes.insert("width".to_string(), info.width.to_string());
        attributes.insert("height".to_string(), info.height.to_string());
        attributes.insert("accessible_component".to_string(), info.accessible_component.to_string());
        attributes.insert("accessible_action".to_string(), info.accessible_action.to_string());
        attributes.insert("accessible_selection".to_string(), info.accessible_selection.to_string());
        attributes.insert("accessible_text".to_string(), info.accessible_text.to_string());
        attributes.insert("accessible_interfaces".to_string(), info.accessible_interfaces.to_string());
        
        Ok(UIElementAttributes { attributes })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Drop for JavaAccessBridgeElement {
    fn drop(&mut self) {
        // Release the Java object to prevent memory leaks
        if let Ok(jab_locked) = self.jab.lock() {
            jab_locked.release_java_object(self.vm_id, self.accessible_context);
        }
    }
}