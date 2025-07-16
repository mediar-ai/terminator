use crate::platforms::AccessibilityEngine;
use crate::{
    element::UIElementImpl, AutomationError, Locator, Selector, UIElement, UIElementAttributes,
};
use crate::{ClickResult, ScreenshotResult};

use accessibility::AXUIElementAttributes;
use accessibility::{AXAttribute, AXUIElement};
use accessibility_sys::error_string;
use anyhow::Result;
use core_foundation::array::{
    CFArrayGetCount, CFArrayGetTypeID, CFArrayGetValueAtIndex, __CFArray,
};
use core_foundation::base::{CFGetTypeID, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use core_graphics::display::{CGPoint, CGSize};
use core_graphics::event::{CGEvent, CGEventFlags, CGKeyCode};
use core_graphics::event_source::CGEventSource;
use image::{DynamicImage, ImageBuffer, Rgba};
use serde_json::{self, Value};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, instrument, warn, Level};
use uni_ocr::{OcrEngine, OcrProvider};

use super::tree_search::ElementsCollectorWithWindows;

// Import the C function for setting attributes
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementSetAttributeValue(
        element: *mut ::std::os::raw::c_void,
        attribute: *const ::std::os::raw::c_void,
        value: *const ::std::os::raw::c_void,
    ) -> i32;
}

// Add these extern "C" declarations if not already present
unsafe extern "C" {
    fn AXValueGetValue(
        value: *const ::std::os::raw::c_void,
        type_: u32,
        out: *mut ::std::os::raw::c_void,
    ) -> i32;
}

// Add these constant definitions instead - these are the official values from Apple's headers
const K_AXVALUE_CGPOINT_TYPE: u32 = 1;
const K_AXVALUE_CGSIZE_TYPE: u32 = 2;

// Add these constant definitions for key codes
const KEY_RETURN: u16 = 36;
const KEY_TAB: u16 = 48;
const KEY_SPACE: u16 = 49;
const KEY_DELETE: u16 = 51;
const KEY_ESCAPE: u16 = 53;
const KEY_ARROW_LEFT: u16 = 123;
const KEY_ARROW_RIGHT: u16 = 124;
const KEY_ARROW_DOWN: u16 = 125;
const KEY_ARROW_UP: u16 = 126;

// Add these constants for modifier keys
const MODIFIER_COMMAND: CGEventFlags = CGEventFlags::CGEventFlagCommand;
const MODIFIER_SHIFT: CGEventFlags = CGEventFlags::CGEventFlagShift;
const MODIFIER_OPTION: CGEventFlags = CGEventFlags::CGEventFlagAlternate;
const MODIFIER_CONTROL: CGEventFlags = CGEventFlags::CGEventFlagControl;
const MODIFIER_FN: CGEventFlags = CGEventFlags::CGEventFlagSecondaryFn;

// Thread-safe wrapper for AXUIElement
#[derive(Clone)]
pub struct ThreadSafeAXUIElement(Arc<AXUIElement>);

// Implement Send and Sync for our wrapper
// SAFETY: AXUIElement is safe to send and share between threads as Apple's
// accessibility API is designed to be called from any thread. The underlying
// Core Foundation objects manage their own thread safety.
unsafe impl Send for ThreadSafeAXUIElement {}
unsafe impl Sync for ThreadSafeAXUIElement {}

impl ThreadSafeAXUIElement {
    pub fn new(element: AXUIElement) -> Self {
        Self(Arc::new(element))
    }

    pub fn system_wide() -> Self {
        Self(Arc::new(AXUIElement::system_wide()))
    }

    pub fn application(pid: i32) -> Self {
        Self(Arc::new(AXUIElement::application(pid)))
    }

    pub fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

// Implement Debug
impl fmt::Debug for ThreadSafeAXUIElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ThreadSafeAXUIElement")
            .field(&"<AXUIElement>")
            .finish()
    }
}

pub struct MacOSEngine {
    system_wide: ThreadSafeAXUIElement,
    use_background_apps: bool,
    activate_app: bool,
}

impl MacOSEngine {
    pub fn new(use_background_apps: bool, activate_app: bool) -> Result<Self, AutomationError> {
        // Check accessibility permissions using FFI directly
        // Since accessibility::AXIsProcessTrustedWithOptions is not available
        let accessibility_enabled = unsafe {
            use core_foundation::dictionary::CFDictionaryRef;

            #[link(name = "ApplicationServices", kind = "framework")]
            unsafe extern "C" {
                fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
            }

            let check_attr = CFString::new("AXTrustedCheckOptionPrompt");
            let options = CFDictionary::from_CFType_pairs(&[(
                check_attr.as_CFType(),
                CFBoolean::true_value().as_CFType(),
            )])
            .as_concrete_TypeRef();

            AXIsProcessTrustedWithOptions(options)
        };

        if !accessibility_enabled {
            return Err(AutomationError::PermissionDenied(
                "Accessibility permissions not granted".to_string(),
            ));
        }

        Ok(Self {
            system_wide: ThreadSafeAXUIElement::system_wide(),
            use_background_apps,
            activate_app,
        })
    }

    // Helper to convert ThreadSafeAXUIElement to our UIElement
    #[instrument(skip(self, ax_element))]
    fn wrap_element(&self, ax_element: ThreadSafeAXUIElement) -> UIElement {
        debug!("[macOS][wrap_element] ax_element: {:?}", ax_element);

        // Try to check element validity by first checking if "AXRole" is present in attribute names
        match ax_element.0.attribute_names() {
            Ok(attr_names) => {
                // ItemRef<CFString> does not have as_str(), so compare using .to_string()
                let has_role = attr_names.iter().any(|attr| attr.to_string() == "AXRole");
                // println!("[macOS][wrap_element] attr_names: {:?}", attr_names);
                if has_role {
                    // Try to get the role if the attribute exists
                    if let Err(e) = ax_element.0.role() {
                        if let accessibility::Error::Ax(code) = e {
                            if code != -25204 {
                                // kAXErrorNoValue
                                let err_str = error_string(code);
                                debug!(
                                    "Warning: Potentially invalid AXUIElement: {:?} (error: {})",
                                    e, err_str
                                );
                            }
                        } else {
                            debug!("Warning: Potentially invalid AXUIElement: {:?}", e);
                        }
                    }
                } else {
                    debug!(
                        "AXUIElement does not have 'AXRole' attribute. Attributes: {:?}",
                        attr_names
                    );
                    // If it doesn't have AXRole, we still consider it valid for wrapping,
                    // but log for debugging.
                }
            }
            Err(e) => {
                let err_str = if let accessibility::Error::Ax(code) = e {
                    error_string(code)
                } else {
                    "<not an AX error>"
                };
                debug!(
                    "Failed to get attribute names for AXUIElement: {:?} (error: {}). Wrapping anyway.",
                    e, err_str
                );
                // If we can't get attribute names, be conservative and allow wrapping,
                // but log for debugging.
            }
        }

        UIElement::new(Box::new(MacOSUIElement {
            element: ax_element,
            use_background_apps: self.use_background_apps,
            activate_app: self.activate_app,
        }))
    }

    // Add this new method to refresh the accessibility tree
    #[allow(clippy::unexpected_cfg_condition)]
    pub fn refresh_accessibility_tree(
        &self,
        app_name: Option<&str>,
    ) -> Result<(), AutomationError> {
        if !self.activate_app {
            return Ok(());
        }

        debug!("Refreshing accessibility tree");

        // If app name is provided, try to activate that app first
        if let Some(name) = app_name {
            unsafe {
                use objc::{class, msg_send, sel, sel_impl};

                let workspace_class = class!(NSWorkspace);
                let shared_workspace: *mut objc::runtime::Object =
                    msg_send![workspace_class, sharedWorkspace];
                let apps: *mut objc::runtime::Object =
                    msg_send![shared_workspace, runningApplications];
                let count: usize = msg_send![apps, count];

                for i in 0..count {
                    let app: *mut objc::runtime::Object = msg_send![apps, objectAtIndex:i];
                    let app_name_obj: *mut objc::runtime::Object = msg_send![app, localizedName];

                    if !app_name_obj.is_null() {
                        let app_name_str: &str = {
                            let nsstring = app_name_obj as *const objc::runtime::Object;
                            let bytes: *const std::os::raw::c_char =
                                msg_send![nsstring, UTF8String];
                            let len: usize = msg_send![nsstring, lengthOfBytesUsingEncoding:4]; // NSUTF8StringEncoding = 4
                            let bytes_slice = std::slice::from_raw_parts(bytes as *const u8, len);
                            std::str::from_utf8_unchecked(bytes_slice)
                        };

                        if app_name_str.to_lowercase() == name.to_lowercase() {
                            // Found the app, activate it
                            let _: () = msg_send![app, activateWithOptions:1]; // NSApplicationActivateIgnoringOtherApps = 1
                            debug!("Activated application: {}", name);

                            // Give the system a moment to update
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            break;
                        }
                    }
                }
            }
        }

        // Force a refresh of the system-wide element
        // This is a bit of a hack, but querying the system-wide element
        // can force the accessibility API to refresh its cache
        let _ = self.system_wide.0.attribute_names();

        Ok(())
    }

    pub fn focus_application_with_cache(
        &self,
        app_name: &str,
        app_cache: Option<&ThreadSafeAXUIElement>,
    ) -> Result<ThreadSafeAXUIElement, AutomationError> {
        debug!("focusing application: {}", app_name);

        // If we have a cached element, try to use it first
        if let Some(cached_element) = app_cache {
            debug!("using cached application element");

            // Check if cached element is still valid
            match cached_element.0.role() {
                Ok(role) if role.to_string() == "AXApplication" => {
                    // First try to activate the app using the cached element
                    unsafe {
                        use objc::{class, msg_send, sel, sel_impl};
                        let pid = get_pid_for_element(cached_element);

                        // Use NSRunningApplication API with the PID
                        let nsra_class = class!(NSRunningApplication);
                        let app: *mut objc::runtime::Object =
                            msg_send![nsra_class, runningApplicationWithProcessIdentifier:pid];
                        if !app.is_null() {
                            let _: () = msg_send![app, activateWithOptions:1];
                            debug!("Activated application using cached element");

                            // Success - return the cached element
                            return Ok(cached_element.clone());
                        }
                    }
                }
                _ => {
                    debug!("Cached element is no longer valid");
                    // Continue with normal flow if cached element is invalid
                }
            }
        }

        // Fallback to existing method
        self.refresh_accessibility_tree(Some(app_name))?;

        // Use the regular way to get application
        unsafe {
            use objc::{class, msg_send, sel, sel_impl};

            let workspace_class = class!(NSWorkspace);
            let shared_workspace: *mut objc::runtime::Object =
                msg_send![workspace_class, sharedWorkspace];
            let apps: *mut objc::runtime::Object = msg_send![shared_workspace, runningApplications];
            let count: usize = msg_send![apps, count];

            for i in 0..count {
                let app: *mut objc::runtime::Object = msg_send![apps, objectAtIndex:i];
                let app_name_obj: *mut objc::runtime::Object = msg_send![app, localizedName];

                if !app_name_obj.is_null() {
                    let app_name_str: &str = {
                        let nsstring = app_name_obj as *const objc::runtime::Object;
                        let bytes: *const std::os::raw::c_char = msg_send![nsstring, UTF8String];
                        let len: usize = msg_send![nsstring, lengthOfBytesUsingEncoding:4]; // NSUTF8StringEncoding = 4
                        let bytes_slice = std::slice::from_raw_parts(bytes as *const u8, len);
                        std::str::from_utf8_unchecked(bytes_slice)
                    };

                    if app_name_str.to_lowercase() == app_name.to_lowercase() {
                        let pid: i32 = msg_send![app, processIdentifier];
                        let ax_element = ThreadSafeAXUIElement::application(pid);

                        // Create new element to return
                        return Ok(ax_element);
                    }
                }
            }
        }

        // If we got here, we couldn't find the application
        Err(AutomationError::ElementNotFound(format!(
            "Application '{}' not found",
            app_name
        )))
    }
}

// Our concrete UIElement implementation for macOS
pub struct MacOSUIElement {
    element: ThreadSafeAXUIElement,
    use_background_apps: bool,
    activate_app: bool,
}

impl std::fmt::Debug for MacOSUIElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MacOSUIElement")
            .field("element", &self.element)
            .finish()
    }
}

impl MacOSUIElement {
    // Helper function to get the containing application
    fn get_application(&self) -> Option<MacOSUIElement> {
        // inefficient but works
        // Start with current element
        let mut current = self.element.clone();

        // Keep traversing up until we find an application or hit the top
        loop {
            // Check if current element is an application
            if let Ok(role) = current.0.role() {
                if role.to_string() == "AXApplication" {
                    return Some(MacOSUIElement {
                        element: current,
                        use_background_apps: self.use_background_apps,
                        activate_app: self.activate_app,
                    });
                }
            }

            // Try to get parent
            let parent_attr = AXAttribute::new(&CFString::new("AXParent"));
            match current.0.attribute(&parent_attr) {
                Ok(value) => {
                    if let Some(parent) = value.downcast::<AXUIElement>() {
                        current = ThreadSafeAXUIElement::new(parent);
                    } else {
                        return None;
                    }
                }
                Err(_) => return None,
            }
        }
    }

    // Add these methods to the MacOSUIElement impl block
    fn click_auto(&self) -> Result<ClickResult, AutomationError> {
        // only mouse simulation works on web and it seems function don't fail on web so let's try to detect if we are on web based on app
        // eg chrome, arc, safari, etc
        let app_name = self.get_application();

        if let Some(app) = app_name {
            // kind of dirty and hacky but it works
            let app_name = app.get_text(1).unwrap_or_default().to_lowercase();
            if app_name.contains("chrome")
                || app_name.contains("safari")
                || app_name.contains("arc")
                || app_name.contains("firefox")
                || app_name.contains("edge")
                || app_name.contains("brave")
                || app_name.contains("opera")
                || app_name.contains("vivaldi")
                || app_name.contains("microsoft edge")
            {
                return self.click_mouse_simulation();
            }
        }

        // 1. Try AXPress action first
        match self.click_press() {
            Ok(result) => return Ok(result),
            Err(e) => debug!("AXPress failed: {:?}, trying alternative methods", e),
        }

        // 2. Try AXClick action
        match self.click_accessibility_click() {
            Ok(result) => return Ok(result),
            Err(e) => debug!("AXClick failed: {:?}, trying alternative methods", e),
        }

        // 3. Try mouse simulation as last resort
        self.click_mouse_simulation()
    }

    fn click_press(&self) -> Result<ClickResult, AutomationError> {
        let press_attr = AXAttribute::new(&CFString::new("AXPress"));
        match self.element.0.perform_action(&press_attr.as_CFString()) {
            Ok(_) => {
                debug!("Successfully clicked element with AXPress");
                Ok(ClickResult {
                    method: "AXPress".to_string(),
                    coordinates: None,
                    details: "Used accessibility AXPress action".to_string(),
                })
            }
            Err(e) => Err(AutomationError::PlatformError(format!(
                "AXPress click failed: {:?}",
                e
            ))),
        }
    }

    fn click_accessibility_click(&self) -> Result<ClickResult, AutomationError> {
        let click_attr = AXAttribute::new(&CFString::new("AXClick"));
        match self.element.0.perform_action(&click_attr.as_CFString()) {
            Ok(_) => {
                debug!("Successfully clicked element with AXClick");
                Ok(ClickResult {
                    method: "AXClick".to_string(),
                    coordinates: None,
                    details: "Used accessibility AXClick action".to_string(),
                })
            }
            Err(e) => Err(AutomationError::PlatformError(format!(
                "AXClick click failed: {:?}",
                e
            ))),
        }
    }

