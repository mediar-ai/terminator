use napi::bindgen_prelude::FromNapiValue;
use napi::{self};
use napi_derive::napi;
use terminator::{
    UIElement as TerminatorUIElement, UIElementAttributes as TerminatorUIElementAttributes,
};

use crate::{
    map_error, Bounds, ClickResult, FontStyle, HighlightHandle, Locator, ScreenshotResult,
    TextPosition, UIElementAttributes,
};

use crate::Selector;
use napi::bindgen_prelude::Either;

/// A UI element in the accessibility tree.
#[napi(js_name = "Element")]
pub struct Element {
    pub(crate) inner: TerminatorUIElement,
}

impl From<TerminatorUIElement> for Element {
    fn from(e: TerminatorUIElement) -> Self {
        Element { inner: e }
    }
}

impl FromNapiValue for Element {
    unsafe fn from_napi_value(
        env: napi::sys::napi_env,
        napi_val: napi::sys::napi_value,
    ) -> napi::Result<Self> {
        let mut result = std::ptr::null_mut();
        let status = napi::sys::napi_get_value_external(env, napi_val, &mut result);
        if status != napi::sys::Status::napi_ok {
            return Err(napi::Error::new(
                napi::Status::InvalidArg,
                "Failed to get external value",
            ));
        }
        Ok(std::ptr::read(result as *const Element))
    }
}

#[napi]
impl Element {
    /// Get the element's ID.
    ///
    /// @returns {string | null} The element's ID, if available.
    #[napi]
    pub fn id(&self) -> Option<String> {
        self.inner.id()
    }

    /// Get the element's role.
    ///
    /// @returns {string} The element's role (e.g., "button", "textfield").
    #[napi]
    pub fn role(&self) -> napi::Result<String> {
        Ok(self.inner.role())
    }

    /// Get all attributes of the element.
    ///
    /// @returns {UIElementAttributes} The element's attributes.
    #[napi]
    pub fn attributes(&self) -> UIElementAttributes {
        let attrs: TerminatorUIElementAttributes = self.inner.attributes();
        UIElementAttributes {
            role: attrs.role,
            name: attrs.name,
            label: attrs.label,
            value: attrs.value,
            description: attrs.description,
            properties: attrs
                .properties
                .into_iter()
                .map(|(k, v)| (k, v.map(|v| v.to_string())))
                .collect(),
            is_keyboard_focusable: attrs.is_keyboard_focusable,
            bounds: attrs.bounds.map(|(x, y, width, height)| Bounds {
                x,
                y,
                width,
                height,
            }),
        }
    }

    /// Get the element's name.
    ///
    /// @returns {string | null} The element's name, if available.
    #[napi]
    pub fn name(&self) -> napi::Result<Option<String>> {
        Ok(self.inner.name())
    }

    /// Get children of this element.
    ///
    /// @returns {Array<Element>} List of child elements.
    #[napi]
    pub fn children(&self) -> napi::Result<Vec<Element>> {
        self.inner
            .children()
            .map(|kids| kids.into_iter().map(Element::from).collect())
            .map_err(map_error)
    }

    /// Get the parent element.
    ///
    /// @returns {Element | null} The parent element, if available.
    #[napi]
    pub fn parent(&self) -> napi::Result<Option<Element>> {
        self.inner
            .parent()
            .map(|opt| opt.map(Element::from))
            .map_err(map_error)
    }

    /// Get element bounds.
    ///
    /// @returns {Bounds} The element's bounds (x, y, width, height).
    #[napi]
    pub fn bounds(&self) -> napi::Result<Bounds> {
        self.inner.bounds().map(Bounds::from).map_err(map_error)
    }

    /// Click on this element.
    ///
    /// @returns {ClickResult} Result of the click operation.
    #[napi]
    pub fn click(&self) -> napi::Result<ClickResult> {
        self.inner.click().map(ClickResult::from).map_err(map_error)
    }

