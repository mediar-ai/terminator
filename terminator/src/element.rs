use crate::errors::AutomationError;
use crate::selector::Selector;
use std::collections::HashMap;
use std::fmt::Debug;
use std::time::Instant;
use tracing::{info, instrument, warn};

use super::{ClickResult, Locator};

/// Represents a UI element in a desktop application
#[derive(Debug)]
pub struct UIElement {
    inner: Box<dyn UIElementImpl>,
}

/// Attributes associated with a UI element
#[derive(Debug)]
pub struct UIElementAttributes {
    pub role: String,
    pub name: Option<String>,
    pub label: Option<String>,
    pub value: Option<String>,
    pub description: Option<String>,
    pub properties: HashMap<String, Option<serde_json::Value>>,
    pub is_keyboard_focusable: Option<bool>,
}

/// Interface for platform-specific element implementations
pub(crate) trait UIElementImpl: Send + Sync + Debug {
    fn object_id(&self) -> usize;
    fn id(&self) -> Option<String>;
    fn role(&self) -> String;
    fn attributes(&self) -> UIElementAttributes;
    fn name(&self) -> Option<String> {
        self.attributes().name
    }
    fn children(&self) -> Result<Vec<UIElement>, AutomationError>;
    fn parent(&self) -> Result<Option<UIElement>, AutomationError>;
    fn bounds(&self) -> Result<(f64, f64, f64, f64), AutomationError>; // x, y, width, height
    fn click(&self) -> Result<ClickResult, AutomationError>;
    fn double_click(&self) -> Result<ClickResult, AutomationError>;
    fn right_click(&self) -> Result<(), AutomationError>;
    fn hover(&self) -> Result<(), AutomationError>;
    fn focus(&self) -> Result<(), AutomationError>;
    fn type_text(&self, text: &str, use_clipboard: bool) -> Result<(), AutomationError>;
    fn press_key(&self, key: &str) -> Result<(), AutomationError>;
    fn get_text(&self, max_depth: usize) -> Result<String, AutomationError>;
    fn set_value(&self, value: &str) -> Result<(), AutomationError>;
    fn is_enabled(&self) -> Result<bool, AutomationError>;
    fn is_visible(&self) -> Result<bool, AutomationError>;
    fn is_focused(&self) -> Result<bool, AutomationError>;
    fn perform_action(&self, action: &str) -> Result<(), AutomationError>;
    fn as_any(&self) -> &dyn std::any::Any;
    fn create_locator(&self, selector: Selector) -> Result<Locator, AutomationError>;
    fn scroll(&self, direction: &str, amount: f64) -> Result<(), AutomationError>;

    // New method to activate the window containing the element
    fn activate_window(&self) -> Result<(), AutomationError>;

    // Add a method to clone the box
    fn clone_box(&self) -> Box<dyn UIElementImpl>;

    // New method for keyboard focusable
    fn is_keyboard_focusable(&self) -> Result<bool, AutomationError>;

    // New method for mouse drag
    fn mouse_drag(&self, start_x: f64, start_y: f64, end_x: f64, end_y: f64) -> Result<(), AutomationError>;

    // New methods for mouse control
    fn mouse_click_and_hold(&self, x: f64, y: f64) -> Result<(), AutomationError>;
    fn mouse_move(&self, x: f64, y: f64) -> Result<(), AutomationError>;
    fn mouse_release(&self) -> Result<(), AutomationError>;
}

impl UIElement {
    /// Create a new UI element from a platform-specific implementation
    pub(crate) fn new(impl_: Box<dyn UIElementImpl>) -> Self {
        Self { inner: impl_ }
    }

    /// Get the element's ID
    #[instrument(skip(self))]
    pub fn id(&self) -> Option<String> {
        let start = Instant::now();
        info!("Getting element ID");
        
        let id = self.inner.id();
        
        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            element_id = id.as_deref().unwrap_or_default(),
            "Element ID retrieved"
        );
        