    fn click_mouse_simulation(&self) -> Result<ClickResult, AutomationError> {
        match self.bounds() {
            Ok((x, y, width, height)) => {
                // Calculate center point of the element
                let center_x = x + width / 2.0;
                let center_y = y + height / 2.0;

                // Use CGEventCreateMouseEvent to simulate mouse click
                use core_graphics::event::{CGEvent, CGEventType, CGMouseButton};
                use core_graphics::event_source::CGEventSource;
                use core_graphics::geometry::CGPoint;

                let point = CGPoint::new(center_x, center_y);

                // Create event source
                let source = CGEventSource::new(
                    core_graphics::event_source::CGEventSourceStateID::HIDSystemState,
                )
                .map_err(|_| {
                    AutomationError::PlatformError("Failed to create event source".to_string())
                })?;

                // Move mouse to position
                let mouse_move = CGEvent::new_mouse_event(
                    source.clone(),
                    CGEventType::MouseMoved,
                    point,
                    CGMouseButton::Left,
                )
                .map_err(|_| {
                    AutomationError::PlatformError("Failed to create mouse move event".to_string())
                })?;
                mouse_move.post(core_graphics::event::CGEventTapLocation::HID);

                // Brief pause to allow UI to respond
                std::thread::sleep(std::time::Duration::from_millis(50));

                debug!("Mouse down at ({}, {})", center_x, center_y);

                // Mouse down
                let mouse_down = CGEvent::new_mouse_event(
                    source.clone(),
                    CGEventType::LeftMouseDown,
                    point,
                    CGMouseButton::Left,
                )
                .map_err(|_| {
                    AutomationError::PlatformError("Failed to create mouse down event".to_string())
                })?;
                mouse_down.post(core_graphics::event::CGEventTapLocation::HID);

                // Brief pause
                std::thread::sleep(std::time::Duration::from_millis(50));

                debug!("Mouse up at ({}, {})", center_x, center_y);

                // Mouse up
                let mouse_up = CGEvent::new_mouse_event(
                    source,
                    CGEventType::LeftMouseUp,
                    point,
                    CGMouseButton::Left,
                )
                .map_err(|_| {
                    AutomationError::PlatformError("Failed to create mouse up event".to_string())
                })?;
                mouse_up.post(core_graphics::event::CGEventTapLocation::HID);

                debug!(
                    "Performed simulated mouse click at ({}, {})",
                    center_x, center_y
                );

                Ok(ClickResult {
                    method: "MouseSimulation".to_string(),
                    coordinates: Some((center_x, center_y)),
                    details: format!(
                        "Used mouse simulation at coordinates ({:.1}, {:.1}), element bounds: ({:.1}, {:.1}, {:.1}, {:.1})",
                        center_x, center_y, x, y, width, height
                    ),
                })
            }
            Err(e) => Err(AutomationError::PlatformError(format!(
                "Failed to determine element bounds for click: {}",
                e
            ))),
        }
    }

    fn get_key_code(&self, key: &str) -> Result<u16, AutomationError> {
        let key_map: HashMap<&str, u16> = [
            ("return", KEY_RETURN),
            ("enter", KEY_RETURN),
            ("tab", KEY_TAB),
            ("space", KEY_SPACE),
            ("delete", KEY_DELETE),
            ("backspace", KEY_DELETE),
            ("esc", KEY_ESCAPE),
            ("escape", KEY_ESCAPE),
            ("left", KEY_ARROW_LEFT),
            ("right", KEY_ARROW_RIGHT),
            ("down", KEY_ARROW_DOWN),
            ("up", KEY_ARROW_UP),
        ]
        .iter()
        .cloned()
        .collect();

        key_map
            .get(key.to_lowercase().as_str())
            .copied()
            .ok_or_else(|| AutomationError::InvalidArgument(format!("Unknown key: {}", key)))
    }

    // Add a method to parse key combinations with modifiers
    fn parse_key_combination(
        &self,
        key_combo: &str,
    ) -> Result<(u16, CGEventFlags), AutomationError> {
        // Change Vec<&str> to Vec<String> to match the to_lowercase() output type
        let parts: Vec<String> = key_combo
            .split('+')
            .map(|s| s.trim().to_lowercase())
            .collect();

        if parts.is_empty() {
            return Err(AutomationError::InvalidArgument(
                "Empty key combination".to_string(),
            ));
        }

        // The last part is the actual key
        let key = &parts[parts.len() - 1];
        let key_code = self.get_key_code(key)?;

        // All parts except the last one are modifiers
        let mut flags = CGEventFlags::empty();
        for modifier in &parts[0..parts.len() - 1] {
            match modifier.as_str() {
                "cmd" | "command" => flags.insert(MODIFIER_COMMAND),
                "shift" => flags.insert(MODIFIER_SHIFT),
                "alt" | "option" => flags.insert(MODIFIER_OPTION),
                "ctrl" | "control" => flags.insert(MODIFIER_CONTROL),
                "fn" => flags.insert(MODIFIER_FN),
                _ => {
                    return Err(AutomationError::InvalidArgument(format!(
                        "Unknown modifier: {}",
                        modifier
                    )));
                }
            }
        }

        Ok((key_code, flags))
    }

    #[instrument(level = "debug", skip(self))]
    fn generate_stable_id(&self) -> String {
        let _span = tracing::span!(Level::DEBUG, "generate_stable_id").entered();
        let start = std::time::Instant::now();

        let mut hasher = DefaultHasher::new();

        // Fetch attributes only once
        let attributes = self.attributes();

        // Hash the element's role
        if let Ok(role) = self.element.0.role() {
            role.to_string().hash(&mut hasher);
        }

        // Hash the element's title/name
        if let Some(title) = attributes.name {
            title.hash(&mut hasher);
        }

        // Hash the element's value
        if let Some(value) = attributes.value {
            value.hash(&mut hasher);
        }

        // Hash the element's position to make it more unique
        if let Ok((x, y, _, _)) = self.bounds() {
            (x as i64).hash(&mut hasher);
            (y as i64).hash(&mut hasher);
        }

        // Hash parent information to make it more unique
        if let Ok(Some(parent)) = self.parent() {
            if let Some(parent_macos) = parent.as_any().downcast_ref::<MacOSUIElement>() {
                if let Ok(parent_role) = parent_macos.element.0.role() {
                    parent_role.to_string().hash(&mut hasher);
                }
            }

            // Also hash parent's label if available
            if let Some(parent_label) = attributes.label {
                parent_label.hash(&mut hasher);
            }
        }

        let result = format!("ax_{:x}", hasher.finish());
        let duration = start.elapsed();
        debug!(
            stable_id = %result,
            duration_ms = %duration.as_millis(),
            "Generated stable_id"
        );
        result
    }
}