    /// Double click on this element.
    ///
    /// @returns {ClickResult} Result of the click operation.
    #[napi]
    pub fn double_click(&self) -> napi::Result<ClickResult> {
        self.inner
            .double_click()
            .map(ClickResult::from)
            .map_err(map_error)
    }

    /// Right click on this element.
    #[napi]
    pub fn right_click(&self) -> napi::Result<()> {
        self.inner.right_click().map_err(map_error)
    }

    /// Hover over this element.
    #[napi]
    pub fn hover(&self) -> napi::Result<()> {
        self.inner.hover().map_err(map_error)
    }

    /// Check if element is visible.
    ///
    /// @returns {boolean} True if the element is visible.
    #[napi]
    pub fn is_visible(&self) -> napi::Result<bool> {
        self.inner.is_visible().map_err(map_error)
    }

    /// Check if element is enabled.
    ///
    /// @returns {boolean} True if the element is enabled.
    #[napi]
    pub fn is_enabled(&self) -> napi::Result<bool> {
        self.inner.is_enabled().map_err(map_error)
    }

    /// Focus this element.
    #[napi]
    pub fn focus(&self) -> napi::Result<()> {
        self.inner.focus().map_err(map_error)
    }

    /// Get text content of this element.
    ///
    /// @param {number} [maxDepth] - Maximum depth to search for text.
    /// @returns {string} The element's text content.
    #[napi]
    pub fn text(&self, max_depth: Option<u32>) -> napi::Result<String> {
        self.inner
            .text(max_depth.unwrap_or(1) as usize)
            .map_err(map_error)
    }

    /// Type text into this element.
    ///
    /// @param {string} text - The text to type.
    /// @param {boolean} [useClipboard] - Whether to use clipboard for pasting.
    #[napi]
    pub fn type_text(&self, text: String, use_clipboard: Option<bool>) -> napi::Result<()> {
        self.inner
            .type_text(&text, use_clipboard.unwrap_or(false))
            .map_err(map_error)
    }

    /// Press a key while this element is focused.
    ///
    /// @param {string} key - The key to press.
    #[napi]
    pub fn press_key(&self, key: String) -> napi::Result<()> {
        self.inner.press_key(&key).map_err(map_error)
    }

    /// Set value of this element.
    ///
    /// @param {string} value - The value to set.
    #[napi]
    pub fn set_value(&self, value: String) -> napi::Result<()> {
        self.inner.set_value(&value).map_err(map_error)
    }

    /// Perform a named action on this element.
    ///
    /// @param {string} action - The action to perform.
    #[napi]
    pub fn perform_action(&self, action: String) -> napi::Result<()> {
        self.inner.perform_action(&action).map_err(map_error)
    }

    /// Invoke this element (triggers the default action).
    /// This is often more reliable than clicking for controls like radio buttons or menu items.
    #[napi]
    pub fn invoke(&self) -> napi::Result<()> {
        self.inner.invoke().map_err(map_error)
    }

    /// Scroll the element in a given direction.
    ///
    /// @param {string} direction - The direction to scroll.
    /// @param {number} amount - The amount to scroll.
    #[napi]
    pub fn scroll(&self, direction: String, amount: f64) -> napi::Result<()> {
        self.inner.scroll(&direction, amount).map_err(map_error)
    }

    /// Activate the window containing this element.
    #[napi]
    pub fn activate_window(&self) -> napi::Result<()> {
        self.inner.activate_window().map_err(map_error)
    }

    /// Minimize the window containing this element.
    #[napi]
    pub fn minimize_window(&self) -> napi::Result<()> {
        self.inner.minimize_window().map_err(map_error)
    }

    /// Maximize the window containing this element.
    #[napi]
    pub fn maximize_window(&self) -> napi::Result<()> {
        self.inner.maximize_window().map_err(map_error)
    }

    /// Check if element is focused.
    ///
    /// @returns {boolean} True if the element is focused.
    #[napi]
    pub fn is_focused(&self) -> napi::Result<bool> {
        self.inner.is_focused().map_err(map_error)
    }