        id
    }

    /// Get the element's role (e.g., "button", "textfield")
    pub fn role(&self) -> String {
        self.inner.role()
    }

    /// Get all attributes of the element
    pub fn attributes(&self) -> UIElementAttributes {
        self.inner.attributes()
    }

    /// Get child elements
    pub fn children(&self) -> Result<Vec<UIElement>, AutomationError> {
        self.inner.children()
    }

    /// Get parent element
    pub fn parent(&self) -> Result<Option<UIElement>, AutomationError> {
        self.inner.parent()
    }

    /// Get element bounds (x, y, width, height)
    pub fn bounds(&self) -> Result<(f64, f64, f64, f64), AutomationError> {
        self.inner.bounds()
    }

    /// Click on this element
    #[instrument(skip(self))]
    pub fn click(&self) -> Result<ClickResult, AutomationError> {
        let start = Instant::now();
        info!("Clicking element");
        
        let result = self.inner.click();
        
        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            "Element clicked"
        );
        
        result
    }

    /// Double-click on this element
    #[instrument(skip(self))]
    pub fn double_click(&self) -> Result<ClickResult, AutomationError> {
        let start = Instant::now();
        info!("Double clicking element");
        
        let result = self.inner.double_click();
        
        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            "Element double clicked"
        );
        
        result
    }

    /// Right-click on this element
    #[instrument(skip(self))]
    pub fn right_click(&self) -> Result<(), AutomationError> {
        let start = Instant::now();
        info!("Right clicking element");
        
        let result = self.inner.right_click();
        
        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            "Element right clicked"
        );
        
        result
    }

    /// Hover over this element
    pub fn hover(&self) -> Result<(), AutomationError> {
        self.inner.hover()
    }

    /// Focus this element
    pub fn focus(&self) -> Result<(), AutomationError> {
        self.inner.focus()
    }

    /// Type text into this element
    pub fn type_text(&self, text: &str, use_clipboard: bool) -> Result<(), AutomationError> {
        self.inner.type_text(text, use_clipboard)
    }

    /// Press a key while this element is focused
    pub fn press_key(&self, key: &str) -> Result<(), AutomationError> {
        self.inner.press_key(key)
    }

    /// Get text content of this element
    pub fn text(&self, max_depth: usize) -> Result<String, AutomationError> {
        self.inner.get_text(max_depth)
    }

    /// Set value of this element
    pub fn set_value(&self, value: &str) -> Result<(), AutomationError> {
        self.inner.set_value(value)
    }

    /// Check if element is enabled
    #[instrument(skip(self))]
    pub fn is_enabled(&self) -> Result<bool, AutomationError> {
        let start = Instant::now();
        info!("Checking if element is enabled");
        
        let is_enabled = self.inner.is_enabled()?;
        
        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            is_enabled,
            "Element enabled status checked"
        );
        
        Ok(is_enabled)
    }

    /// Check if element is visible
    pub fn is_visible(&self) -> Result<bool, AutomationError> {
        self.inner.is_visible()
    }

    /// Check if element is focused
    pub fn is_focused(&self) -> Result<bool, AutomationError> {
        self.inner.is_focused()
    }

    /// Perform a named action on this element
    pub fn perform_action(&self, action: &str) -> Result<(), AutomationError> {
        self.inner.perform_action(action)
    }

    /// Get the underlying implementation as a specific type
    pub(crate) fn as_any(&self) -> &dyn std::any::Any {
        self.inner.as_any()
    }

    /// Find elements matching the selector within this element
    pub fn locator(&self, selector: impl Into<Selector>) -> Result<Locator, AutomationError> {
        let selector = selector.into();
        self.inner.create_locator(selector)
    }

    /// Scroll the element in a given direction
    pub fn scroll(&self, direction: &str, amount: f64) -> Result<(), AutomationError> {
        self.inner.scroll(direction, amount)
    }

    /// Activate the window containing this element (bring to foreground)
    pub fn activate_window(&self) -> Result<(), AutomationError> {
        self.inner.activate_window()
    }

    /// Get the element's name
    #[instrument(skip(self))]
    pub fn name(&self) -> Option<String> {
        let start = Instant::now();
        info!("Getting element name");
        
        let name = self.inner.name();
        
        let duration = start.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            element_name = name.as_deref().unwrap_or_default(),
            "Element name retrieved"
        );
        
        name
    }

    /// Check if element is keyboard focusable
    pub fn is_keyboard_focusable(&self) -> Result<bool, AutomationError> {
        self.inner.is_keyboard_focusable()
    }

    /// Drag mouse from start to end coordinates
    pub fn mouse_drag(&self, start_x: f64, start_y: f64, end_x: f64, end_y: f64) -> Result<(), AutomationError> {
        self.inner.mouse_drag(start_x, start_y, end_x, end_y)
    }

    /// Press and hold mouse at (x, y)
    pub fn mouse_click_and_hold(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        self.inner.mouse_click_and_hold(x, y)
    }

    /// Move mouse to (x, y)
    pub fn mouse_move(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        self.inner.mouse_move(x, y)
    }

    /// Release mouse button
    pub fn mouse_release(&self) -> Result<(), AutomationError> {
        self.inner.mouse_release()
    }
}

impl PartialEq for UIElement {
    fn eq(&self, other: &Self) -> bool {
        self.inner.object_id() == other.inner.object_id()
    }
}

impl Eq for UIElement {}

impl std::hash::Hash for UIElement {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.object_id().hash(state);
    }
}

impl Clone for UIElement {
    fn clone(&self) -> Self {
        // We can't directly clone the inner Box<dyn UIElementImpl>,
        // but we can create a new UIElement with the same identity
        // that will behave the same way
        Self {
            inner: self.inner.clone_box(),
        }
    }
}