impl UIElementImpl for MacOSUIElement {
    fn object_id(&self) -> usize {
        // Convert stable string ID to usize
        let stable_id = self.generate_stable_id();
        let mut hasher = DefaultHasher::new();
        stable_id.hash(&mut hasher);
        let id = hasher.finish() as usize;
        debug!("Stable ID: {:?}", stable_id);
        debug!("Hash: {:?}", id);
        id
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> Option<String> {
        Some(self.object_id().to_string())
    }

    fn role(&self) -> String {
        // Get the actual role
        let role = self
            .element
            .0
            .role()
            .map(|r| r.to_string())
            .unwrap_or_default();

        debug!("Original role from AXUIElement: {}", role);

        // Map macOS-specific roles to generic roles
        // TODO: why first? any issue?
        macos_role_to_generic_role(&role)
            .first()
            .unwrap_or(&role)
            .to_string()
    }

    fn attributes(&self) -> UIElementAttributes {
        let _span = tracing::span!(tracing::Level::DEBUG, "attributes").entered();
        let start = std::time::Instant::now();

        let role = self
            .element
            .0
            .role()
            .map(|r| r.to_string())
            .unwrap_or_default();
        debug!("Getting attributes for element: {:?}", role);

        let properties = HashMap::new();

        // Check if this is a window element first
        let is_window = role == "AXWindow";

        debug!("Element is_window: {}", is_window);

        // Special case for windows
        if is_window {
            debug!("Starting window-specific attribute collection");

            let mut attrs = UIElementAttributes {
                role: "window".to_string(),
                name: None,
                label: None,
                value: None,
                description: None,
                text: self.get_text(0).ok(),
                properties,
                is_keyboard_focusable: Some(false), // macos: not implemented
                is_focused: self.is_focused().ok(),
                enabled: self.is_enabled().ok(),
                bounds: None, // Will be populated by get_configurable_attributes if focusable
                is_toggled: None,
                is_selected: None,
                child_count: None,
                index_in_parent: None,
            };

            // Special handling for window title - try multiple attributes
            let title_attrs = [
                "AXTitle",
                "AXTitleUIElement",
                "AXDocument",
                "AXFilename",
                "AXName",
            ];

            debug!(
                "Attempting to find window title using {} attributes",
                title_attrs.len()
            );
            // Fetch attr_names once for window
            let attr_names = match self.element.0.attribute_names() {
                Ok(names) => names,
                Err(e) => {
                    debug!("Failed to get attribute names: {:?}", e);
                    let duration = start.elapsed();
                    debug!("Window attributes completed in {:?}", duration);
                    if duration > std::time::Duration::from_millis(100) {
                        warn!(
                            "⚠️ attributes() unusually long: window attributes collection took {:?} (>100ms) for element: {:?} (role: AXWindow)",
                            duration, self.element.0
                        );
                    }
                    return attrs;
                }
            };
            for title_attr_name in title_attrs {
                if attr_names.iter().any(|n| n.to_string() == title_attr_name) {
                    let title_attr = AXAttribute::new(&CFString::new(title_attr_name));
                    if let Ok(value) = self.element.0.attribute(&title_attr) {
                        if let Some(cf_string) = value.downcast_into::<CFString>() {
                            let title = cf_string.to_string();
                            attrs.name = Some(title.clone());
                            attrs.label = Some(title);
                            debug!(
                                "Found window title via {}: {:?}",
                                title_attr_name, attrs.name
                            );
                            break;
                        }
                    }
                }
            }

            // Try to get window position and size for debugging
            debug!("Checking for window position attribute");
            if attr_names.iter().any(|n| n.to_string() == "AXPosition") {
                let pos_attr = AXAttribute::new(&CFString::new("AXPosition"));
                if let Ok(_) = self.element.0.attribute(&pos_attr) {
                    debug!("Window has position attribute");
                }
            }

            // Try to get standard macOS window attributes
            let std_attrs = ["AXMinimized", "AXMain", "AXFocused"];
            debug!("Collecting {} standard window attributes", std_attrs.len());

            for attr_name in std_attrs {
                if attr_names.iter().any(|n| n.to_string() == attr_name) {
                    let attr = AXAttribute::new(&CFString::new(attr_name));
                    if let Ok(value) = self.element.0.attribute(&attr) {
                        let attr_name_str = attr_name.to_string();
                        if let Some(cf_bool) = value.downcast_into::<CFBoolean>() {
                            let bool_val = cf_bool == CFBoolean::true_value();
                            attrs
                                .properties
                                .insert(attr_name_str.clone(), Some(Value::Bool(bool_val)));
                            debug!("Added window attribute {}: {:?}", attr_name_str, bool_val);
                        }
                    }
                }
            }

            // Populate AI layout fields
            if let Ok(children) = self.children() {
                attrs.child_count = Some(children.len());
            }
            if let Ok(Some(parent)) = self.parent() {
                if let Ok(siblings) = parent.children() {
                    let self_element = UIElement::new(self.clone_box());
                    if let Some(idx) = siblings.iter().position(|e| e == &self_element) {
                        attrs.index_in_parent = Some(idx);
                    }
                }
            }
            if let Ok(selected) = self.is_selected() {
                attrs.is_selected = Some(selected);
            }

            let duration = start.elapsed();
            debug!("Window attributes completed in {:?}", duration);
            if duration > std::time::Duration::from_millis(100) {
                warn!(
                    "⚠️ attributes() unusually long: window attributes collection took {:?} (>100ms) for element: {:?} (role: AXWindow)",
                    duration, self.element.0
                );
            }
            return attrs;
        }

        debug!("Starting non-window element attribute collection");

        // For non-window elements, use standard attribute retrieval
        let mut attrs = UIElementAttributes {
            // Use our role() method which handles the mapping of AXMenuItem to button
            role: self.role(),
            name: None,
            label: None,
            value: None,
            description: None,
            text: self.get_text(0).ok(),
            properties,
            is_keyboard_focusable: Some(false), // macos: not implemented
            is_focused: self.is_focused().ok(),
            enabled: self.is_enabled().ok(),
            bounds: None, // Will be populated by get_configurable_attributes if focusable
            is_toggled: None,
            is_selected: None,
            child_count: None,
            index_in_parent: None,
        };

        // Debug attribute collection
        debug!(
            "Collecting attributes for element with role: {}",
            attrs.role
        );

        // Fetch attr_names once for non-window elements
        let attr_names = match self.element.0.attribute_names() {
            Ok(names) => {
                if names.is_empty() {
                    debug!("attribute_names returned an empty list; skipping attribute retrieval");
                    let duration = start.elapsed();
                    debug!(
                        "Non-window attributes completed in {:?} with {} properties",
                        duration,
                        attrs.properties.len()
                    );
                    if duration > std::time::Duration::from_millis(100) {
                        warn!(
                            "⚠️ attributes() unusually long: non-window attributes collection took {:?} (>100ms) for element: {:?} (role: {:?})",
                            duration, self.element.0, attrs.role
                        );
                    }
                    return attrs;
                }
                names
            }
            Err(e) => {
                debug!("Failed to get attribute names: {:?}", e);
                let duration = start.elapsed();
                debug!(
                    "Non-window attributes completed in {:?} with {} properties",
                    duration,
                    attrs.properties.len()
                );
                if duration > std::time::Duration::from_millis(100) {
                    warn!(
                        "⚠️ attributes() unusually long: non-window attributes collection took {:?} (>100ms) for element: {:?} (role: {:?})",
                        duration, self.element.0, attrs.role
                    );
                }
                return attrs;
            }
        };

        // Only attempt to get AXTitle if present
        if attr_names.iter().any(|n| n.to_string() == "AXTitle") {
            let label_attr = AXAttribute::new(&CFString::new("AXTitle"));
            if let Ok(value) = self.element.0.attribute(&label_attr) {
                if let Some(cf_string) = value.downcast_into::<CFString>() {
                    let title = cf_string.to_string();
                    attrs.name = Some(title.clone());
                    attrs.label = Some(title);
                    debug!("Found AXTitle: {:?}", attrs.name);
                }
            }
        } else if attr_names.iter().any(|n| n.to_string() == "AXLabel") {
            // Fallback to AXLabel if AXTitle is not present
            let alt_label_attr = AXAttribute::new(&CFString::new("AXLabel"));
            if let Ok(value) = self.element.0.attribute(&alt_label_attr) {
                if let Some(cf_string) = value.downcast_into::<CFString>() {
                    let label = cf_string.to_string();
                    attrs.name = Some(label.clone());
                    attrs.label = Some(label);
                    debug!("Found AXLabel: {:?}", attrs.name);
                }
            }
        }

        // Only attempt to get AXDescription if present
        if attr_names.iter().any(|n| n.to_string() == "AXDescription") {
            let desc_attr = AXAttribute::new(&CFString::new("AXDescription"));
            if let Ok(value) = self.element.0.attribute(&desc_attr) {
                if let Some(cf_string) = value.downcast_into::<CFString>() {
                    attrs.description = Some(cf_string.to_string());
                    debug!("Found AXDescription: {:?}", attrs.description);
                }
            }
        }

        // Collect all other attributes, but only if present in attr_names
        debug!("Starting collection of all available attributes");
        debug!("Found {} total attributes to process", attr_names.len());
        // Print the list of attribute names for debugging
        let attr_names_vec: Vec<String> = attr_names.iter().map(|n| n.to_string()).collect();
        debug!("Attribute names for element: {:?}", attr_names_vec);

        for (index, name) in attr_names.iter().enumerate() {
            // if index % 10 == 0 {
            debug!("Processing attribute {} of {}", index + 1, attr_names.len());
            // }
            // Only attempt to fetch attribute if it's in attr_names (guaranteed by loop)
            let attr = AXAttribute::new(&name);
            match self.element.0.attribute(&attr) {
                Ok(value) => {
                    let parsed_value = parse_ax_attribute_value(&name.to_string(), value);
                    attrs.properties.insert(name.to_string(), parsed_value);
                }
                Err(e) => {
                    // Avoid logging for common expected errors to reduce noise
                    if !matches!(
                        e,
                        accessibility::Error::Ax(-25212)
                            | accessibility::Error::Ax(-25205)
                            | accessibility::Error::Ax(-25204)
                    ) {
                        debug!("Error getting attribute {:?}: {:?}", name, e);
                    }
                }
            }
        }
        debug!("Completed processing all {} attributes", attr_names.len());

        // Populate AI layout fields
        if let Ok(children) = self.children() {
            attrs.child_count = Some(children.len());
        }
        if let Ok(Some(parent)) = self.parent() {
            if let Ok(siblings) = parent.children() {
                let self_element = UIElement::new(self.clone_box());
                if let Some(idx) = siblings.iter().position(|e| e == &self_element) {
                    attrs.index_in_parent = Some(idx);
                }
            }
        }
        if let Ok(selected) = self.is_selected() {
            attrs.is_selected = Some(selected);
        }

        let duration = start.elapsed();
        debug!(
            "Non-window attributes completed in {:?} with {} properties",
            duration,
            attrs.properties.len()
        );
        if duration > std::time::Duration::from_millis(100) {
            warn!(
                "⚠️ attributes() unusually long: non-window attributes collection took {:?} (>100ms) for element: {:?} (role: {:?})",
                duration, self.element.0, attrs.role
            );
        }
        attrs
    }

    fn children(&self) -> Result<Vec<UIElement>, AutomationError> {
        debug!("Getting children for element: {:?}", self.element.0.role());
        let mut all_children = Vec::new();

        // First try to get windows
        if let Ok(windows) = self.element.0.windows() {
            debug!("Found {} windows", windows.len());

            // Add all windows to our collection
            for window in windows.iter() {
                all_children.push(UIElement::new(Box::new(MacOSUIElement {
                    element: ThreadSafeAXUIElement::new(window.clone()),
                    use_background_apps: self.use_background_apps,
                    activate_app: self.activate_app,
                })));
            }
        }
        // try main window
        if let Ok(window) = self.element.0.main_window() {
            debug!("Found main window");
            all_children.push(UIElement::new(Box::new(MacOSUIElement {
                element: ThreadSafeAXUIElement::new(window.clone()),
                use_background_apps: self.use_background_apps,
                activate_app: self.activate_app,
            })));
        }

        // Then get regular children
        match self.element.0.children() {
            Ok(children) => {
                // Add regular children to our collection
                for child in children.iter() {
                    all_children.push(UIElement::new(Box::new(MacOSUIElement {
                        element: ThreadSafeAXUIElement::new(child.clone()),
                        use_background_apps: self.use_background_apps,
                        activate_app: self.activate_app,
                    })));
                }

                Ok(all_children)
            }
            Err(e) => {
                // If we have windows but failed to get children, return the windows
                if !all_children.is_empty() {
                    debug!(
                        "Failed to get regular children but returning {} windows",
                        all_children.len()
                    );
                    Ok(all_children)
                } else {
                    // Otherwise return the error
                    Err(AutomationError::PlatformError(format!(
                        "Failed to get children: {}",
                        e
                    )))
                }
            }
        }
    }

    fn parent(&self) -> Result<Option<UIElement>, AutomationError> {
        // Get parent of this element
        let attr = AXAttribute::new(&CFString::new("AXParent"));

        match self.element.0.attribute(&attr) {
            Ok(value) => {
                if let Some(parent) = value.downcast::<AXUIElement>() {
                    Ok(Some(UIElement::new(Box::new(MacOSUIElement {
                        element: ThreadSafeAXUIElement::new(parent),
                        use_background_apps: self.use_background_apps,
                        activate_app: self.activate_app,
                    }))))
                } else {
                    Ok(None) // No parent
                }
            }
            Err(_) => Ok(None),
        }
    }

    fn bounds(&self) -> Result<(f64, f64, f64, f64), AutomationError> {
        let mut x = 0.0;
        let mut y = 0.0;
        let mut width = 0.0;
        let mut height = 0.0;

        // Get position
        if let Ok(position) = self
            .element
            .0
            .attribute(&AXAttribute::new(&CFString::new("AXPosition")))
        {
            unsafe {
                let value_ref = position.as_CFTypeRef();

                // Use AXValueGetValue to extract CGPoint data directly
                let mut point: CGPoint = CGPoint { x: 0.0, y: 0.0 };
                let point_ptr = &mut point as *mut CGPoint as *mut ::std::os::raw::c_void;

                if AXValueGetValue(value_ref as *const _, K_AXVALUE_CGPOINT_TYPE, point_ptr) != 0 {
                    x = point.x;
                    y = point.y;
                }
            }
        }

        // Get size
        if let Ok(size) = self
            .element
            .0
            .attribute(&AXAttribute::new(&CFString::new("AXSize")))
        {
            unsafe {
                let value_ref = size.as_CFTypeRef();

                // Use AXValueGetValue to extract CGSize data directly
                let mut cg_size: CGSize = CGSize {
                    width: 0.0,
                    height: 0.0,
                };
                let size_ptr = &mut cg_size as *mut CGSize as *mut ::std::os::raw::c_void;

                if AXValueGetValue(value_ref as *const _, K_AXVALUE_CGSIZE_TYPE, size_ptr) != 0 {
                    width = cg_size.width;
                    height = cg_size.height;
                }
            }
        }

        debug!(
            "Element bounds: x={}, y={}, width={}, height={}",
            x, y, width, height
        );

        Ok((x, y, width, height))
    }

    fn click(&self) -> Result<ClickResult, AutomationError> {
        // Use the default Auto selection
        self.click_auto()
    }

    fn double_click(&self) -> Result<ClickResult, AutomationError> {
        // First click
        let first_click = self.click()?;

        // Second click - if this fails, return error from second click
        match self.click() {
            Ok(second_click) => {
                // Return information about both clicks
                Ok(ClickResult {
                    method: second_click.method,
                    coordinates: second_click.coordinates,
                    details: format!(
                        "Double-click: First click: {}, Second click: {}",
                        first_click.details, second_click.details
                    ),
                })
            }
            Err(e) => Err(e),
        }
    }

    fn right_click(&self) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "Right-click not yet implemented for macOS".to_string(),
        ))
    }

    fn hover(&self) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "Hover not yet implemented for macOS".to_string(),
        ))
    }

    fn focus(&self) -> Result<(), AutomationError> {
        // Implement proper focus functionality using AXUIElementPerformAction with the "AXRaise" action
        // or by setting it as the AXFocusedUIElement of its parent window

        // First try using the AXRaise action
        let raise_attr = AXAttribute::new(&CFString::new("AXRaise"));
        if let Ok(_) = self.element.0.perform_action(&raise_attr.as_CFString()) {
            debug!("Successfully raised element");

            // Now try to directly focus the element
            // Get the application element
            if let Some(app) = self.get_application() {
                // Set the focused element
                unsafe {
                    let app_ref =
                        app.element.0.as_concrete_TypeRef() as *mut ::std::os::raw::c_void;
                    let attr_str = CFString::new("AXFocusedUIElement");
                    let attr_str_ref =
                        attr_str.as_concrete_TypeRef() as *const ::std::os::raw::c_void;
                    let elem_ref =
                        self.element.0.as_concrete_TypeRef() as *const ::std::os::raw::c_void;

                    let result = AXUIElementSetAttributeValue(app_ref, attr_str_ref, elem_ref);
                    if result == 0 {
                        debug!("Successfully set focus to element");
                        return Ok(());
                    } else {
                        debug!("Failed to set element as focused: error code {}", result);
                    }
                }
            }
        }

        // If we can't use AXRaise or set focus directly, try to click the element
        // which often gives it focus as a side effect
        debug!("Attempting to focus by clicking the element");

        // Handle the ClickResult by mapping to unit result
        self.click().map(|_result| {
            // Optionally log the details of how the click was performed
            debug!("Focus achieved via click method: {}", _result.method);
            ()
        })
    }

    fn type_text(&self, text: &str, _use_clipboard: bool) -> Result<(), AutomationError> {
        // First, try to focus the element, but continue even if focus fails for web inputs
        match self.focus() {
            Ok(_) => debug!("Successfully focused element for typing"),
            Err(e) => {
                debug!("Focus failed, but continuing with type_text: {:?}", e);
                // Click the element, which is often needed for web inputs
                if let Err(click_err) = self.click() {
                    debug!("Click also failed: {:?}", click_err);
                }
            }
        }

        // Check if this is a web input by examining the role
        let is_web_input = {
            let role = self.role().to_lowercase();
            role.contains("web") || role.contains("generic")
        };

        // For web inputs, we might need a different approach
        if is_web_input {
            debug!("Detected web input, using specialized handling");

            // Try different attribute names that web inputs might use
            for attr_name in &["AXValue", "AXValueAttribute", "AXText"] {
                let cf_string = CFString::new(text);
                unsafe {
                    let element_ref =
                        self.element.0.as_concrete_TypeRef() as *mut ::std::os::raw::c_void;
                    let attr_str = CFString::new(attr_name);
                    let attr_str_ref =
                        attr_str.as_concrete_TypeRef() as *const ::std::os::raw::c_void;
                    let value_ref =
                        cf_string.as_concrete_TypeRef() as *const ::std::os::raw::c_void;

                    let result = AXUIElementSetAttributeValue(element_ref, attr_str_ref, value_ref);
                    if result == 0 {
                        debug!("Successfully set text using {}", attr_name);
                        return Ok(());
                    }
                }
            }
        }

        // Standard approach for native controls
        // Create a CFString from the input text
        let cf_string = CFString::new(text);

        // Set the value of the element using direct AXUIElementSetAttributeValue call
        unsafe {
            let element_ref = self.element.0.as_concrete_TypeRef() as *mut ::std::os::raw::c_void;
            let attr_str = CFString::new("AXValue");
            let attr_str_ref = attr_str.as_concrete_TypeRef() as *const ::std::os::raw::c_void;
            let value_ref = cf_string.as_concrete_TypeRef() as *const ::std::os::raw::c_void;

            let result = AXUIElementSetAttributeValue(element_ref, attr_str_ref, value_ref);

            if result != 0 {
                debug!(
                    "Failed to set text value via AXValue: error code {}",
                    result
                );

                return Err(AutomationError::PlatformError(format!(
                    "Failed to set text: error code {}",
                    result
                )));
            }
        }

        Ok(())
    }

    fn press_key(&self, key_combo: &str) -> Result<(), AutomationError> {
        debug!("Pressing key combination: {}", key_combo);

        // Get element role and details for better error reporting
        let element_role = self.role();
        let element_label = self.attributes().label.unwrap_or_default();

        // First, try to focus the element - FAIL if focus fails
        match self.focus() {
            Ok(_) => debug!("successfully focused element for key press"),
            Err(e) => {
                let error_msg = format!(
                    "key press aborted - failed to focus {} element '{}' before pressing '{}': {}",
                    element_role, element_label, key_combo, e
                );
                debug!("{}", error_msg);
                return Err(AutomationError::PlatformError(error_msg));
            }
        }

        // Parse the key combination
        let (key_code, flags) = self.parse_key_combination(key_combo)?;

        // Create event source
        let source =
            CGEventSource::new(core_graphics::event_source::CGEventSourceStateID::HIDSystemState)
                .map_err(|_| {
                AutomationError::PlatformError("Failed to create event source".to_string())
            })?;

        // Key down event with modifiers
        let key_down = CGEvent::new_keyboard_event(source.clone(), key_code as CGKeyCode, true)
            .map_err(|_| {
                AutomationError::PlatformError("Failed to create key down event".to_string())
            })?;

        // Set modifiers if any
        if !flags.is_empty() {
            key_down.set_flags(flags);
        }

        key_down.post(core_graphics::event::CGEventTapLocation::HID);

        // Brief pause
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Key up event with same modifiers
        let key_up =
            CGEvent::new_keyboard_event(source, key_code as CGKeyCode, false).map_err(|_| {
                AutomationError::PlatformError("Failed to create key up event".to_string())
            })?;

        // Set the same modifiers for key up
        if !flags.is_empty() {
            key_up.set_flags(flags);
        }

        key_up.post(core_graphics::event::CGEventTapLocation::HID);

        debug!("Successfully pressed key combination: {}", key_combo);
        Ok(())
    }

    fn get_text(&self, max_depth: usize) -> Result<String, AutomationError> {
        debug!("collecting all text with max_depth={}", max_depth);

        // Create a collector that matches ALL elements (predicate always returns true)
        // This will collect every accessible element in the tree
        let collector = ElementsCollectorWithWindows::new(&self.element.0, |_| true)
            .with_limits(None, Some(max_depth)); // Apply the max_depth

        // Get all elements
        let elements = collector.find_all();
        debug!("collected {} elements for text extraction", elements.len());

        // Extract text from all collected elements
        let mut all_text: Vec<String> = Vec::new();
        for element in elements {
            // Extract text attributes from each element
            for attr_name in &[
                "AXValue",
                "AXTitle",
                "AXDescription",
                "AXHelp",
                "AXLabel",
                "AXText",
            ] {
                let attr = AXAttribute::new(&CFString::new(attr_name));
                if let Ok(value) = element.attribute(&attr) {
                    if let Some(cf_string) = value.downcast_into::<CFString>() {
                        let text = cf_string.to_string();
                        if !text.is_empty() && !all_text.contains(&text) {
                            all_text.push(text);
                        }
                    }
                }
            }
        }

        Ok(all_text.join("\n"))
    }

    fn set_value(&self, value: &str) -> Result<(), AutomationError> {
        // This is essentially the same implementation as type_text for macOS,
        // as both rely on setting the AXValue attribute

        // Create a CFString from the input value
        let cf_string = CFString::new(value);

        // Set the value of the element using direct AXUIElementSetAttributeValue call
        unsafe {
            let element_ref = self.element.0.as_concrete_TypeRef() as *mut ::std::os::raw::c_void;
            let attr_str = CFString::new("AXValue");
            let attr_str_ref = attr_str.as_concrete_TypeRef() as *const ::std::os::raw::c_void;
            let value_ref = cf_string.as_concrete_TypeRef() as *const ::std::os::raw::c_void;

            let result = AXUIElementSetAttributeValue(element_ref, attr_str_ref, value_ref);

            if result != 0 {
                debug!("Failed to set value via AXValue: error code {}", result);

                return Err(AutomationError::PlatformError(format!(
                    "Failed to set value: error code {}",
                    result
                )));
            }
        }

        Ok(())
    }

    fn is_enabled(&self) -> Result<bool, AutomationError> {
        // not implemented
        Err(AutomationError::UnsupportedOperation(
            "is_enabled not yet implemented for macOS".to_string(),
        ))
    }

    fn is_visible(&self) -> Result<bool, AutomationError> {
        // There's no direct "visible" attribute, but we can approximate with bounds
        match self.bounds() {
            Ok((_, _, width, height)) => {
                // If element has non-zero size, it's probably visible
                Ok(width > 0.0 && height > 0.0)
            }
            Err(_) => {
                // If we can't get bounds, assume it's not visible
                Ok(false)
            }
        }
    }

    fn is_focused(&self) -> Result<bool, AutomationError> {
        // not implemented
        Err(AutomationError::UnsupportedOperation(
            "is_focused not yet implemented for macOS".to_string(),
        ))
    }

    fn perform_action(&self, action: &str) -> Result<(), AutomationError> {
        // Perform a named action
        let action_attr = AXAttribute::new(&CFString::new(action));

        self.element
            .0
            .perform_action(&action_attr.as_CFString())
            .map_err(|e| {
                AutomationError::PlatformError(format!(
                    "Failed to perform action {}: {}",
                    action, e
                ))
            })
    }

    fn create_locator(&self, selector: Selector) -> Result<Locator, AutomationError> {
        // Get the platform-specific instance of the engine
        let engine = MacOSEngine::new(self.use_background_apps, self.activate_app)?;

        // If this is an application element, refresh the tree
        if self
            .element
            .0
            .role()
            .map_or(false, |r| r.to_string() == "AXApplication")
        {
            if let Some(app_name) = self.attributes().label {
                engine.refresh_accessibility_tree(Some(&app_name))?;
            }
        }

        // Add some debug output to understand the current element
        let attrs = self.attributes();
        debug!(
            "Creating locator for element: role={}, label={:?}",
            attrs.role, attrs.label
        );

        // Create a new locator with this element as root
        let self_element = UIElement::new(Box::new(MacOSUIElement {
            element: self.element.clone(),
            use_background_apps: self.use_background_apps,
            activate_app: self.activate_app,
        }));

        // Create a locator for the selector with the engine, then set root to this element
        let locator = Locator::new(std::sync::Arc::new(engine), selector).within(self_element);

        Ok(locator)
    }

    fn clone_box(&self) -> Box<dyn UIElementImpl> {
        Box::new(MacOSUIElement {
            element: self.element.clone(),
            use_background_apps: self.use_background_apps,
            activate_app: self.activate_app,
        })
    }

    fn scroll(&self, direction: &str, amount: f64) -> Result<(), AutomationError> {
        // First try to focus the element to ensure it can receive scroll events
        let _ = self.focus();

        // Get element bounds to determine where to scroll
        let (x, y, width, height) = self.bounds()?;
        let center_x = x + width / 2.0;
        let center_y = y + height / 2.0;

        // Create event source
        let source =
            CGEventSource::new(core_graphics::event_source::CGEventSourceStateID::HIDSystemState)
                .map_err(|_| {
                AutomationError::PlatformError("Failed to create event source".to_string())
            })?;

        // Convert amount to scroll units (typically lines)
        let scroll_amount = amount as i32;

        // Create scroll event based on direction
        let (scroll_x, scroll_y) = match direction.to_lowercase().as_str() {
            "up" => (0, -scroll_amount),
            "down" => (0, scroll_amount),
            "left" => (-scroll_amount, 0),
            "right" => (scroll_amount, 0),
            _ => {
                return Err(AutomationError::InvalidArgument(format!(
                    "Invalid scroll direction: {}. Must be up, down, left, or right",
                    direction
                )));
            }
        };

        // Create scroll wheel event
        let scroll_event = CGEvent::new_scroll_event(
            source, 0, 1, // number of wheels (1 for standard mouse wheel)
            scroll_y, scroll_x, 0, // z scroll amount (unused)
        )
        .map_err(|_| AutomationError::PlatformError("Failed to create scroll event".to_string()))?;

        // Post the event at the center point location
        scroll_event.post(core_graphics::event::CGEventTapLocation::HID);

        debug!(
            "Scrolled {} by {} lines at position ({}, {})",
            direction, amount, center_x, center_y
        );

        Ok(())
    }

    fn activate_window(&self) -> Result<(), AutomationError> {
        // On macOS, focusing an element within the window
        // using AXRaise or setting focus often brings the window forward.
        debug!(
            "Activating window by focusing element: {:?}",
            self.element.0
        );
        self.focus()
    }

    fn minimize_window(&self) -> Result<(), AutomationError> {
        // On macOS, try to minimize the window containing this element
        debug!("Minimizing window for element: {:?}", self.element.0);

        // First, try to find the window element
        match self.window() {
            Ok(Some(window)) => {
                // Try to use the AXMinimizeButton action if available
                let minimize_button_attr = AXAttribute::new(&CFString::new("AXMinimizeButton"));
                if let Some(macos_window) = window.as_any().downcast_ref::<MacOSUIElement>() {
                    if let Ok(minimize_button_value) =
                        macos_window.element.0.attribute(&minimize_button_attr)
                    {
                        if let Some(minimize_button) =
                            minimize_button_value.downcast_into::<AXUIElement>()
                        {
                            let press_action = CFString::new("AXPress");
                            if minimize_button.perform_action(&press_action).is_ok() {
                                debug!("Window minimized using AXMinimizeButton");
                                return Ok(());
                            }
                        }
                    }
                }

                // Fallback: try using Cmd+M to minimize
                window.press_key("cmd+m").map_err(|e| {
                    AutomationError::PlatformError(format!("Failed to minimize window: {}", e))
                })
            }
            Ok(None) => {
                // No window found, try Cmd+M on the current element
                self.press_key("cmd+m").map_err(|e| {
                    AutomationError::PlatformError(format!(
                        "Failed to minimize (no window found): {}",
                        e
                    ))
                })
            }
            Err(e) => Err(AutomationError::PlatformError(format!(
                "Failed to get window for minimize: {}",
                e
            ))),
        }
    }

    fn maximize_window(&self) -> Result<(), AutomationError> {
        // On macOS, try to maximize the window containing this element
        debug!("Maximizing window for element: {:?}", self.element.0);

        // Try to find the window element
        match self.window() {
            Ok(Some(window)) => {
                // Try to use the AXZoomButton action if available
                let zoom_button_attr = AXAttribute::new(&CFString::new("AXZoomButton"));
                if let Some(macos_window) = window.as_any().downcast_ref::<MacOSUIElement>() {
                    if let Ok(zoom_button_value) =
                        macos_window.element.0.attribute(&zoom_button_attr)
                    {
                        if let Some(zoom_button) = zoom_button_value.downcast_into::<AXUIElement>()
                        {
                            let press_action = CFString::new("AXPress");
                            if zoom_button.perform_action(&press_action).is_ok() {
                                debug!("Window maximized using AXZoomButton");
                                return Ok(());
                            }
                        }
                    }
                }

                // AXZoomButton not available or failed
                Err(AutomationError::PlatformError(
                    "AXZoomButton not available for window maximize".to_string(),
                ))
            }
            Ok(None) => {
                // No window found
                Err(AutomationError::PlatformError(
                    "No window found for maximize operation".to_string(),
                ))
            }
            Err(e) => Err(AutomationError::PlatformError(format!(
                "Failed to get window for maximize: {}",
                e
            ))),
        }
    }

    fn mouse_drag(
        &self,
        _start_x: f64,
        _start_y: f64,
        _end_x: f64,
        _end_y: f64,
    ) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "mouse_drag is not implemented for macOS yet".to_string(),
        ))
    }

    fn is_keyboard_focusable(&self) -> Result<bool, AutomationError> {
        // Not implemented for macOS yet
        Ok(false)
    }

    fn mouse_click_and_hold(&self, _x: f64, _y: f64) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "mouse_click_and_hold is not implemented for macOS yet".to_string(),
        ))
    }

    fn mouse_move(&self, _x: f64, _y: f64) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "mouse_move is not implemented for macOS yet".to_string(),
        ))
    }

    fn mouse_release(&self) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "mouse_release is not implemented for macOS yet".to_string(),
        ))
    }

    fn application(&self) -> Result<Option<UIElement>, AutomationError> {
        let mut current_ax_element = (*self.element.0).clone();
        loop {
            match current_ax_element.role() {
                Ok(role) => {
                    if role.to_string() == "AXApplication" {
                        return Ok(Some(UIElement::new(Box::new(MacOSUIElement {
                            element: ThreadSafeAXUIElement::new(current_ax_element),
                            use_background_apps: self.use_background_apps,
                            activate_app: self.activate_app,
                        }))));
                    }
                }
                Err(e) => {
                    return Err(AutomationError::PlatformError(format!(
                        "Failed to get role: {}",
                        e
                    )));
                }
            }

            match current_ax_element.parent() {
                Ok(parent_ax_element) => current_ax_element = parent_ax_element,
                Err(_) => return Ok(None), // No more parents or error getting parent
            }
        }
    }

    fn window(&self) -> Result<Option<UIElement>, AutomationError> {
        let mut current_ax_element = (*self.element.0).clone();
        loop {
            match current_ax_element.role() {
                Ok(role_cfstring) => {
                    let role = role_cfstring.to_string();
                    // AXWindow is the main window, AXSheet or AXDrawer can be dialogs/sidebars (acting as windows)
                    // AXWebArea can sometimes be the main content area of a browser tab.
                    if role == "AXWindow"
                        || role == "AXSheet"
                        || role == "AXDrawer"
                        || role == "AXWebArea"
                    {
                        return Ok(Some(UIElement::new(Box::new(MacOSUIElement {
                            element: ThreadSafeAXUIElement::new(current_ax_element),
                            use_background_apps: self.use_background_apps,
                            activate_app: self.activate_app,
                        }))));
                    }
                }
                Err(e) => {
                    return Err(AutomationError::PlatformError(format!(
                        "Failed to get role: {}",
                        e
                    )));
                }
            }

            match current_ax_element.parent() {
                Ok(parent_ax_element) => current_ax_element = parent_ax_element,
                Err(_) => return Ok(None), // No more parents or error getting parent
            }
        }
    }

    fn highlight(
        &self,
        _color: Option<u32>,
        _duration: Option<std::time::Duration>,
    ) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "highlight is not implemented for macOS yet".to_string(),
        ))
    }

    fn capture(&self) -> Result<ScreenshotResult, AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "capture is not implemented for macOS yet".to_string(),
        ))
    }

    fn set_transparency(&self, _percentage: u8) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "set_transparency is not implemented for macOS yet".to_string(),
        ))
    }

    fn process_id(&self) -> Result<u32, AutomationError> {
        let pid = get_pid_for_element(&self.element);
        if pid != -1 {
            Ok(pid as u32)
        } else {
            Err(AutomationError::PlatformError(
                "Failed to get process ID for element".to_string(),
            ))
        }
    }

    fn close(&self) -> Result<(), AutomationError> {
        // Check the element role to determine if it's closable
        let role = self.role();

        match role.as_str() {
            "AXWindow" | "AXDialog" | "AXSheet" => {
                // For windows and dialogs, try to find and click the close button

                // First try to use the AXCloseButton action if available
                let close_button_attr = AXAttribute::new(&CFString::new("AXCloseButton"));
                if let Ok(close_button_value) = self.element.0.attribute(&close_button_attr) {
                    if let Some(close_button) = close_button_value.downcast_into::<AXUIElement>() {
                        let press_action = CFString::new("AXPress");
                        if close_button.perform_action(&press_action).is_ok() {
                            return Ok(());
                        }
                    }
                }

                // Fallback: try to send Cmd+W to close the window
                match self.press_key("cmd+w") {
                    Ok(_) => Ok(()),
                    Err(_) => {
                        // Final fallback: try Cmd+Q to quit the application (for windows that don't support Cmd+W)
                        self.press_key("cmd+q").map_err(|e| {
                            AutomationError::PlatformError(format!("Failed to close window: {}", e))
                        })
                    }
                }
            }
            "AXButton" => {
                // For buttons, check if it's a close button by name/identifier
                if let Some(name) = self.name() {
                    if name.to_lowercase().contains("close")
                        || name.contains("✕")
                        || name.contains("×")
                    {
                        return self.click().map(|_| ());
                    }
                }

                // Check identifier for close button indicators
                if let Ok(identifier_attr) = self
                    .element
                    .0
                    .attribute(&AXAttribute::new(&CFString::new("AXIdentifier")))
                {
                    if let Some(identifier) = identifier_attr.downcast_into::<CFString>() {
                        let id_str = identifier.to_string().to_lowercase();
                        if id_str.contains("close") || id_str.contains("dismiss") {
                            return self.click().map(|_| ());
                        }
                    }
                }

                // Regular button - do nothing
                Ok(())
            }
            _ => {
                // For other element types, do nothing (safely)
                Ok(())
            }
        }
    }

    // TODO: macOS backend doesn't yet expose current URL.
    fn url(&self) -> Option<String> {
        None
    }

    fn select_option(&self, _option_name: &str) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "select_option is not implemented for macOS yet".to_string(),
        ))
    }

    fn list_options(&self) -> Result<Vec<String>, AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "list_options is not implemented for macOS yet".to_string(),
        ))
    }

    fn is_toggled(&self) -> Result<bool, AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "is_toggled is not implemented for macOS yet".to_string(),
        ))
    }

    fn set_toggled(&self, _state: bool) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "set_toggled is not implemented for macOS yet".to_string(),
        ))
    }

    fn get_range_value(&self) -> Result<f64, AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "get_range_value is not implemented for macOS yet".to_string(),
        ))
    }

    fn set_range_value(&self, _value: f64) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "set_range_value is not implemented for macOS yet".to_string(),
        ))
    }

    fn is_selected(&self) -> Result<bool, AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "is_selected is not implemented for macOS yet".to_string(),
        ))
    }

    fn set_selected(&self, _state: bool) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "set_selected is not implemented for macOS yet".to_string(),
        ))
    }

    fn invoke(&self) -> Result<(), AutomationError> {
        // Use AXPress as the most common invocation action on macOS
        let press_attr = AXAttribute::new(&CFString::new("AXPress"));
        match self.element.0.perform_action(&press_attr.as_CFString()) {
            Ok(_) => {
                debug!("Successfully invoked element with AXPress");
                Ok(())
            }
            Err(e) => Err(AutomationError::PlatformError(format!(
                "Invoke with AXPress failed: {:?}",
                e
            ))),
        }
    }
}

