use pyo3::prelude::*;
use pyo3_stub_gen::derive::*;
use ::terminator_core::element::UIElement as TerminatorUIElement;
use crate::exceptions::automation_error_to_pyerr;
use crate::types::{UIElementAttributes, Bounds, ClickResult};
use serde::ser::{Serialize, Serializer, SerializeStruct};

/// Represents a UI element in the desktop UI tree.
#[gen_stub_pyclass]
#[pyclass(name = "UIElement")]
#[derive(Clone)]
pub struct UIElement {
    pub inner: TerminatorUIElement,
}

impl Serialize for UIElement {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("UIElement", 3)?;
        state.serialize_field("role", &self.inner.role())?;
        state.serialize_field("name", &self.inner.name())?;
        state.serialize_field("id", &self.inner.id())?;
        // Optionally add more fields, e.g. attributes, bounds, etc.
        state.end()
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl UIElement {
    #[pyo3(name = "role", text_signature = "($self)")]
    /// Get the element's role (e.g., "button", "textfield").
    /// 
    /// Returns:
    ///     str: The element's role.
    pub fn role(&self) -> String {
        self.inner.role()
    }

    #[pyo3(name = "name", text_signature = "($self)")]
    /// Get the element's name.
    /// 
    /// Returns:
    ///     Optional[str]: The element's name, if available.
    pub fn name(&self) -> Option<String> {
        self.inner.name()
    }

    #[pyo3(name = "id", text_signature = "($self)")]
    /// Get the element's ID.
    /// 
    /// Returns:
    ///     Optional[str]: The element's ID, if available.
    pub fn id(&self) -> Option<String> {
        self.inner.id()
    }

    #[pyo3(name = "attributes", text_signature = "($self)")]
    /// Get all attributes of the element.
    /// 
    /// Returns:
    ///     UIElementAttributes: The element's attributes.
    pub fn attributes(&self) -> PyResult<UIElementAttributes> {
        let attrs = self.inner.attributes();
        Ok(UIElementAttributes {
            role: attrs.role,
            name: attrs.name,
            label: attrs.label,
            value: attrs.value,
            description: attrs.description,
            properties: attrs.properties.into_iter()
                .map(|(k, v)| (k, v.map(|v| v.to_string())))
                .collect(),
            is_keyboard_focusable: attrs.is_keyboard_focusable,
        })
    }

    #[pyo3(name = "children", text_signature = "($self)")]
    /// Get child elements.
    /// 
    /// Returns:
    ///     List[UIElement]: List of child elements.
    pub fn children(&self) -> PyResult<Vec<UIElement>> {
        self.inner.children()
            .map(|kids| kids.into_iter().map(|e| UIElement { inner: e }).collect())
            .map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "parent", text_signature = "($self)")]
    /// Get parent element.
    /// 
    /// Returns:
    ///     Optional[UIElement]: The parent element, if available.
    pub fn parent(&self) -> PyResult<Option<UIElement>> {
        self.inner.parent()
            .map(|opt| opt.map(|e| UIElement { inner: e }))
            .map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "bounds", text_signature = "($self)")]
    /// Get element bounds (x, y, width, height).
    /// 
    /// Returns:
    ///     Bounds: The element's bounds.
    pub fn bounds(&self) -> PyResult<Bounds> {
        let (x, y, width, height) = self.inner.bounds().map_err(|e| automation_error_to_pyerr(e))?;
        Ok(Bounds { x, y, width, height })
    }

    #[pyo3(name = "click", text_signature = "($self)")]
    /// Click on this element.
    /// 
    /// Returns:
    ///     ClickResult: Result of the click operation.
    pub fn click(&self) -> PyResult<ClickResult> {
        self.inner.click()
            .map(ClickResult::from)
            .map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "double_click", text_signature = "($self)")]
    /// Double click on this element.
    /// 
    /// Returns:
    ///     ClickResult: Result of the click operation.
    pub fn double_click(&self) -> PyResult<ClickResult> {
        self.inner.double_click()
            .map(ClickResult::from)
            .map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "right_click", text_signature = "($self)")]
    /// Right click on this element.
    /// 
    /// Returns:
    ///     None
    pub fn right_click(&self) -> PyResult<()> {
        self.inner.right_click()
            .map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "hover", text_signature = "($self)")]
    /// Hover over this element.
    /// 
    /// Returns:
    ///     None
    pub fn hover(&self) -> PyResult<()> {
        self.inner.hover()
            .map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "is_visible", text_signature = "($self)")]
    /// Check if element is visible.
    /// 
    /// Returns:
    ///     bool: True if the element is visible.
    pub fn is_visible(&self) -> PyResult<bool> {
        self.inner.is_visible().map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "is_enabled", text_signature = "($self)")]
    /// Check if element is enabled.
    /// 
    /// Returns:
    ///     bool: True if the element is enabled.
    pub fn is_enabled(&self) -> PyResult<bool> {
        self.inner.is_enabled().map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "focus", text_signature = "($self)")]
    /// Focus this element.
    /// 
    /// Returns:
    ///     None
    pub fn focus(&self) -> PyResult<()> {
        self.inner.focus().map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "text", signature = (max_depth=None))]
    #[pyo3(text_signature = "($self, max_depth)")]
    /// Get text content of this element.
    /// 
    /// Args:
    ///     max_depth (Optional[int]): Maximum depth to search for text.
    /// 
    /// Returns:
    ///     str: The element's text content.
    pub fn text(&self, max_depth: Option<usize>) -> PyResult<String> {
        self.inner.text(max_depth.unwrap_or(1)).map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "type_text", signature = (text, use_clipboard=None))]
    #[pyo3(text_signature = "($self, text, use_clipboard)")]
    /// Type text into this element.
    /// 
    /// Args:
    ///     text (str): The text to type.
    ///     use_clipboard (Optional[bool]): Whether to use clipboard for pasting.
    /// 
    /// Returns:
    ///     None
    pub fn type_text(&self, text: &str, use_clipboard: Option<bool>) -> PyResult<()> {
        self.inner.type_text(text, use_clipboard.unwrap_or(false)).map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "press_key", text_signature = "($self, key)")]
    /// Press a key while this element is focused.
    /// 
    /// Args:
    ///     key (str): The key to press.
    /// 
    /// Returns:
    ///     None
    pub fn press_key(&self, key: &str) -> PyResult<()> {
        self.inner.press_key(key).map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "set_value", text_signature = "($self, value)")]
    /// Set value of this element.
    /// 
    /// Args:
    ///     value (str): The value to set.
    /// 
    /// Returns:
    ///     None
    pub fn set_value(&self, value: &str) -> PyResult<()> {
        self.inner.set_value(value).map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "perform_action", text_signature = "($self, action)")]
    /// Perform a named action on this element.
    /// 
    /// Args:
    ///     action (str): The action to perform.
    /// 
    /// Returns:
    ///     None
    pub fn perform_action(&self, action: &str) -> PyResult<()> {
        self.inner.perform_action(action).map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "scroll", text_signature = "($self, direction, amount)")]
    /// Scroll the element in a given direction.
    /// 
    /// Args:
    ///     direction (str): The direction to scroll.
    ///     amount (float): The amount to scroll.
    /// 
    /// Returns:
    ///     None
    pub fn scroll(&self, direction: &str, amount: f64) -> PyResult<()> {
        self.inner.scroll(direction, amount).map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "activate_window", text_signature = "($self)")]
    /// Activate the window containing this element.
    /// 
    /// Returns:
    ///     None
    pub fn activate_window(&self) -> PyResult<()> {
        self.inner.activate_window().map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "is_focused", text_signature = "($self)")]
    /// Check if element is focused.
    /// 
    /// Returns:
    ///     bool: True if the element is focused.
    pub fn is_focused(&self) -> PyResult<bool> {
        self.inner.is_focused().map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "is_keyboard_focusable", text_signature = "($self)")]
    /// Check if element is keyboard focusable.
    /// 
    /// Returns:
    ///     bool: True if the element can receive keyboard focus.
    pub fn is_keyboard_focusable(&self) -> PyResult<bool> {
        self.inner.is_keyboard_focusable().map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "mouse_drag", text_signature = "($self, start_x, start_y, end_x, end_y)")]
    /// Drag mouse from start to end coordinates.
    /// 
    /// Args:
    ///     start_x (float): Starting X coordinate.
    ///     start_y (float): Starting Y coordinate.
    ///     end_x (float): Ending X coordinate.
    ///     end_y (float): Ending Y coordinate.
    /// 
    /// Returns:
    ///     None
    pub fn mouse_drag(&self, start_x: f64, start_y: f64, end_x: f64, end_y: f64) -> PyResult<()> {
        self.inner.mouse_drag(start_x, start_y, end_x, end_y).map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "mouse_click_and_hold", text_signature = "($self, x, y)")]
    /// Press and hold mouse at coordinates.
    /// 
    /// Args:
    ///     x (float): X coordinate.
    ///     y (float): Y coordinate.
    /// 
    /// Returns:
    ///     None
    pub fn mouse_click_and_hold(&self, x: f64, y: f64) -> PyResult<()> {
        self.inner.mouse_click_and_hold(x, y).map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "mouse_move", text_signature = "($self, x, y)")]
    /// Move mouse to coordinates.
    /// 
    /// Args:
    ///     x (float): X coordinate.
    ///     y (float): Y coordinate.
    /// 
    /// Returns:
    ///     None
    pub fn mouse_move(&self, x: f64, y: f64) -> PyResult<()> {
        self.inner.mouse_move(x, y).map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "mouse_release", text_signature = "($self)")]
    /// Release mouse button.
    /// 
    /// Returns:
    ///     None
    pub fn mouse_release(&self) -> PyResult<()> {
        self.inner.mouse_release().map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "application", text_signature = "($self)")]
    /// Get the containing application element.
    /// 
    /// Returns:
    ///     Optional[UIElement]: The containing application element, if available.
    pub fn application(&self) -> PyResult<Option<UIElement>> {
        self.inner.application()
            .map(|opt| opt.map(|e| UIElement { inner: e }))
            .map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "window", text_signature = "($self)")]
    /// Get the containing window element.
    /// 
    /// Returns:
    ///     Optional[UIElement]: The containing window element, if available.
    pub fn window(&self) -> PyResult<Option<UIElement>> {
        self.inner.window()
            .map(|opt| opt.map(|e| UIElement { inner: e }))
            .map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "locator", text_signature = "($self, selector)")]
    /// Create a locator from this element.
    /// 
    /// Args:
    ///     selector (str): The selector string.
    /// 
    /// Returns:
    ///     Locator: A new locator for finding elements.
    pub fn locator(&self, selector: &str) -> PyResult<crate::locator::Locator> {
        let locator = self.inner.locator(selector).map_err(|e| automation_error_to_pyerr(e))?;
        Ok(crate::locator::Locator { inner: locator })
    }

    #[pyo3(name = "explore", text_signature = "($self)")]
    /// Explore this element and its direct children.
    /// 
    /// Returns:
    ///     ExploreResponse: Details about the element and its children.
    pub fn explore(&self) -> PyResult<crate::types::ExploreResponse> {
        self.inner.explore()
            .map(|response| crate::types::ExploreResponse {
                parent: UIElement { inner: response.parent },
                children: response.children.into_iter().map(|child| crate::types::ExploredElementDetail {
                    role: child.role,
                    name: child.name,
                    id: child.id,
                    bounds: child.bounds.map(|(x, y, width, height)| Bounds { x, y, width, height }),
                    value: child.value,
                    description: child.description,
                    text: child.text,
                    parent_id: child.parent_id,
                    children_ids: child.children_ids,
                    suggested_selector: child.suggested_selector,
                }).collect(),
            })
            .map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "process_id", text_signature = "($self)")]
    /// Get the process ID of the application containing this element.
    /// 
    /// Returns:
    ///     int: The process ID.
    pub fn process_id(&self) -> PyResult<u32> {
        self.inner.process_id().map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "highlight", signature = (color=None, duration_ms=None))]
    #[pyo3(text_signature = "($self, color, duration_ms)")]
    /// Highlights the element with a colored border.
    /// 
    /// Args:
    ///     color (Optional[int]): BGR color code (32-bit integer). Default: 0x0000FF (red)
    ///     duration_ms (Optional[int]): Duration in milliseconds.
    /// 
    /// Returns:
    ///     None
    pub fn highlight(&self, color: Option<u32>, duration_ms: Option<u64>) -> PyResult<()> {
        let duration = duration_ms.map(std::time::Duration::from_millis);
        self.inner.highlight(color, duration).map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "capture", text_signature = "($self)")]
    /// Capture a screenshot of this element.
    /// 
    /// Returns:
    ///     ScreenshotResult: The screenshot data containing image data and dimensions.
    pub fn capture(&self) -> PyResult<crate::types::ScreenshotResult> {
        self.inner.capture()
            .map(|result| crate::types::ScreenshotResult {
                image_data: result.image_data,
                width: result.width,
                height: result.height,
            })
            .map_err(|e| automation_error_to_pyerr(e))
    }
} 