    /// Check if element is keyboard focusable.
    ///
    /// @returns {boolean} True if the element can receive keyboard focus.
    #[napi]
    pub fn is_keyboard_focusable(&self) -> napi::Result<bool> {
        self.inner.is_keyboard_focusable().map_err(map_error)
    }

    /// Drag mouse from start to end coordinates.
    ///
    /// @param {number} startX - Starting X coordinate.
    /// @param {number} startY - Starting Y coordinate.
    /// @param {number} endX - Ending X coordinate.
    /// @param {number} endY - Ending Y coordinate.
    #[napi]
    pub fn mouse_drag(
        &self,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
    ) -> napi::Result<()> {
        self.inner
            .mouse_drag(start_x, start_y, end_x, end_y)
            .map_err(map_error)
    }

    /// Press and hold mouse at coordinates.
    ///
    /// @param {number} x - X coordinate.
    /// @param {number} y - Y coordinate.
    #[napi]
    pub fn mouse_click_and_hold(&self, x: f64, y: f64) -> napi::Result<()> {
        self.inner.mouse_click_and_hold(x, y).map_err(map_error)
    }

    /// Move mouse to coordinates.
    ///
    /// @param {number} x - X coordinate.
    /// @param {number} y - Y coordinate.
    #[napi]
    pub fn mouse_move(&self, x: f64, y: f64) -> napi::Result<()> {
        self.inner.mouse_move(x, y).map_err(map_error)
    }

    /// Release mouse button.
    #[napi]
    pub fn mouse_release(&self) -> napi::Result<()> {
        self.inner.mouse_release().map_err(map_error)
    }

    /// Create a locator from this element.
    /// Accepts either a selector string or a Selector object.
    ///
    /// @param {string | Selector} selector - The selector.
    /// @returns {Locator} A new locator for finding elements.
    #[napi]
    pub fn locator(
        &self,
        #[napi(ts_arg_type = "string | Selector")] selector: Either<String, &Selector>,
    ) -> napi::Result<Locator> {
        use napi::bindgen_prelude::Either::*;
        let sel_rust: terminator::selector::Selector = match selector {
            A(sel_str) => sel_str.as_str().into(),
            B(sel_obj) => sel_obj.inner.clone(),
        };
        let loc = self.inner.locator(sel_rust).map_err(map_error)?;
        Ok(Locator::from(loc))
    }

    /// Get the containing application element.
    ///
    /// @returns {Element | null} The containing application element, if available.
    #[napi]
    pub fn application(&self) -> napi::Result<Option<Element>> {
        self.inner
            .application()
            .map(|opt| opt.map(Element::from))
            .map_err(map_error)
    }

    /// Get the containing window element.
    ///
    /// @returns {Element | null} The containing window element, if available.
    #[napi]
    pub fn window(&self) -> napi::Result<Option<Element>> {
        self.inner
            .window()
            .map(|opt| opt.map(Element::from))
            .map_err(map_error)
    }

    /// Highlights the element with a colored border and optional text overlay.
    ///
    /// @param {number} [color] - Optional BGR color code (32-bit integer). Default: 0x0000FF (red)
    /// @param {number} [durationMs] - Optional duration in milliseconds.
    /// @param {string} [text] - Optional text to display. Text will be truncated to 10 characters.
    /// @param {TextPosition} [textPosition] - Optional position for the text overlay (default: Top)
    /// @param {FontStyle} [fontStyle] - Optional font styling for the text
    /// @returns {HighlightHandle} Handle that can be used to close the highlight early
    #[napi]
    pub fn highlight(
        &self,
        color: Option<u32>,
        duration_ms: Option<f64>,
        text: Option<String>,
        text_position: Option<TextPosition>,
        font_style: Option<FontStyle>,
    ) -> napi::Result<HighlightHandle> {
        let duration = duration_ms.map(|ms| std::time::Duration::from_millis(ms as u64));

        #[cfg(target_os = "windows")]
        {
            let rust_text_position = text_position.map(|pos| pos.into());
            let rust_font_style = font_style.map(|style| style.into());

            let handle = self
                .inner
                .highlight(
                    color,
                    duration,
                    text.as_deref(),
                    rust_text_position,
                    rust_font_style,
                )
                .map_err(map_error)?;

            Ok(HighlightHandle::new(handle))
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = (color, duration, text, text_position, font_style);
            Ok(HighlightHandle::new_dummy())
        }
    }