// Helper function to parse AXUIElement attribute values into appropriate types
fn parse_ax_attribute_value(
    name: &str,
    value: core_foundation::base::CFType,
) -> Option<serde_json::Value> {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_graphics::geometry::{CGPoint, CGSize};
    use serde_json::{json, Value};

    // Handle different types based on known attribute names and value types
    match name {
        // String values (text, identifiers, descriptions)
        "AXRole" | "AXRoleDescription" | "AXIdentifier" | "AXValue" => {
            if let Some(cf_string) = value.downcast_into::<CFString>() {
                return Some(Value::String(cf_string.to_string()));
            }
        }

        // Boolean values
        "AXEnabled" | "AXFocused" => {
            if let Some(cf_bool) = value.downcast_into::<CFBoolean>() {
                return Some(Value::Bool(cf_bool == CFBoolean::true_value()));
            }
        }

        // Numeric values
        "AXNumberOfCharacters" | "AXInsertionPointLineNumber" => {
            if let Some(cf_num) = value.downcast_into::<CFNumber>() {
                if let Some(num) = cf_num.to_i64() {
                    return Some(Value::Number(serde_json::Number::from(num)));
                } else if let Some(num) = cf_num.to_f64() {
                    // Need to handle possible NaN/Infinity which aren't allowed in JSON
                    if num.is_finite() {
                        return serde_json::Number::from_f64(num).map(Value::Number);
                    } else {
                        return Some(Value::Null);
                    }
                }
            }
        }

        // Position, Size and Frame require special handling with AXValue
        "AXPosition" => {
            // Try to extract CGPoint using AXValueGetValue
            unsafe {
                let value_ref = value.as_CFTypeRef();
                let mut point = CGPoint { x: 0.0, y: 0.0 };
                let point_ptr = &mut point as *mut CGPoint as *mut ::std::os::raw::c_void;

                if AXValueGetValue(value_ref, K_AXVALUE_CGPOINT_TYPE, point_ptr) != 0 {
                    return Some(json!({
                        "x": point.x,
                        "y": point.y
                    }));
                }
            }
        }

        "AXSize" => {
            // Try to extract CGSize using AXValueGetValue
            unsafe {
                let value_ref = value.as_CFTypeRef();
                let mut size = CGSize {
                    width: 0.0,
                    height: 0.0,
                };
                let size_ptr = &mut size as *mut CGSize as *mut ::std::os::raw::c_void;

                if AXValueGetValue(value_ref, K_AXVALUE_CGSIZE_TYPE, size_ptr) != 0 {
                    return Some(json!({
                        "width": size.width,
                        "height": size.height
                    }));
                }
            }
        }

        // For attributes that are references to other UI elements
        "AXParent" | "AXWindow" | "AXTopLevelUIElement" => {
            // get object id
            if let Some(ax_element) = value.downcast_into::<AXUIElement>() {
                let address = &ax_element as *const _ as usize;
                return Some(Value::String(format!("{}", address)));
            }
        }

        // For array types (children)
        name if name.starts_with("AXChildren") => {
            debug!("Processing AXChildren attribute");

            unsafe {
                let value_ref = value.as_CFTypeRef();
                let type_id = CFGetTypeID(value_ref);

                if type_id == CFArrayGetTypeID() {
                    // Cast to CFArrayRef
                    let array_ref = value_ref as *const __CFArray;
                    let count = CFArrayGetCount(array_ref);
                    debug!("AXChildren array with {} elements", count);

                    // Create an array of element addresses
                    let mut items = Vec::with_capacity(count as usize);
                    for i in 0..count {
                        let item = CFArrayGetValueAtIndex(array_ref, i as isize);
                        if !item.is_null() {
                            // Correctly wrap the raw pointer into AXUIElement
                            let ax_element = AXUIElement::wrap_under_get_rule(item as *mut _);
                            let address = &ax_element as *const _ as usize;
                            items.push(json!(format!("{}", address)));
                        }
                    }
                    return Some(Value::Array(items));
                }
            }

            return None;
        }

        _ => {}
    }

    // Fallback for unhandled types
    None
}

// Add this helper function after the selector handler
fn element_contains_text(e: &AXUIElement, text: &str) -> bool {
    // Check immediate element attributes for text
    let contains_in_value = e
        .value()
        .ok()
        .and_then(|v| v.downcast_into::<CFString>())
        .map_or(false, |s| s.to_string().contains(text));

    if contains_in_value {
        return true;
    }

    // Check title, description and other text attributes
    let contains_in_title = e
        .title()
        .ok()
        .map_or(false, |t| t.to_string().contains(text));

    let contains_in_desc = e
        .description()
        .ok()
        .map_or(false, |d| d.to_string().contains(text));

    // Check common text attributes
    for attr_name in &[
        "AXValue",
        "AXTitle",
        "AXDescription",
        "AXHelp",
        "AXLabel",
        "AXText",
    ] {
        let attr = AXAttribute::new(&CFString::new(attr_name));
        if let Ok(value) = e.attribute(&attr) {
            if let Some(cf_string) = value.downcast_into::<CFString>() {
                if cf_string.to_string().contains(text) {
                    return true;
                }
            }
        }
    }

    contains_in_title || contains_in_desc
}

