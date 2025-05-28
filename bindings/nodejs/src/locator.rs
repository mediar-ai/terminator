use napi_derive::napi;
use terminator::Locator as TerminatorLocator;

use crate::{
    Element,
    Bounds,
    ClickResult,
    UIElementAttributes,
    map_error,
    types::{ExploreResponse, ExploredElementDetail},
};

/// Locator for finding UI elements by selector.
#[napi(js_name = "Locator")]
pub struct Locator {
    inner: TerminatorLocator,
}

impl From<TerminatorLocator> for Locator {
    fn from(l: TerminatorLocator) -> Self {
        Locator { inner: l }
    }
}

#[napi]
impl Locator {
    /// (async) Get the first matching element.
    /// 
    /// @returns {Promise<Element>} The first matching element.
    #[napi]
    pub async fn first(&self) -> napi::Result<Element> {
        self.inner.first(None).await.map(Element::from).map_err(map_error)
    }

    /// (async) Get all matching elements.
    /// 
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @param {number} [depth] - Maximum depth to search.
    /// @returns {Promise<Array<Element>>} List of matching elements.
    #[napi]
    pub async fn all(&self, timeout_ms: Option<f64>, depth: Option<u32>) -> napi::Result<Vec<Element>> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        let depth = depth.map(|d| d as usize);
        self.inner.all(timeout, depth).await.map(|els| els.into_iter().map(Element::from).collect()).map_err(map_error)
    }

    /// (async) Wait for the first matching element.
    /// 
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<Element>} The first matching element.
    #[napi]
    pub async fn wait(&self, timeout_ms: Option<f64>) -> napi::Result<Element> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.wait(timeout).await.map(Element::from).map_err(map_error)
    }

    /// Set a default timeout for this locator.
    /// 
    /// @param {number} timeoutMs - Timeout in milliseconds.
    /// @returns {Locator} A new locator with the specified timeout.
    #[napi]
    pub fn timeout(&self, timeout_ms: f64) -> Locator {
        let loc = self.inner.clone().set_default_timeout(std::time::Duration::from_millis(timeout_ms as u64));
        Locator::from(loc)
    }

    /// Set the root element for this locator.
    /// 
    /// @param {Element} element - The root element.
    /// @returns {Locator} A new locator with the specified root element.
    #[napi]
    pub fn within(&self, element: &Element) -> Locator {
        let loc = self.inner.clone().within(element.inner.clone());
        Locator::from(loc)
    }

    /// Chain another selector.
    /// 
    /// @param {string} selector - The selector string.
    /// @returns {Locator} A new locator with the chained selector.
    #[napi]
    pub fn locator(&self, selector: String) -> napi::Result<Locator> {
        let sel: terminator::selector::Selector = selector.as_str().into();
        let loc = self.inner.clone().locator(sel);
        Ok(Locator::from(loc))
    }

    /// (async) Click on the first matching element.
    /// 
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<ClickResult>} Result of the click operation.
    #[napi]
    pub async fn click(&self, timeout_ms: Option<f64>) -> napi::Result<ClickResult> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.click(timeout).await.map(ClickResult::from).map_err(map_error)
    }

    /// (async) Type text into the first matching element.
    /// 
    /// @param {string} text - The text to type.
    /// @param {boolean} [useClipboard] - Whether to use clipboard for pasting.
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<void>}
    #[napi]
    pub async fn type_text(&self, text: String, use_clipboard: Option<bool>, timeout_ms: Option<f64>) -> napi::Result<()> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.type_text(&text, use_clipboard.unwrap_or(false), timeout).await.map_err(map_error)
    }

    /// (async) Press a key on the first matching element.
    /// 
    /// @param {string} key - The key to press.
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<void>}
    #[napi]
    pub async fn press_key(&self, key: String, timeout_ms: Option<f64>) -> napi::Result<()> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.press_key(&key, timeout).await.map_err(map_error)
    }

    /// (async) Get text from the first matching element.
    /// 
    /// @param {number} [maxDepth] - Maximum depth to search for text.
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<string>} The element's text content.
    #[napi]
    pub async fn text(&self, max_depth: Option<u32>, timeout_ms: Option<f64>) -> napi::Result<String> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.text(max_depth.unwrap_or(1) as usize, timeout).await.map_err(map_error)
    }

    /// (async) Get attributes from the first matching element.
    /// 
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<UIElementAttributes>} The element's attributes.
    #[napi]
    pub async fn attributes(&self, timeout_ms: Option<f64>) -> napi::Result<UIElementAttributes> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.attributes(timeout).await.map(|attrs| {
            UIElementAttributes {
                role: attrs.role,
                name: attrs.name,
                label: attrs.label,
                value: attrs.value,
                description: attrs.description,
                properties: attrs.properties.into_iter()
                    .map(|(k, v)| (k, v.map(|v| v.to_string())))
                    .collect(),
                is_keyboard_focusable: attrs.is_keyboard_focusable,
            }
        }).map_err(map_error)
    }

    /// (async) Get bounds from the first matching element.
    /// 
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<Bounds>} The element's bounds.
    #[napi]
    pub async fn bounds(&self, timeout_ms: Option<f64>) -> napi::Result<Bounds> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.bounds(timeout).await.map(Bounds::from).map_err(map_error)
    }

    /// (async) Check if the element is visible.
    /// 
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<boolean>} True if the element is visible.
    #[napi]
    pub async fn is_visible(&self, timeout_ms: Option<f64>) -> napi::Result<bool> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.is_visible(timeout).await.map_err(map_error)
    }

    /// (async) Wait for the element to be enabled.
    /// 
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<Element>} The enabled element.
    #[napi]
    pub async fn expect_enabled(&self, timeout_ms: Option<f64>) -> napi::Result<Element> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.expect_enabled(timeout).await.map(Element::from).map_err(map_error)
    }

    /// (async) Wait for the element to be visible.
    /// 
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<Element>} The visible element.
    #[napi]
    pub async fn expect_visible(&self, timeout_ms: Option<f64>) -> napi::Result<Element> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.expect_visible(timeout).await.map(Element::from).map_err(map_error)
    }

    /// (async) Wait for the element's text to equal the expected text.
    /// 
    /// @param {string} expectedText - The expected text.
    /// @param {number} [maxDepth] - Maximum depth to search for text.
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<Element>} The element with matching text.
    #[napi]
    pub async fn expect_text_equals(&self, expected_text: String, max_depth: Option<u32>, timeout_ms: Option<f64>) -> napi::Result<Element> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.expect_text_equals(&expected_text, max_depth.unwrap_or(1) as usize, timeout).await.map(Element::from).map_err(map_error)
    }

    /// (async) Double click on the first matching element.
    /// 
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<ClickResult>} Result of the click operation.
    #[napi]
    pub async fn double_click(&self, timeout_ms: Option<f64>) -> napi::Result<ClickResult> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.double_click(timeout).await.map(ClickResult::from).map_err(map_error)
    }

    /// (async) Right click on the first matching element.
    /// 
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<void>}
    #[napi]
    pub async fn right_click(&self, timeout_ms: Option<f64>) -> napi::Result<()> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.right_click(timeout).await.map_err(map_error)
    }

    /// (async) Hover over the first matching element.
    /// 
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<void>}
    #[napi]
    pub async fn hover(&self, timeout_ms: Option<f64>) -> napi::Result<()> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.hover(timeout).await.map_err(map_error)
    }

    /// (async) Explore the first matching element and its direct children.
    /// 
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<ExploreResponse>} Details about the element and its children.
    #[napi]
    pub async fn explore(&self, timeout_ms: Option<f64>) -> napi::Result<ExploreResponse> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.explore(timeout).await
            .map(|response| ExploreResponse {
                parent: Element::from(response.parent),
                children: response.children.into_iter().map(|child| ExploredElementDetail {
                    role: child.role,
                    name: child.name,
                    id: child.id,
                    bounds: child.bounds.map(Bounds::from),
                    value: child.value,
                    description: child.description,
                    text: child.text,
                    parent_id: child.parent_id,
                    children_ids: child.children_ids,
                    suggested_selector: child.suggested_selector,
                }).collect(),
            })
            .map_err(map_error)
    }

    /// (async) Get the id of the first matching element.
    ///
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<?string>} The element's id, or null if not present.
    #[napi]
    pub async fn id(&self, timeout_ms: Option<f64>) -> napi::Result<Option<String>> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.id(timeout).await.map_err(map_error)
    }

    /// (async) Get the role of the first matching element.
    ///
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<string>} The element's role.
    #[napi]
    pub async fn role(&self, timeout_ms: Option<f64>) -> napi::Result<String> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.role(timeout).await.map_err(map_error)
    }

    /// (async) Get the children of the first matching element.
    ///
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<Array<Element>>} The element's children.
    #[napi]
    pub async fn children(&self, timeout_ms: Option<f64>) -> napi::Result<Vec<Element>> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.children(timeout).await.map(|els| els.into_iter().map(Element::from).collect()).map_err(map_error)
    }

    /// (async) Get the parent of the first matching element.
    ///
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<?Element>} The element's parent, or null if not present.
    #[napi]
    pub async fn parent(&self, timeout_ms: Option<f64>) -> napi::Result<Option<Element>> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.parent(timeout).await.map(|opt| opt.map(Element::from)).map_err(map_error)
    }

    /// (async) Set value of the first matching element.
    ///
    /// @param {string} value - The value to set.
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<void>}
    #[napi]
    pub async fn set_value(&self, value: String, timeout_ms: Option<f64>) -> napi::Result<()> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.set_value(&value, timeout).await.map_err(map_error)
    }

    /// (async) Check if the first matching element is focused.
    ///
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<boolean>} True if the element is focused.
    #[napi]
    pub async fn is_focused(&self, timeout_ms: Option<f64>) -> napi::Result<bool> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.is_focused(timeout).await.map_err(map_error)
    }

    /// (async) Perform a named action on the first matching element.
    ///
    /// @param {string} action - The action name.
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<void>}
    #[napi]
    pub async fn perform_action(&self, action: String, timeout_ms: Option<f64>) -> napi::Result<()> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.perform_action(&action, timeout).await.map_err(map_error)
    }

    /// (async) Scroll the first matching element in a given direction.
    ///
    /// @param {string} direction - The scroll direction (e.g., "up", "down").
    /// @param {number} amount - The amount to scroll.
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<void>}
    #[napi]
    pub async fn scroll(&self, direction: String, amount: f64, timeout_ms: Option<f64>) -> napi::Result<()> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.scroll(&direction, amount, timeout).await.map_err(map_error)
    }

    /// (async) Activate the window containing the first matching element.
    ///
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<void>}
    #[napi]
    pub async fn activate_window(&self, timeout_ms: Option<f64>) -> napi::Result<()> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.activate_window(timeout).await.map_err(map_error)
    }

    /// (async) Get the name of the first matching element.
    ///
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<?string>} The element's name, or null if not present.
    #[napi]
    pub async fn name(&self, timeout_ms: Option<f64>) -> napi::Result<Option<String>> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.name(timeout).await.map_err(map_error)
    }

    /// (async) Check if the first matching element is keyboard focusable.
    ///
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<boolean>} True if the element is keyboard focusable.
    #[napi]
    pub async fn is_keyboard_focusable(&self, timeout_ms: Option<f64>) -> napi::Result<bool> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.is_keyboard_focusable(timeout).await.map_err(map_error)
    }

    /// (async) Drag mouse from start to end coordinates on the first matching element.
    ///
    /// @param {number} startX - Starting x coordinate.
    /// @param {number} startY - Starting y coordinate.
    /// @param {number} endX - Ending x coordinate.
    /// @param {number} endY - Ending y coordinate.
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<void>}
    #[napi]
    pub async fn mouse_drag(&self, start_x: f64, start_y: f64, end_x: f64, end_y: f64, timeout_ms: Option<f64>) -> napi::Result<()> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.mouse_drag(start_x, start_y, end_x, end_y, timeout).await.map_err(map_error)
    }

    /// (async) Press and hold mouse at (x, y) on the first matching element.
    ///
    /// @param {number} x - X coordinate.
    /// @param {number} y - Y coordinate.
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<void>}
    #[napi]
    pub async fn mouse_click_and_hold(&self, x: f64, y: f64, timeout_ms: Option<f64>) -> napi::Result<()> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.mouse_click_and_hold(x, y, timeout).await.map_err(map_error)
    }

    /// (async) Move mouse to (x, y) on the first matching element.
    ///
    /// @param {number} x - X coordinate.
    /// @param {number} y - Y coordinate.
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<void>}
    #[napi]
    pub async fn mouse_move(&self, x: f64, y: f64, timeout_ms: Option<f64>) -> napi::Result<()> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.mouse_move(x, y, timeout).await.map_err(map_error)
    }

    /// (async) Release mouse button on the first matching element.
    ///
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<void>}
    #[napi]
    pub async fn mouse_release(&self, timeout_ms: Option<f64>) -> napi::Result<()> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.mouse_release(timeout).await.map_err(map_error)
    }

    /// (async) Get the containing application element of the first matching element.
    ///
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<?Element>} The application element, or null if not present.
    #[napi]
    pub async fn application(&self, timeout_ms: Option<f64>) -> napi::Result<Option<Element>> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.application(timeout).await.map(|opt| opt.map(Element::from)).map_err(map_error)
    }

    /// (async) Get the containing window element of the first matching element.
    ///
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<?Element>} The window element, or null if not present.
    #[napi]
    pub async fn window(&self, timeout_ms: Option<f64>) -> napi::Result<Option<Element>> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.window(timeout).await.map(|opt| opt.map(Element::from)).map_err(map_error)
    }

    /// (async) Highlights the first matching element with a colored border.
    /// 
    /// @param {number} [color] - Optional BGR color code (32-bit integer). Default: 0x0000FF (red)
    /// @param {number} [durationMs] - Optional duration in milliseconds.
    /// @param {number} [timeoutMs] - Optional timeout in milliseconds.
    /// @returns {Promise<void>}
    #[napi]
    pub async fn highlight(&self, color: Option<u32>, duration_ms: Option<f64>, timeout_ms: Option<f64>) -> napi::Result<()> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        let duration = duration_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.highlight(color, duration, timeout).await.map_err(map_error)
    }
} 