    /// Capture a screenshot of this element.
    ///
    /// @returns {ScreenshotResult} The screenshot data containing image data and dimensions.
    #[napi]
    pub fn capture(&self) -> napi::Result<ScreenshotResult> {
        self.inner
            .capture()
            .map(|result| ScreenshotResult {
                image_data: result.image_data,
                width: result.width,
                height: result.height,
                monitor: result.monitor.map(crate::types::Monitor::from),
            })
            .map_err(map_error)
    }

    /// Get the process ID of the application containing this element.
    ///
    /// @returns {number} The process ID.
    #[napi]
    pub fn process_id(&self) -> napi::Result<u32> {
        self.inner.process_id().map_err(map_error)
    }

    #[napi]
    pub fn to_string(&self) -> napi::Result<String> {
        let id_part = self.inner.id().map_or("null".to_string(), |id| id);

        let attrs = self.inner.attributes();
        let json =
            serde_json::to_string(&attrs).map_err(|e| napi::Error::from_reason(e.to_string()))?;

        Ok(format!("Element<{id_part}, {json}>"))
    }

    /// Sets the transparency of the window.
    ///
    /// @param {number} percentage - The transparency percentage from 0 (completely transparent) to 100 (completely opaque).
    /// @returns {void}
    #[napi]
    pub fn set_transparency(&self, percentage: u8) -> napi::Result<()> {
        self.inner.set_transparency(percentage).map_err(map_error)
    }

    /// Close the element if it's closable (like windows, applications).
    /// Does nothing for non-closable elements (like buttons, text, etc.).
    ///
    /// @returns {void}
    #[napi]
    pub fn close(&self) -> napi::Result<()> {
        self.inner.close().map_err(map_error)
    }

    /// Get the monitor containing this element.
    ///
    /// @returns {Monitor} The monitor information for the display containing this element.
    #[napi]
    pub fn monitor(&self) -> napi::Result<crate::types::Monitor> {
        self.inner
            .monitor()
            .map(crate::types::Monitor::from)
            .map_err(map_error)
    }

    /// Selects an option in a dropdown or combobox by its visible text.
    ///
    /// @param {string} optionName - The visible text of the option to select.
    /// @returns {void}
    #[napi]
    pub fn select_option(&self, option_name: String) -> napi::Result<()> {
        self.inner.select_option(&option_name).map_err(map_error)
    }

    /// Lists all available option strings from a dropdown or list box.
    ///
    /// @returns {Array<string>} List of available option strings.
    #[napi]
    pub fn list_options(&self) -> napi::Result<Vec<String>> {
        self.inner.list_options().map_err(map_error)
    }

    /// Checks if a control (like a checkbox or toggle switch) is currently toggled on.
    ///
    /// @returns {boolean} True if the control is toggled on.
    #[napi]
    pub fn is_toggled(&self) -> napi::Result<bool> {
        self.inner.is_toggled().map_err(map_error)
    }

    /// Sets the state of a toggleable control.
    /// It only performs an action if the control is not already in the desired state.
    ///
    /// @param {boolean} state - The desired toggle state.
    /// @returns {void}
    #[napi]
    pub fn set_toggled(&self, state: bool) -> napi::Result<()> {
        self.inner.set_toggled(state).map_err(map_error)
    }

    /// Execute JavaScript in web browser using dev tools console.
    /// Returns the result of the script execution as a string.
    ///
    /// @param {string} script - The JavaScript code to execute.
    /// @returns {Promise<string>} The result of script execution.
    #[napi]
    pub async fn execute_browser_script(&self, script: String) -> napi::Result<String> {
        self.inner
            .execute_browser_script(&script)
            .await
            .map_err(map_error)
    }
}