// Helper function to get PID from an AXUIElement
fn get_pid_for_element(element: &ThreadSafeAXUIElement) -> i32 {
    // Use accessibility API to get the PID
    unsafe {
        let element_ref = element.0.as_concrete_TypeRef() as *mut ::std::os::raw::c_void;

        // Link with ApplicationServices framework
        #[link(name = "ApplicationServices", kind = "framework")]
        unsafe extern "C" {
            fn AXUIElementGetPid(element: *mut ::std::os::raw::c_void, pid: *mut i32) -> i32;
        }

        let mut pid: i32 = 0;
        let result = AXUIElementGetPid(element_ref, &mut pid);

        if result == 0 {
            return pid;
        }

        // Fallback to -1 if we couldn't get the PID
        -1
    }
}

// Modified to return Vec<String> for multiple possible role matches
fn map_generic_role_to_macos_roles(role: &str) -> Vec<String> {
    match role.to_lowercase().as_str() {
        "application" => vec!["AXApplication".to_string()],
        "window" => vec!["AXWindow".to_string()],
        "button" => vec![
            "AXButton".to_string(),
            "AXMenuItem".to_string(),
            "AXMenuBarItem".to_string(),
            "AXStaticText".to_string(), // Some text might be clickable buttons
            "AXImage".to_string(),      // Some images might be clickable buttons
        ], // Button can be any of these
        "checkbox" => vec!["AXCheckBox".to_string()],
        "menu" => vec!["AXMenu".to_string()],
        "menuitem" => vec!["AXMenuItem".to_string(), "AXMenuBarItem".to_string()], // Include both types
        "dialog" => vec!["AXSheet".to_string(), "AXDialog".to_string()], // macOS often uses Sheet or Dialog
        "text" | "textfield" | "input" | "textbox" => vec![
            "AXTextField".to_string(),
            "AXTextArea".to_string(),
            "AXText".to_string(),
            "AXComboBox".to_string(),
            "AXTextEdit".to_string(),
            "AXSearchField".to_string(),
            "AXWebArea".to_string(), // Web content might contain inputs
            "AXGroup".to_string(),   // Twitter uses groups that contain editable content
            "AXGenericElement".to_string(), // Generic elements that might be inputs
            "AXURIField".to_string(), // Explicit URL field type
            "AXAddressField".to_string(), // Another common name for URL fields
            "AXStaticText".to_string(), // Static text fields
        ],
        // Add specific support for URL fields
        "url" | "urlfield" => vec![
            "AXTextField".to_string(),    // URL fields are often text fields
            "AXURIField".to_string(),     // Explicit URL field type
            "AXAddressField".to_string(), // Another common name for URL fields
        ],
        "list" => vec!["AXList".to_string()],
        "listitem" => vec!["AXCell".to_string()], // List items are often cells in macOS
        "combobox" => vec!["AXPopUpButton".to_string(), "AXComboBox".to_string()],
        "tab" => vec!["AXTabGroup".to_string()],
        "tabitem" => vec!["AXRadioButton".to_string()], // Tab items are sometimes radio buttons
        "toolbar" => vec!["AXToolbar".to_string()],

        _ => vec![role.to_string()], // Keep as-is for unknown roles
    }
}

fn macos_role_to_generic_role(role: &str) -> Vec<String> {
    match role.to_lowercase().as_str() {
        "AXWindow" => vec!["window".to_string()],
        "AXButton" | "AXMenuItem" | "AXMenuBarItem" | "AXPopUpButton" => vec!["button".to_string()],
        "AXTextField" | "AXTextArea" | "AXTextEdit" | "AXSearchField" | "AXURIField"
        | "AXAddressField" => vec![
            "textfield".to_string(),
            "input".to_string(),
            "textbox".to_string(),
            "url".to_string(),
            "urlfield".to_string(),
        ],
        "AXList" => vec!["list".to_string()],
        "AXCell" => vec!["listitem".to_string()],
        "AXSheet" | "AXDialog" => vec!["dialog".to_string()],
        "AXGroup" | "AXGenericElement" | "AXWebArea" => {
            vec!["group".to_string(), "genericElement".to_string()]
        }
        _ => vec![role.to_string()],
    }
}

// List of application localized names to exclude.
const EXCLUDED_APPLICATION_NAMES: &[&str] = &[
    // These are known to consistently fail to retrieve role
    // and take an unusually long time to do so.
    "<unknown>",
    "Raycast Web Content",
    "superwhisper Web Content",
    // "QLPreviewGenerationExtension",
    "QLPreviewGenerationExtension (Open and Save Panel Service (Google Chrome))",
    // "com.apple.Safari.SandboxBroker",
    "com.apple.Safari.SandboxBroker (Safari)",
    // Additional exclusions based on Hammerspoon's window_filter.lua:
    // See: https://github.com/Hammerspoon/hammerspoon/blob/master/extensions/window/window_filter.lua#L115-L138
    // Apps that often have no PID or are not accessible
    "universalaccessd",
    "sharingd",
    "Safari Networking",
    "Spotlight Networking",
    "iTunes Helper",
    "Safari Web Content",
    "App Store Web Content",
    "Safari Database Storage",
    "Google Chrome Helper",
    "Spotify Helper",
    "Todoist Networking",
    "Safari Storage",
    "Todoist Database Storage",
    "AAM Updates Notifier",
    "Slack Helper",
    // Apps that often have no windows or are system/background agents
    "com.apple.internetaccounts",
    "CoreServicesUIAgent",
    "AirPlayUIAgent",
    "com.apple.security.pboxd",
    "PowerChime",
    "SystemUIServer",
    "Dock",
    "com.apple.dock.extra",
    "storeuid",
    "Folder Actions Dispatcher",
    "Keychain Circle Notification",
    "Wi-Fi",
    "Image Capture Extension",
    "iCloud Photos",
    "System Events",
    "Speech Synthesis Server",
    "Dropbox Finder Integration",
    "LaterAgent",
    "Karabiner_AXNotifier",
    "Photos Agent",
    "EscrowSecurityAlert",
    "com.apple.MailServiceAgent",
    "Mail Web Content",
    "nbagent",
    "rcd",
    "Evernote Helper",
    "BTTRelaunch",
];

#[async_trait::async_trait]
impl AccessibilityEngine for MacOSEngine {
    fn get_root_element(&self) -> UIElement {
        self.wrap_element(ThreadSafeAXUIElement::system_wide())
    }

    fn get_element_by_id(&self, _id: i32) -> Result<UIElement, AutomationError> {
        // TODO: Implement PID-based element finding for macOS
        // This is non-trivial as AXUIElement does not directly expose PIDs in the same way.
        // We might need to iterate through applications and their windows.
        Err(AutomationError::UnsupportedOperation(
            "get_element_by_id not fully implemented for macOS".to_string(),
        ))
    }

    fn get_focused_element(&self) -> Result<UIElement, AutomationError> {
        let system_wide = AXUIElement::system_wide();
        let focused_app_attr = AXAttribute::new(&CFString::new("AXFocusedApplication"));
        let focused_element_attr = AXAttribute::new(&CFString::new("AXFocusedUIElement"));

        // Get the focused application first
        let focused_app = system_wide.attribute(&focused_app_attr).map_err(|e| {
            AutomationError::ElementNotFound(format!("Failed to get focused application: {}", e))
        })?;

        if let Some(app_element) = focused_app.downcast::<AXUIElement>() {
            // Then get the focused element within that application
            let focused_element = app_element.attribute(&focused_element_attr).map_err(|e| {
                AutomationError::ElementNotFound(format!(
                    "Failed to get focused element within application: {}",
                    e
                ))
            })?;

            if let Some(element) = focused_element.downcast::<AXUIElement>() {
                Ok(self.wrap_element(ThreadSafeAXUIElement::new(element)))
            } else {
                Err(AutomationError::ElementNotFound(
                    "Focused element attribute did not contain a valid AXUIElement".to_string(),
                ))
            }
        } else {
            Err(AutomationError::ElementNotFound(
                "Focused application attribute did not contain a valid AXUIElement".to_string(),
            ))
        }
    }

    fn get_applications(&self) -> Result<Vec<UIElement>, AutomationError> {
        let mut apps = Vec::new();
        unsafe {
            use objc::{class, msg_send, sel, sel_impl};
            use std::time::Instant;

            let workspace_class = class!(NSWorkspace);
            let shared_workspace: *mut objc::runtime::Object =
                msg_send![workspace_class, sharedWorkspace];
            let running_apps: *mut objc::runtime::Object =
                msg_send![shared_workspace, runningApplications];
            let count: usize = msg_send![running_apps, count];

            for i in 0..count {
                debug!("================================");
                debug!("[macOS][get_applications] i: {}", i);
                let iter_start = Instant::now();

                let app: *mut objc::runtime::Object = msg_send![running_apps, objectAtIndex:i];
                let pid: i32 = msg_send![app, processIdentifier];
                let elapsed_so_far = iter_start.elapsed();
                debug!(
                    "[macOS][get_applications] i: {}, pid: {}, elapsed so far: {} ms",
                    i,
                    pid,
                    elapsed_so_far.as_millis()
                );

                let ax_element = ThreadSafeAXUIElement::application(pid);
                // Log after creating AXUIElement
                let elapsed_after_ax = iter_start.elapsed();
                debug!(
                    "[macOS][get_applications] i: {}, after AXUIElement::application, elapsed so far: {} ms",
                    i,
                    elapsed_after_ax.as_millis()
                );

                // Get the localized name of the application (for debugging/logging)
                let localized_name: Option<String> = {
                    let nsstring: *mut objc::runtime::Object = msg_send![app, localizedName];
                    if !nsstring.is_null() {
                        let cstr: *const std::os::raw::c_char = msg_send![nsstring, UTF8String];
                        if !cstr.is_null() {
                            let cstr = std::ffi::CStr::from_ptr(cstr);
                            cstr.to_str().ok().map(|s| s.to_string())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };

                // Skip excluded applications by localized name
                if let Some(ref name) = localized_name {
                    if EXCLUDED_APPLICATION_NAMES
                        .iter()
                        .any(|&excluded| excluded == name)
                    {
                        debug!(
                            "[macOS][get_applications] Skipping excluded application: {:?}",
                            name
                        );
                        continue;
                    } else {
                        debug!(
                            "[macOS][get_applications] Application localized_name: {:?}",
                            name
                        );
                    }
                }

                let elapsed_localized_name = iter_start.elapsed();
                debug!(
                    "[macOS][get_applications] i: {}, pid: {}, localized_name: {:?}, elapsed so far: {} ms",
                    i,
                    pid,
                    localized_name,
                    elapsed_localized_name.as_millis()
                );

                // Filter: Only include if .role() == "AXApplication"
                // This is a reliable accessibility filter based on runtime introspection
                match ax_element.0.role() {
                    Ok(role_cfstring) => {
                        let elapsed_after_role = iter_start.elapsed();
                        debug!(
                            "[macOS][get_applications] i: {}, after role() fetch, role: {}, elapsed so far: {} ms",
                            i,
                            role_cfstring.to_string(),
                            elapsed_after_role.as_millis()
                        );
                        if role_cfstring.to_string() == "AXApplication" {
                            let elapsed_before_push = iter_start.elapsed();
                            debug!(
                                "[macOS][get_applications] i: {}, pushing AXApplication, elapsed so far: {} ms",
                                i,
                                elapsed_before_push.as_millis()
                            );

                            apps.push(self.wrap_element(ax_element));

                            let elapsed_after_push = iter_start.elapsed();
                            debug!(
                                "[macOS][get_applications] i: {}, after push, elapsed so far: {} ms",
                                i,
                                elapsed_after_push.as_millis()
                            );
                        } else {
                            // did not match AXApplication
                            let elapsed_non_match = iter_start.elapsed();
                            debug!(
                                "[macOS][get_applications] i: {}, did not match AXApplication, elapsed so far: {} ms",
                                i,
                                elapsed_non_match.as_millis()
                            );
                        }
                    }
                    Err(e) => {
                        // failed to get role
                        let elapsed_error = iter_start.elapsed();
                        debug!(
                            "⚠️ [macOS][get_applications] i: {}, failed to get role: {:?}, elapsed so far: {} ms",
                            i,
                            e,
                            elapsed_error.as_millis()
                        );

                        // Get and print list of attr_names for debugging
                        // match ax_element.0.attribute_names() {
                        //     Ok(attr_names) => {
                        //         debug!(
                        //             "[macOS][get_applications] i: {}, failed to get role, attribute names: {:?}",
                        //             i,
                        //             attr_names.iter().map(|a| a.to_string()).collect::<Vec<_>>()
                        //         );
                        //     }
                        //     Err(attr_err) => {
                        //         debug!(
                        //             "[macOS][get_applications] i: {}, failed to get role, and failed to get attribute names: {:?}",
                        //             i,
                        //             attr_err
                        //         );
                        //     }
                        // }
                    }
                }

                let iter_duration = iter_start.elapsed();
                debug!(
                    "[macOS][get_applications] Iteration {} duration: {} ms",
                    i,
                    iter_duration.as_millis()
                );
                if iter_duration > std::time::Duration::from_millis(100) {
                    debug!(
                        "⚠️ [macOS][get_applications] Iteration {} unusually long: took {:?} (>100ms) for pid: {}",
                        i, iter_duration, pid
                    );
                    // Get the last application in the apps vector (if any)
                    // if let Some(last_app) = apps.last() {
                    //     debug!("[macOS][get_applications] Getting last app attributes...");
                    //     // Print all attributes of the last application element
                    //     let attrs = last_app.attributes();
                    //     debug!("[macOS][get_applications] Last app attributes:");
                    //     debug!("  role: {:?}", attrs.role);
                    //     debug!("  name: {:?}", attrs.name);
                    //     debug!("  label: {:?}", attrs.label);
                    //     debug!("  value: {:?}", attrs.value);
                    //     debug!("  description: {:?}", attrs.description);
                    //     debug!("  properties: {:?}", attrs.properties);
                    //     debug!("  is_keyboard_focusable: {:?}", attrs.is_keyboard_focusable);

                    //     // Print debug! using .role() too, but handle error carefully
                    //     // Note: last_app.role() returns a String, not a Result
                    //     // debug!(
                    //     //     "[macOS][get_applications] Last app .role(): {:?}",
                    //     //     last_app.element.0.role()
                    //     // );
                    // }
                }
            }
        }
        Ok(apps)
    }

    fn get_application_by_name(&self, name: &str) -> Result<UIElement, AutomationError> {
        // Refresh tree first to ensure we have the latest data
        self.refresh_accessibility_tree(Some(name))?;

        // Use Objective-C runtime to find the running application by name
        unsafe {
            use objc::{class, msg_send, sel, sel_impl};

            let workspace_class = class!(NSWorkspace);
            let shared_workspace: *mut objc::runtime::Object =
                msg_send![workspace_class, sharedWorkspace];
            let apps: *mut objc::runtime::Object = msg_send![shared_workspace, runningApplications];
            let count: usize = msg_send![apps, count];

            // First, collect all (i, pid, localized_name) tuples
            let mut candidates: Vec<(usize, i32, String)> = Vec::with_capacity(count);

            for i in 0..count {
                let app: *mut objc::runtime::Object = msg_send![apps, objectAtIndex:i];
                let app_name_obj: *mut objc::runtime::Object = msg_send![app, localizedName];

                if !app_name_obj.is_null() {
                    let app_name_str: &str = {
                        let nsstring = app_name_obj as *const objc::runtime::Object;
                        let bytes: *const std::os::raw::c_char = msg_send![nsstring, UTF8String];
                        let len: usize = msg_send![nsstring, lengthOfBytesUsingEncoding:4]; // NSUTF8StringEncoding = 4
                        let bytes_slice = std::slice::from_raw_parts(bytes as *const u8, len);
                        std::str::from_utf8_unchecked(bytes_slice)
                    };
                    let pid: i32 = msg_send![app, processIdentifier];
                    // Print i of total count and localized name while checking
                    debug!(
                        "[macOS][get_application_by_name] i: {}/{} localized_name: {:?}",
                        i + 1,
                        count,
                        app_name_str
                    );
                    candidates.push((i, pid, app_name_str.to_string()));
                }
            }

            // Try to match by localized name (case-insensitive)
            if let Some((_, pid, _)) = candidates
                .iter()
                .find(|(_, _, app_name)| app_name.to_lowercase() == name.to_lowercase())
            {
                let ax_element = ThreadSafeAXUIElement::application(*pid);
                let ui_element = self.wrap_element(ax_element);
                return Ok(ui_element);
            }

            // If no match, try matching by AXUIElement's .name attribute (expensive)
            for (i, pid, _app_name) in &candidates {
                let ax_element = ThreadSafeAXUIElement::application(*pid);
                let ui_element = self.wrap_element(ax_element);
                let short_name = ui_element.attributes().name.unwrap_or_default();
                debug!(
                    "[macOS][get_application_by_name] i: {}/{} AXUIElement name: {:?}",
                    i + 1,
                    count,
                    short_name
                );
                if short_name.to_lowercase() == name.to_lowercase() {
                    return Ok(ui_element);
                }
            }
        }

        Err(AutomationError::ElementNotFound(format!(
            "Application '{}' not found",
            name
        )))
    }

    fn get_application_by_pid(
        &self,
        pid: i32,
        _timeout: Option<Duration>,
    ) -> Result<UIElement, AutomationError> {
        // Create an AXUIElement for the application with the given PID
        let app_element = ThreadSafeAXUIElement::application(pid);

        // Check if the element is valid by getting its role
        match app_element.0.role() {
            Ok(role) if role.to_string() == "AXApplication" => Ok(self.wrap_element(app_element)),
            _ => Err(AutomationError::ElementNotFound(format!(
                "Application with PID {} not found",
                pid
            ))),
        }
    }

    fn find_elements(
        &self,
        selector: &Selector,
        root: Option<&UIElement>,
        timeout: Option<Duration>, // Now properly implemented
        _depth: Option<usize>,     // Depth parameter required by trait
    ) -> Result<Vec<UIElement>, AutomationError> {
        // macOS: Require explicit application root if root is None
        let (actual_root, actual_selector) = if root.is_none() {
            // Only support selector chains starting with application role
            match selector {
                Selector::Chain(selectors) if !selectors.is_empty() => match &selectors[0] {
                    Selector::Role {
                        role,
                        name: Some(app_name),
                    } if role.eq_ignore_ascii_case("application") => {
                        debug!(
                            "macOS: inferring application root from selector chain: {}",
                            app_name
                        );
                        let app_el = self.get_application_by_name(app_name)?;
                        (Some(app_el), Selector::Chain(selectors[1..].to_vec()))
                    }
                    _ => {
                        return Err(AutomationError::InvalidArgument(
                                "macOS: Selector chain must start with { role: 'application', name: ... } if no root is provided".to_string()
                            ));
                    }
                },
                _ => {
                    return Err(AutomationError::InvalidArgument(
                        "macOS: No root provided and selector is not a chain starting with application role".to_string()
                    ));
                }
            }
        } else {
            (root.cloned(), selector.clone())
        };

        // Set up timeout handling
        let effective_timeout = timeout.unwrap_or(Duration::from_secs(30));
        let start_time = std::time::Instant::now();

        // Now call the original logic with the resolved root and selector
        let start_element = if let Some(el) = actual_root {
            if let Some(macos_el) = el.as_any().downcast_ref::<MacOSUIElement>() {
                macos_el.element.clone()
            } else {
                // If downcast fails or no root specified, start from system wide
                debug!(
                    "Root element provided is not a MacOSUIElement, starting search from system wide."
                );
                self.system_wide.clone()
            }
        } else {
            self.system_wide.clone()
        };

        // Refresh accessibility tree if the root is an application
        if let Ok(role) = start_element.0.role() {
            if role.to_string() == "AXApplication" {
                if let Some(app_name) = start_element.0.title().ok().map(|s| s.to_string()) {
                    self.refresh_accessibility_tree(Some(&app_name))?;
                } else {
                    self.refresh_accessibility_tree(None)?;
                }
            }
        }

        match &actual_selector {
            Selector::Role { role, name } => {
                let target_roles = map_generic_role_to_macos_roles(role);
                let name_filter = name.as_ref().map(|n| n.to_lowercase());

                // Implement timeout retry loop for this selector type
                loop {
                    let target_roles_clone = target_roles.clone();
                    let name_filter_clone = name_filter.clone();
                    let collector = ElementsCollectorWithWindows::new(&start_element.0, move |e| {
                        match e.role() {
                            Ok(r) => {
                                let current_role = r.to_string();
                                // Check if current role matches any of the target roles
                                if target_roles_clone.contains(&current_role) {
                                    // If name filter exists, check if element name contains it (case-insensitive)
                                    if let Some(filter_name) = &name_filter_clone {
                                        // Check title, label, description, value
                                        let matches_name = e.title().ok().map_or(false, |t| {
                                            t.to_string().to_lowercase().contains(filter_name)
                                        }) || e
                                            .attribute(&AXAttribute::new(&CFString::new("AXLabel")))
                                            .ok()
                                            .and_then(|v| v.downcast_into::<CFString>())
                                            .map_or(false, |s| {
                                                s.to_string().to_lowercase().contains(filter_name)
                                            })
                                            || e.description().ok().map_or(false, |d| {
                                                d.to_string().to_lowercase().contains(filter_name)
                                            })
                                            || e.value()
                                                .ok()
                                                .and_then(|v| v.downcast_into::<CFString>())
                                                .map_or(false, |s| {
                                                    s.to_string()
                                                        .to_lowercase()
                                                        .contains(filter_name)
                                                });
                                        matches_name
                                    } else {
                                        true // No name filter, role match is sufficient
                                    }
                                } else {
                                    false // Role doesn't match
                                }
                            }
                            Err(_) => false, // Error getting role, don't include
                        }
                    });

                    let elements = collector
                        .find_all()
                        .into_iter()
                        .map(|e| self.wrap_element(ThreadSafeAXUIElement::new(e)))
                        .collect::<Vec<UIElement>>();

                    // Check if we found elements or if timeout exceeded
                    if !elements.is_empty() || start_time.elapsed() >= effective_timeout {
                        return Ok(elements);
                    }

                    // Wait a bit before retrying
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
            Selector::Id(id_str) => {
                let target_id_str = id_str.clone();
                let use_bg = self.use_background_apps;
                let activate = self.activate_app;
                let collector = ElementsCollectorWithWindows::new(&start_element.0, move |e| {
                    let macos_elem_wrapper = MacOSUIElement {
                        element: ThreadSafeAXUIElement::new(e.clone()),
                        use_background_apps: use_bg,
                        activate_app: activate,
                    };
                    macos_elem_wrapper
                        .id()
                        .map_or(false, |calc_id| calc_id == target_id_str)
                }); // Ensure only 2 arguments

                // Collect into Vec<UIElement>
                Ok(collector
                    .find_all()
                    .into_iter()
                    .map(|e| self.wrap_element(ThreadSafeAXUIElement::new(e)))
                    .collect::<Vec<UIElement>>())
            }
            Selector::Name(name) => {
                let name_lower = name.to_lowercase();
                let collector = ElementsCollectorWithWindows::new(&start_element.0, move |e| {
                    // Check various attributes that might contain the name
                    e.title().ok().map_or(false, |t| {
                        t.to_string().to_lowercase().contains(&name_lower)
                    }) || e
                        .attribute(&AXAttribute::new(&CFString::new("AXLabel")))
                        .ok()
                        .and_then(|v| v.downcast_into::<CFString>())
                        .map_or(false, |s| {
                            s.to_string().to_lowercase().contains(&name_lower)
                        })
                        || e.description().ok().map_or(false, |d| {
                            d.to_string().to_lowercase().contains(&name_lower)
                        })
                        || e.value()
                            .ok()
                            .and_then(|v| v.downcast_into::<CFString>())
                            .map_or(false, |s| {
                                s.to_string().to_lowercase().contains(&name_lower)
                            })
                }); // Add None for implicit_wait

                Ok(collector
                    .find_all()
                    .into_iter()
                    .map(|e| self.wrap_element(ThreadSafeAXUIElement::new(e)))
                    .collect())
            }
            Selector::Text(text) => {
                let text_lower = text.to_lowercase();
                let collector = ElementsCollectorWithWindows::new(&start_element.0, move |e| {
                    // Use the helper function to check if the element contains the text
                    element_contains_text(e, &text_lower)
                }); // Add None for implicit_wait

                Ok(collector
                    .find_all()
                    .into_iter()
                    .map(|e| self.wrap_element(ThreadSafeAXUIElement::new(e)))
                    .collect())
            }
            Selector::Path(_) => Err(AutomationError::UnsupportedOperation(
                "Path selector not yet supported for macOS".to_string(),
            )),
            Selector::NativeId(_) => Err(AutomationError::UnsupportedOperation(
                "NativeId selector not yet supported for macOS".to_string(),
            )),
            Selector::Attributes(_) => Err(AutomationError::UnsupportedOperation(
                "Attributes selector not yet supported for macOS".to_string(),
            )),
            Selector::Filter(_) => Err(AutomationError::UnsupportedOperation(
                "Filter selector not yet supported for macOS".to_string(),
            )),
            Selector::Visible(_) => Err(AutomationError::UnsupportedOperation(
                "Visible selector not yet supported for macOS".to_string(),
            )),
            Selector::Chain(selectors) => {
                if selectors.is_empty() {
                    return Err(AutomationError::InvalidArgument(
                        "Selector chain cannot be empty".to_string(),
                    ));
                }

                let mut current_roots = vec![self.wrap_element(start_element)]; // Start with the initial root

                // Iterate through selectors, refining the list of matching elements
                for (i, selector) in selectors.iter().enumerate() {
                    let mut next_roots = Vec::new();
                    let is_last_selector = i == selectors.len() - 1;

                    for root_element in &current_roots {
                        // Find elements matching the current selector within the current root
                        let found_elements = self
                            .find_elements(selector, Some(root_element), timeout, None)
                            .map_err(|e| {
                                AutomationError::PlatformError(format!(
                                    "Recursive find_elements failed: {}",
                                    e
                                ))
                            })?;

                        if is_last_selector {
                            // If it's the last selector, collect all found elements
                            next_roots.extend(found_elements);
                        } else {
                            // If not the last selector, and we found exactly one element,
                            // use it as the root for the next iteration.
                            // If zero or multiple found, the chain breaks here for finding *elements*.
                            if found_elements.len() == 1 {
                                next_roots.push(found_elements.into_iter().next().unwrap());
                            } else {
                                // If 0 or >1 elements found before the last selector,
                                // it means the path diverged or ended. No elements match the full chain.
                                // Clear next_roots to signal no further matches.
                                next_roots.clear();
                                break; // Exit the inner loop (over current_roots)
                            }
                        }
                    }

                    current_roots = next_roots;
                    if current_roots.is_empty() && !is_last_selector {
                        // If no elements were found matching an intermediate selector, break early.
                        break;
                    }
                }
                Ok(current_roots) // Return all elements found by the last selector
            }
            Selector::ClassName(_) => Err(AutomationError::UnsupportedOperation(
                "ClassName selector is not yet supported for macOS".to_string(),
            )),
            Selector::LocalizedRole(_) => Err(AutomationError::UnsupportedOperation(
                "LocalizedRole selector is not yet supported for macOS".to_string(),
            )),

            Selector::Invalid(reason) => Err(AutomationError::InvalidArgument(reason.clone())),
            Selector::RightOf(_) | Selector::LeftOf(_) | Selector::Above(_) | Selector::Below(_) | Selector::Near(_) => Err(AutomationError::UnsupportedOperation(
                "Relative selectors (RightOf/LeftOf/Above/Below/Near) are not yet supported for macOS".to_string(),
            )),
            Selector::Nth(_) => Err(AutomationError::UnsupportedOperation(
                "Nth selector not yet supported for macOS".to_string(),
            )),
            Selector::Has(_) => Err(AutomationError::UnsupportedOperation(
                "Has selector not yet supported for macOS".to_string(),
            )),
        }
    }

    #[instrument(level = "debug", skip(self, selector, root, timeout))]
    fn find_element(
        &self,
        selector: &Selector,
        root: Option<&UIElement>,
        timeout: Option<Duration>, // Now properly implemented
    ) -> Result<UIElement, AutomationError> {
        // macOS: Require explicit application root if root is None
        let (actual_root, actual_selector) = if root.is_none() {
            match selector {
                Selector::Chain(selectors) if !selectors.is_empty() => match &selectors[0] {
                    Selector::Role {
                        role,
                        name: Some(app_name),
                    } if role.eq_ignore_ascii_case("application") => {
                        debug!(
                            "macOS: inferring application root from selector chain: {}",
                            app_name
                        );
                        let app_el = self.get_application_by_name(app_name)?;
                        (Some(app_el), Selector::Chain(selectors[1..].to_vec()))
                    }
                    _ => {
                        return Err(AutomationError::InvalidArgument(
                                "macOS: Selector chain must start with { role: 'application', name: ... } if no root is provided".to_string()
                            ));
                    }
                },
                _ => {
                    return Err(AutomationError::InvalidArgument(
                        "macOS: No root provided and selector is not a chain starting with application role".to_string()
                    ));
                }
            }
        } else {
            (root.cloned(), selector.clone())
        };

        let start_element = if let Some(el) = actual_root {
            if let Some(macos_el) = el.as_any().downcast_ref::<MacOSUIElement>() {
                macos_el.element.clone()
            } else {
                self.system_wide.clone()
            }
        } else {
            self.system_wide.clone()
        };

        // Refresh accessibility tree if the root is an application
        if let Ok(role) = start_element.0.role() {
            if role.to_string() == "AXApplication" {
                if let Some(app_name) = start_element.0.title().ok().map(|s| s.to_string()) {
                    self.refresh_accessibility_tree(Some(&app_name))?;
                } else {
                    self.refresh_accessibility_tree(None)?;
                }
            }
        }

        // Debug: Print the serializable tree of the start_element/root (max depth 2)
        #[cfg(debug_assertions)]
        {
            use serde_json;
            let ui_element = self.wrap_element(start_element.clone());
            let tree = ui_element.to_serializable_tree(2);
            match serde_json::to_string_pretty(&tree) {
                Ok(json_str) => debug!(
                    "start_element serializable tree (max_depth=2):\n{}",
                    json_str
                ),
                Err(e) => debug!("Failed to serialize start_element tree: {}", e),
            }
        }

        debug!("find_element called with selector: {:?}", selector);
        match &actual_selector {
            Selector::Role { role, name } => {
                let target_roles = map_generic_role_to_macos_roles(role);
                let name_filter = name.as_ref().map(|n| n.to_lowercase());

                debug!("target_roles: {:?}", target_roles);
                debug!("name_filter: {:?}", name_filter);

                let collector =
                    ElementsCollectorWithWindows::new(&start_element.0, move |e| match e.role() {
                        Ok(r) => {
                            let current_role = r.to_string();
                            debug!("current_role: {:?}", current_role);

                            if target_roles.contains(&current_role) {
                                if let Some(filter_name) = &name_filter {
                                    let matches_name = e.title().ok().map_or(false, |t| {
                                        t.to_string().to_lowercase().contains(filter_name)
                                    }) || e
                                        .attribute(&AXAttribute::new(&CFString::new("AXLabel")))
                                        .ok()
                                        .and_then(|v| v.downcast_into::<CFString>())
                                        .map_or(false, |s| {
                                            s.to_string().to_lowercase().contains(filter_name)
                                        })
                                        || e.description().ok().map_or(false, |d| {
                                            d.to_string().to_lowercase().contains(filter_name)
                                        })
                                        || e.value()
                                            .ok()
                                            .and_then(|v| v.downcast_into::<CFString>())
                                            .map_or(false, |s| {
                                                s.to_string().to_lowercase().contains(filter_name)
                                            });
                                    matches_name
                                } else {
                                    true
                                }
                            } else {
                                false
                            }
                        }
                        Err(_) => false,
                    }); // Add None for implicit_wait

                // Find all matching elements and print their roles, then return the first one found
                let found_elements = collector.with_max_results(Some(1)).find_all();
                // let found_elements = collector.find_all();

                for elem in &found_elements {
                    match elem.role() {
                        Ok(r) => println!("Found element with role: {}", r),
                        Err(e) => println!("Failed to get role for element: {:?}", e),
                    }
                }

                match found_elements.into_iter().next() {
                    Some(e) => Ok(self.wrap_element(ThreadSafeAXUIElement::new(e))),
                    None => Err(AutomationError::ElementNotFound(format!(
                        "Element with role '{}'{} not found",
                        role,
                        name.as_ref()
                            .map(|n| format!(" and name containing '{}'", n))
                            .unwrap_or_default()
                    ))),
                }
            }
            Selector::Id(id_str) => {
                let target_id_str = id_str.clone();
                let use_bg = self.use_background_apps;
                let activate = self.activate_app;
                let collector = ElementsCollectorWithWindows::new(&start_element.0, move |e| {
                    let macos_elem_wrapper = MacOSUIElement {
                        element: ThreadSafeAXUIElement::new(e.clone()),
                        use_background_apps: use_bg, // Use copied value
                        activate_app: activate,      // Use copied value
                    };
                    let calc_id = macos_elem_wrapper.id();
                    debug!("Element id: {:?}", calc_id);
                    let matches = calc_id
                        .as_ref()
                        .map(|id| id == &target_id_str)
                        .unwrap_or(false);
                    debug!(
                        "Comparing element id: {:?} with target id: {:?} => {}",
                        calc_id, target_id_str, matches
                    );
                    matches
                }); // Ensure only 2 arguments

                match collector.find_all().into_iter().next() {
                    Some(e) => Ok(self.wrap_element(ThreadSafeAXUIElement::new(e))),
                    None => Err(AutomationError::ElementNotFound(format!(
                        "Element with ID '{}' not found",
                        id_str
                    ))),
                }
            }
            Selector::Name(name) => {
                let name_lower = name.to_lowercase();
                let collector = ElementsCollectorWithWindows::new(&start_element.0, move |e| {
                    e.title().ok().map_or(false, |t| {
                        t.to_string().to_lowercase().contains(&name_lower)
                    }) || e
                        .attribute(&AXAttribute::new(&CFString::new("AXLabel")))
                        .ok()
                        .and_then(|v| v.downcast_into::<CFString>())
                        .map_or(false, |s| {
                            s.to_string().to_lowercase().contains(&name_lower)
                        })
                        || e.description().ok().map_or(false, |d| {
                            d.to_string().to_lowercase().contains(&name_lower)
                        })
                        || e.value()
                            .ok()
                            .and_then(|v| v.downcast_into::<CFString>())
                            .map_or(false, |s| {
                                s.to_string().to_lowercase().contains(&name_lower)
                            })
                }); // Ensure only 2 arguments
                    // Find all matching elements and return the first one found
                match collector.find_all().into_iter().next() {
                    Some(e) => Ok(self.wrap_element(ThreadSafeAXUIElement::new(e))),
                    None => Err(AutomationError::ElementNotFound(format!(
                        "Element with name containing '{}' not found",
                        name
                    ))),
                }
            }
            Selector::Text(text) => {
                let text_lower = text.to_lowercase();
                let collector = ElementsCollectorWithWindows::new(&start_element.0, move |e| {
                    element_contains_text(e, &text_lower)
                }); // Add None for implicit_wait
                    // Find all matching elements and return the first one found
                match collector.find_all().into_iter().next() {
                    Some(e) => Ok(self.wrap_element(ThreadSafeAXUIElement::new(e))),
                    None => Err(AutomationError::ElementNotFound(format!(
                        "Element containing text '{}' not found",
                        text
                    ))),
                }
            }
            Selector::Path(_) => Err(AutomationError::UnsupportedOperation(
                "Path selector not yet supported for macOS".to_string(),
            )),
            Selector::NativeId(_) => Err(AutomationError::UnsupportedOperation(
                "NativeId selector not yet supported for macOS".to_string(),
            )),
            Selector::Attributes(_) => Err(AutomationError::UnsupportedOperation(
                "Attributes selector not yet supported for macOS".to_string(),
            )),
            Selector::Filter(_) => Err(AutomationError::UnsupportedOperation(
                "Filter selector not yet supported for macOS".to_string(),
            )),
            Selector::Chain(selectors) => {
                if selectors.is_empty() {
                    return Err(AutomationError::InvalidArgument(
                        "Selector chain cannot be empty".to_string(),
                    ));
                }

                let mut current_element = self.wrap_element(start_element); // Start with the initial root
                debug!("Find with chain, current_element: {:?}", current_element);

                for selector in selectors {
                    // Find exactly one element matching the current selector within the current element
                    let found_elements =
                        self.find_elements(selector, Some(&current_element), timeout, None)?;

                    match found_elements.len() {
                        1 => {
                            // Found exactly one, update current_element for the next iteration
                            current_element = found_elements.into_iter().next().unwrap();
                        }
                        0 => {
                            // Found none, the chain is broken
                            return Err(AutomationError::ElementNotFound(format!(
                                "Element not found for selector {:?} in chain",
                                selector
                            )));
                        }
                        _ => {
                            // Found more than one, ambiguous chain
                            // Replace AmbiguousMatch with PlatformError
                            return Err(AutomationError::PlatformError(format!(
                                "Multiple elements found for selector {:?} in chain, cannot resolve single element",
                                selector
                            )));
                        }
                    }
                }
                // If the loop completes, current_element holds the final result
                Ok(current_element)
            }
            Selector::ClassName(_) => Err(AutomationError::UnsupportedOperation(
                "ClassName selector is not yet supported for macOS".to_string(),
            )),
            Selector::Visible(_) => Err(AutomationError::UnsupportedOperation(
                "Visible selector not yet supported for macOS".to_string(),
            )),
            Selector::LocalizedRole(_) => Err(AutomationError::UnsupportedOperation(
                "LocalizedRole selector is not yet supported for macOS".to_string(),
            )),

            Selector::Invalid(reason) => Err(AutomationError::InvalidArgument(reason.clone())),
            Selector::RightOf(_) | Selector::LeftOf(_) | Selector::Above(_) | Selector::Below(_) | Selector::Near(_) => Err(AutomationError::UnsupportedOperation(
                "Relative selectors (RightOf/LeftOf/Above/Below/Near) are not yet supported for macOS".to_string(),
            )),
            Selector::Nth(_) => Err(AutomationError::UnsupportedOperation(
                "Nth selector not yet supported for macOS".to_string(),
            )),
            Selector::Has(_) => Err(AutomationError::UnsupportedOperation(
                "Has selector not yet supported for macOS".to_string(),
            )),
        }
    }

    fn open_application(&self, app_name: &str) -> Result<UIElement, AutomationError> {
        // Use `open -a` command
        let status = std::process::Command::new("open")
            .arg("-a")
            .arg(app_name)
            .status()
            .map_err(|e| {
                AutomationError::PlatformError(format!("Failed to run open command: {}", e))
            })?;

        if !status.success() {
            return Err(AutomationError::PlatformError(format!(
                "Failed to open application '{}'. 'open -a' command failed.",
                app_name
            )));
        }

        // Wait a bit for the app to launch
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Get the application element
        self.get_application_by_name(app_name)
    }

    fn open_url(
        &self,
        url: &str,
        browser: Option<crate::Browser>,
    ) -> Result<UIElement, AutomationError> {
        let mut command = std::process::Command::new("open");

        // Only handle custom browser paths explicitly; everything else uses the default browser.
        if let Some(crate::Browser::Custom(ref path)) = browser {
            command.arg("-a").arg(path);
        }

        command.arg(url);

        let status = command.status().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to run open command for URL: {}", e))
        })?;

        if !status.success() {
            return Err(AutomationError::PlatformError(format!(
                "Failed to open URL '{}' {:?}. 'open' command failed.",
                url, browser
            )));
        }

        // Wait briefly for the browser to appear.
        std::thread::sleep(std::time::Duration::from_millis(500));

        // If we launched a specific custom browser, try to return its UI element; otherwise we cannot know which app opened.
        if let Some(crate::Browser::Custom(path)) = browser {
            self.get_application_by_name(&path)
        } else {
            Err(AutomationError::UnsupportedOperation(
                "Cannot get UIElement for default browser after opening URL".to_string(),
            ))
        }
    }

    fn open_file(&self, file_path: &str) -> Result<(), AutomationError> {
        let status = std::process::Command::new("open")
            .arg(file_path) // Just pass the file path to `open`
            .status()
            .map_err(|e| {
                AutomationError::PlatformError(format!(
                    "Failed to run open command for file: {}",
                    e
                ))
            })?;

        if !status.success() {
            return Err(AutomationError::PlatformError(format!(
                "Failed to open file '{}'. 'open' command failed.",
                file_path
            )));
        }
        Ok(())
    }

    async fn run_command(
        &self,
        _windows_command: Option<&str>, // Marked as unused
        unix_command: Option<&str>,
    ) -> Result<crate::CommandOutput, AutomationError> {
        // Directly call the implementation logic (previously in impl MacOSEngine)
        let command_str = unix_command.ok_or_else(|| {
            AutomationError::InvalidArgument("Unix command must be provided".to_string())
        })?;

        // Use tokio::process::Command for async execution
        let output = tokio::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(command_str)
            .output()
            .await // Await the async output
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?;

        Ok(crate::CommandOutput {
            exit_status: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    async fn capture_screen(&self) -> Result<ScreenshotResult, AutomationError> {
        // Directly call the implementation logic
        let monitors = xcap::Monitor::all().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitors: {}", e))
        })?;
        let mut primary_monitor: Option<xcap::Monitor> = None;
        for monitor in monitors {
            match monitor.is_primary() {
                Ok(true) => {
                    primary_monitor = Some(monitor);
                    break;
                }
                Ok(false) => continue,
                Err(e) => {
                    return Err(AutomationError::PlatformError(format!(
                        "Error checking monitor primary status: {}",
                        e
                    )));
                }
            }
        }
        let primary_monitor = primary_monitor.ok_or_else(|| {
            AutomationError::PlatformError("Could not find primary monitor".to_string())
        })?;

        let image = primary_monitor.capture_image().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to capture screen: {}", e))
        })?;

        Ok(ScreenshotResult {
            image_data: image.to_vec(),
            width: image.width(),
            height: image.height(),
            monitor: None,
        })
    }

    // ============== NEW MONITOR ABSTRACTIONS ==============

    async fn list_monitors(&self) -> Result<Vec<crate::Monitor>, AutomationError> {
        let monitors = xcap::Monitor::all().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitors: {}", e))
        })?;

        let mut result = Vec::new();
        for (index, monitor) in monitors.iter().enumerate() {
            let name = monitor.name().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor name: {}", e))
            })?;

            let is_primary = monitor.is_primary().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to check primary status: {}", e))
            })?;

            let width = monitor.width().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor width: {}", e))
            })?;

            let height = monitor.height().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor height: {}", e))
            })?;

            let x = monitor.x().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor x position: {}", e))
            })?;

            let y = monitor.y().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor y position: {}", e))
            })?;

            let scale_factor = monitor.scale_factor().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor scale factor: {}", e))
            })? as f64;

            result.push(crate::Monitor {
                id: format!("monitor_{}", index),
                name,
                is_primary,
                width,
                height,
                x,
                y,
                scale_factor,
            });
        }

        Ok(result)
    }

    async fn get_primary_monitor(&self) -> Result<crate::Monitor, AutomationError> {
        let monitors = self.list_monitors().await?;
        monitors
            .into_iter()
            .find(|m| m.is_primary)
            .ok_or_else(|| AutomationError::PlatformError("No primary monitor found".to_string()))
    }

    async fn get_active_monitor(&self) -> Result<crate::Monitor, AutomationError> {
        // Get all windows
        let windows = xcap::Window::all()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get windows: {}", e)))?;

        // Find the focused window
        let focused_window = windows
            .iter()
            .find(|w| w.is_focused().unwrap_or(false))
            .ok_or_else(|| {
                AutomationError::ElementNotFound("No focused window found".to_string())
            })?;

        // Get the monitor for the focused window
        let xcap_monitor = focused_window.current_monitor().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get current monitor: {}", e))
        })?;

        // Convert to our Monitor struct
        let name = xcap_monitor.name().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitor name: {}", e))
        })?;

        let is_primary = xcap_monitor.is_primary().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to check primary status: {}", e))
        })?;

        // Find the monitor index for ID generation
        let monitors = xcap::Monitor::all().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitors: {}", e))
        })?;

        let monitor_index = monitors
            .iter()
            .position(|m| m.name().map(|n| n == name).unwrap_or(false))
            .unwrap_or(0);

        let width = xcap_monitor.width().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitor width: {}", e))
        })?;

        let height = xcap_monitor.height().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitor height: {}", e))
        })?;

        let x = xcap_monitor.x().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitor x position: {}", e))
        })?;

        let y = xcap_monitor.y().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitor y position: {}", e))
        })?;

        let scale_factor = xcap_monitor.scale_factor().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitor scale factor: {}", e))
        })? as f64;

        Ok(crate::Monitor {
            id: format!("monitor_{}", monitor_index),
            name,
            is_primary,
            width,
            height,
            x,
            y,
            scale_factor,
        })
    }

    async fn get_monitor_by_id(&self, id: &str) -> Result<crate::Monitor, AutomationError> {
        let monitors = self.list_monitors().await?;
        monitors.into_iter().find(|m| m.id == id).ok_or_else(|| {
            AutomationError::ElementNotFound(format!("Monitor with ID '{}' not found", id))
        })
    }

    async fn get_monitor_by_name(&self, name: &str) -> Result<crate::Monitor, AutomationError> {
        let monitors = self.list_monitors().await?;
        monitors
            .into_iter()
            .find(|m| m.name == name)
            .ok_or_else(|| {
                AutomationError::ElementNotFound(format!("Monitor '{}' not found", name))
            })
    }

    async fn capture_monitor_by_id(
        &self,
        id: &str,
    ) -> Result<crate::ScreenshotResult, AutomationError> {
        let monitor = self.get_monitor_by_id(id).await?;

        // Find the xcap monitor by name
        let monitors = xcap::Monitor::all().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitors: {}", e))
        })?;

        let xcap_monitor = monitors
            .into_iter()
            .find(|m| m.name().map(|n| n == monitor.name).unwrap_or(false))
            .ok_or_else(|| {
                AutomationError::ElementNotFound(format!("Monitor '{}' not found", monitor.name))
            })?;

        let image = xcap_monitor.capture_image().map_err(|e| {
            AutomationError::PlatformError(format!(
                "Failed to capture monitor '{}': {}",
                monitor.name, e
            ))
        })?;

        Ok(ScreenshotResult {
            image_data: image.to_vec(),
            width: image.width(),
            height: image.height(),
            monitor: Some(monitor),
        })
    }

    // ============== DEPRECATED METHODS ==============

    async fn capture_monitor_by_name(
        &self,
        name: &str,
    ) -> Result<ScreenshotResult, AutomationError> {
        // Directly call the implementation logic
        let monitors = xcap::Monitor::all().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitors: {}", e))
        })?;
        let mut target_monitor: Option<xcap::Monitor> = None;
        for monitor in monitors {
            match monitor.name() {
                Ok(monitor_name) if monitor_name == name => {
                    target_monitor = Some(monitor);
                    break;
                }
                Ok(_) => continue,
                Err(e) => {
                    return Err(AutomationError::PlatformError(format!(
                        "Error getting monitor name: {}",
                        e
                    )));
                }
            }
        }
        let target_monitor = target_monitor.ok_or_else(|| {
            AutomationError::ElementNotFound(format!("Monitor '{}' not found", name))
        })?;

        let image = target_monitor.capture_image().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to capture monitor '{}': {}", name, e))
        })?;

        Ok(ScreenshotResult {
            image_data: image.to_vec(),
            width: image.width(),
            height: image.height(),
            monitor: None,
        })
    }

    async fn ocr_image_path(&self, image_path: &str) -> Result<String, AutomationError> {
        // Call the implementation from the MacOSEngine struct
        // Directly call the implementation logic
        let engine = OcrEngine::new(OcrProvider::Auto).map_err(|e| {
            AutomationError::PlatformError(format!("Failed to create OCR engine: {}", e))
        })?;

        let (text, _language, _confidence) = engine // Destructure the tuple
            .recognize_file(image_path)
            .await
            .map_err(|e| {
                AutomationError::PlatformError(format!("OCR recognition failed: {}", e))
            })?;

        Ok(text) // Return only the text
    }

    async fn ocr_screenshot(
        &self,
        screenshot: &ScreenshotResult,
    ) -> Result<String, AutomationError> {
        // Call the implementation from the MacOSEngine struct
        // Directly call the implementation logic
        // Reconstruct the image buffer from raw data
        let img_buffer: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(
            screenshot.width,
            screenshot.height,
            screenshot.image_data.clone(), // Clone data into the buffer
        )
        .ok_or_else(|| {
            AutomationError::InvalidArgument(
                "Invalid screenshot data for buffer creation".to_string(),
            )
        })?;

        // Convert to DynamicImage
        let dynamic_image = DynamicImage::ImageRgba8(img_buffer);

        let engine = OcrEngine::new(OcrProvider::Auto).map_err(|e| {
            AutomationError::PlatformError(format!("Failed to create OCR engine: {}", e))
        })?;

        let (text, _language, _confidence) = engine
            .recognize_image(&dynamic_image) // Use recognize_image
            .await
            .map_err(|e| {
                AutomationError::PlatformError(format!("OCR recognition failed: {}", e))
            })?;

        Ok(text)
    }

    fn activate_browser_window_by_title(&self, title: &str) -> Result<(), AutomationError> {
        // 1. Find Browser Applications
        let browser_names = [
            "Safari",
            "Google Chrome",
            "Firefox",
            "Arc",
            "Microsoft Edge",
            "Brave Browser",
            "Opera",
            "Vivaldi",
        ];
        let mut browser_apps = Vec::new();
        for name in browser_names {
            if let Ok(app) = self.get_application_by_name(name) {
                if let Some(macos_app) = app.as_any().downcast_ref::<MacOSUIElement>() {
                    browser_apps.push(macos_app.element.clone());
                }
            }
        }

        if browser_apps.is_empty() {
            return Err(AutomationError::ElementNotFound(
                "No common browser applications found running.".to_string(),
            ));
        }

        // 2. Search within each browser for the window/tab
        for browser_ax_element in browser_apps {
            // Search for windows within the browser app
            // Use ElementsCollectorWithWindows
            let title_str = title.to_string();
            let collector = ElementsCollectorWithWindows::new(&browser_ax_element.0, move |e| {
                match e.role() {
                    Ok(role) => {
                        // Check if it's a window and title contains the target text
                        role.to_string() == "AXWindow"
                            && e.title()
                                .map_or(false, |t| t.to_string().contains(&title_str))
                    }
                    Err(_) => false,
                }
            });

            // Check only the first result if needed
            if let Some(window_element) = collector.find_all().into_iter().next() {
                // Found the window, now try to activate it
                let macos_window = self.wrap_element(ThreadSafeAXUIElement::new(window_element));
                return macos_window.activate_window(); // Use the activate_window impl
            }

            // If not found in windows, search for tabs (might be AXRadioButton, AXTab, AXButton etc.)
            // This is less reliable as structure varies greatly.
            // Use ElementsCollectorWithWindows
            let title_str = title.to_string();
            let tab_collector =
                ElementsCollectorWithWindows::new(&browser_ax_element.0, move |e| {
                    match e.role() {
                        Ok(role_str) => {
                            let role = role_str.to_string();
                            // Check common roles for tabs and if title/label contains the target text
                            (role == "AXRadioButton"
                                || role == "AXTab"
                                || role == "AXButton"
                                || role == "AXStaticText")
                                && (e
                                    .title()
                                    .map_or(false, |t| t.to_string().contains(&title_str))
                                    || e.attribute(&AXAttribute::new(&CFString::new("AXLabel")))
                                        .ok()
                                        .and_then(|v| v.downcast_into::<CFString>())
                                        .map_or(false, |s| s.to_string().contains(&title_str)))
                        }
                        Err(_) => false,
                    }
                }); // Add None for implicit_wait

            if let Some(tab_element) = tab_collector.find_all().into_iter().next() {
                // Found a potential tab element. Try to activate its window.
                // We need to walk up the tree to find the containing window.
                let mut current = tab_element.clone(); // Clone here before the loop potentially moves it
                loop {
                    if let Ok(role) = current.role() {
                        if role.to_string() == "AXWindow" {
                            let macos_window =
                                self.wrap_element(ThreadSafeAXUIElement::new(current));
                            return macos_window.activate_window();
                        }
                    }
                    match current.parent() {
                        Ok(parent) => current = parent,
                        Err(_) => break, // Reached top or error
                    }
                }
                // If window activation failed via tab, maybe try clicking the tab?
                // Clone tab_element again before creating macos_tab to fix move error.
                let macos_tab = self.wrap_element(ThreadSafeAXUIElement::new(tab_element.clone()));
                if macos_tab.click().is_ok() {
                    // If click succeeded, try activating the window again
                    if let Ok(Some(window_element_trait_obj)) = macos_tab.parent() {
                        // Downcast to call the concrete role() if needed, or use the trait's role()
                        if let Some(macos_window_element) = window_element_trait_obj
                            .as_any()
                            .downcast_ref::<MacOSUIElement>()
                        {
                            // Check the role using the UIElementImpl trait method
                            if macos_window_element.role() == "AXWindow" {
                                // Call activate_window on the trait object
                                return window_element_trait_obj.activate_window();
                            }
                        }
                    }
                }
            }
        }

        Err(AutomationError::ElementNotFound(format!(
            "Could not find or activate a browser window/tab with title containing: {}",
            title
        )))
    }

    async fn get_current_browser_window(&self) -> Result<UIElement, AutomationError> {
        use accessibility::AXAttribute;
        use core_foundation::string::CFString;
        use objc::{class, msg_send, sel, sel_impl};

        // 1. Get focused application and focused UI element
        let system_wide = accessibility::AXUIElement::system_wide();
        let focused_app_attr = AXAttribute::new(&CFString::new("AXFocusedApplication"));
        let focused_element_attr = AXAttribute::new(&CFString::new("AXFocusedUIElement"));

        let focused_app = system_wide.attribute(&focused_app_attr).map_err(|e| {
            AutomationError::ElementNotFound(format!("Failed to get focused application: {}", e))
        })?;
        let focused_element = system_wide.attribute(&focused_element_attr).map_err(|e| {
            AutomationError::ElementNotFound(format!("Failed to get focused UI element: {}", e))
        })?;

        let app_ax_element = if let Some(ax) = focused_app.downcast::<accessibility::AXUIElement>()
        {
            ax
        } else {
            return Err(AutomationError::ElementNotFound(
                "Focused application attribute did not contain a valid AXUIElement".to_string(),
            ));
        };
        let focused_ax_element =
            if let Some(ax) = focused_element.downcast::<accessibility::AXUIElement>() {
                ax
            } else {
                return Err(AutomationError::ElementNotFound(
                    "Focused UI element attribute did not contain a valid AXUIElement".to_string(),
                ));
            };

        // 2. Get PID and app name
        let pid = {
            // Use accessibility API to get the PID
            unsafe {
                let element_ref =
                    app_ax_element.as_concrete_TypeRef() as *mut ::std::os::raw::c_void;
                #[link(name = "ApplicationServices", kind = "framework")]
                unsafe extern "C" {
                    fn AXUIElementGetPid(
                        element: *mut ::std::os::raw::c_void,
                        pid: *mut i32,
                    ) -> i32;
                }
                let mut pid: i32 = 0;
                let result = AXUIElementGetPid(element_ref, &mut pid);
                if result == 0 {
                    pid
                } else {
                    -1
                }
            }
        };
        if pid == -1 {
            return Err(AutomationError::ElementNotFound(
                "Failed to get PID for focused application".to_string(),
            ));
        }

        // 3. Get app name from PID using Objective-C
        let app_name = unsafe {
            let nsra_class = class!(NSRunningApplication);
            let app: *mut objc::runtime::Object =
                msg_send![nsra_class, runningApplicationWithProcessIdentifier:pid];
            if app.is_null() {
                return Err(AutomationError::ElementNotFound(format!(
                    "No NSRunningApplication for PID {}",
                    pid
                )));
            }
            let app_name_obj: *mut objc::runtime::Object = msg_send![app, localizedName];
            if app_name_obj.is_null() {
                return Err(AutomationError::ElementNotFound(format!(
                    "No localizedName for PID {}",
                    pid
                )));
            }
            let nsstring = app_name_obj as *const objc::runtime::Object;
            let bytes: *const std::os::raw::c_char = msg_send![nsstring, UTF8String];
            let len: usize = msg_send![nsstring, lengthOfBytesUsingEncoding:4]; // NSUTF8StringEncoding = 4
            let bytes_slice = std::slice::from_raw_parts(bytes as *const u8, len);
            std::str::from_utf8_unchecked(bytes_slice).to_string()
        };

        // 4. Check if app is a known browser
        let known_browsers = [
            "Safari",
            "Google Chrome",
            "Firefox",
            "Arc",
            "Microsoft Edge",
            "Brave Browser",
            "Opera",
            "Vivaldi",
        ];
        let is_browser = known_browsers
            .iter()
            .any(|b| app_name.to_lowercase().contains(&b.to_lowercase()));
        if !is_browser {
            return Err(AutomationError::ElementNotFound(format!(
                "Currently focused application '{}' is not a recognized browser.",
                app_name
            )));
        }

        // 5. Try to get the focused window from the app element
        let window = app_ax_element
            .main_window()
            .ok()
            .or_else(|| {
                app_ax_element
                    .windows()
                    .ok()
                    .and_then(|ws| ws.get(0).map(|w| w.to_owned()))
            })
            .unwrap_or(focused_ax_element.clone());

        Ok(self.wrap_element(ThreadSafeAXUIElement::new(window)))
    }

    fn activate_application(&self, app_name: &str) -> Result<(), AutomationError> {
        let app_element = self.get_application_by_name(app_name)?;
        // Downcast to call activate_window directly
        if let Some(macos_el) = app_element.as_any().downcast_ref::<MacOSUIElement>() {
            // Use the focus method which handles activation logic
            macos_el.focus()
        } else {
            Err(AutomationError::PlatformError(
                "Failed to downcast to MacOSUIElement for activation".to_string(),
            ))
        }
    }

    async fn get_current_window(&self) -> Result<UIElement, AutomationError> {
        let focused_element_wrapper = self.get_focused_element()?;
        // Downcast to MacOSUIElement to access the window() method
        if let Some(macos_focused_element) = focused_element_wrapper
            .as_any()
            .downcast_ref::<MacOSUIElement>()
        {
            macos_focused_element.window()?.ok_or_else(|| {
                AutomationError::ElementNotFound(
                    "Could not find a parent window for the focused element.".to_string(),
                )
            })
        } else {
            Err(AutomationError::PlatformError(
                "Focused element is not a MacOSUIElement.".to_string(),
            ))
        }
    }

    async fn get_current_application(&self) -> Result<UIElement, AutomationError> {
        let focused_element_wrapper = self.get_focused_element()?;
        // Downcast to MacOSUIElement to access the application() method
        if let Some(macos_focused_element) = focused_element_wrapper
            .as_any()
            .downcast_ref::<MacOSUIElement>()
        {
            macos_focused_element.application()?.ok_or_else(|| {
                AutomationError::ElementNotFound(
                    "Could not find a parent application for the focused element.".to_string(),
                )
            })
        } else {
            Err(AutomationError::PlatformError(
                "Focused element is not a MacOSUIElement.".to_string(),
            ))
        }
    }

    fn get_window_tree(
        &self,
        pid: u32,
        title: Option<&str>,
        config: crate::platforms::TreeBuildConfig,
    ) -> Result<crate::UINode, AutomationError> {
        use crate::platforms::{PropertyLoadingMode, TreeBuildConfig};
        use crate::UINode;
        use std::time::Instant;
        use tracing::{debug, info, warn};

        info!(
            "[macOS] get_window_tree: pid={}, title={:?}, config={:?}",
            pid, title, config
        );

        // 1. Get the application element by PID
        let app_element = self.get_application_by_pid(pid as i32, None)?;
        let app_ax_element =
            if let Some(macos_el) = app_element.as_any().downcast_ref::<MacOSUIElement>() {
                macos_el.element.0.clone()
            } else {
                return Err(AutomationError::PlatformError(
                    "Failed to downcast to MacOSUIElement".to_string(),
                ));
            };

        // 2. Collect all AXWindow elements for this application
        let windows_collector = ElementsCollectorWithWindows::new(&app_ax_element, |e| {
            e.role().is_ok_and(|r| r.to_string() == "AXWindow")
        });
        let windows = windows_collector.find_all();
        debug!(
            "[macOS] Found {} AXWindow elements for PID {}",
            windows.len(),
            pid
        );
        if windows.is_empty() {
            return Err(AutomationError::ElementNotFound(format!(
                "No windows found for process ID {}.",
                pid
            )));
        }

        // 3. Filter by title if provided
        let mut window_candidates: Vec<(AXUIElement, String)> = Vec::new();
        for win in &windows {
            let title_str = win.title().ok().map(|t| t.to_string()).unwrap_or_default();
            window_candidates.push((win.clone(), title_str));
        }

        let selected_window = if let Some(title_filter) = title {
            let title_filter = title_filter.to_lowercase();
            // Try to find the best match (case-insensitive substring)
            let mut best: Option<(AXUIElement, String)> = None;
            for (win, win_title) in &window_candidates {
                if win_title.to_lowercase().contains(&title_filter) {
                    best = Some((win.clone(), win_title.clone()));
                    break;
                }
            }
            if let Some((win, _)) = best {
                win
            } else {
                warn!(
                    "[macOS] No window title matched '{}', falling back to first window.",
                    title_filter
                );
                window_candidates[0].0.clone()
            }
        } else {
            // No title filter, use the first window
            window_candidates[0].0.clone()
        };

        let window_element = self.wrap_element(ThreadSafeAXUIElement::new(selected_window));

        // 4. Build the UI tree recursively using the config
        struct TreeBuildingContext {
            config: TreeBuildConfig,
            property_mode: PropertyLoadingMode,
            elements_processed: usize,
            max_depth_reached: usize,
            errors_encountered: usize,
        }

        fn build_ui_node_tree_configurable(
            element: &UIElement,
            current_depth: usize,
            context: &mut TreeBuildingContext,
        ) -> Result<UINode, AutomationError> {
            context.elements_processed += 1;
            context.max_depth_reached = context.max_depth_reached.max(current_depth);
            if context.elements_processed % context.config.yield_every_n_elements.unwrap_or(50) == 0
            {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            // Get attributes based on property mode
            let attributes = match &context.property_mode {
                PropertyLoadingMode::Fast => element.attributes(),
                PropertyLoadingMode::Complete => element.attributes(), // TODO: implement full
                PropertyLoadingMode::Smart => element.attributes(),    // TODO: implement smart
            };
            let mut children_nodes = Vec::new();
            match element.children() {
                Ok(children) => {
                    for batch in children.chunks(context.config.batch_size.unwrap_or(50)) {
                        for child in batch {
                            match build_ui_node_tree_configurable(child, current_depth + 1, context)
                            {
                                Ok(child_node) => children_nodes.push(child_node),
                                Err(e) => {
                                    context.errors_encountered += 1;
                                    debug!("[macOS] Failed to process child: {}", e);
                                }
                            }
                        }
                        if batch.len() == context.config.batch_size.unwrap_or(50)
                            && children.len() > context.config.batch_size.unwrap_or(50)
                        {
                            std::thread::sleep(std::time::Duration::from_millis(1));
                        }
                    }
                }
                Err(e) => {
                    context.errors_encountered += 1;
                    debug!("[macOS] Failed to get children: {}", e);
                }
            }
            Ok(UINode {
                id: element.id(),
                attributes,
                children: children_nodes,
            })
        }

        let mut context = TreeBuildingContext {
            config: config.clone(),
            property_mode: config.property_mode.clone(),
            elements_processed: 0,
            max_depth_reached: 0,
            errors_encountered: 0,
        };
        let start = Instant::now();
        let result = build_ui_node_tree_configurable(&window_element, 0, &mut context)?;
        let elapsed = start.elapsed();
        info!(
            "[macOS] Tree built: elements={}, depth={}, errors={}, elapsed_ms={}",
            context.elements_processed,
            context.max_depth_reached,
            context.errors_encountered,
            elapsed.as_millis()
        );
        Ok(result)
    }

    async fn get_active_monitor_name(&self) -> Result<String, AutomationError> {
        let monitor = self.get_active_monitor().await?;
        Ok(monitor.name)
    }

    /// Enable downcasting to concrete engine types
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn press_key(&self, _key: &str) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "press_key is not implemented for MacOSEngine yet".to_string(),
        ))
    }

    fn zoom_in(&self, _level: u32) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "zoom_in is not implemented for MacOSEngine yet".to_string(),
        ))
    }

    fn zoom_out(&self, _level: u32) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "zoom_out is not implemented for MacOSEngine yet".to_string(),
        ))
    }

    fn set_zoom(&self, _percentage: u32) -> Result<(), AutomationError> {
        // On macOS, zoom is typically controlled via Cmd+Plus/Minus/0
        // However, the implementation would be similar to Windows
        // For now, returning unimplemented
        Err(AutomationError::UnsupportedOperation(
            "set_zoom is not implemented for MacOSEngine yet".to_string(),
        ))
    }
}
