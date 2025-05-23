use napi::Status;
use napi_derive::napi;
use std::sync::{Arc, Mutex, Once};
use terminator::{Desktop, element::UIElement, locator::Locator, errors::AutomationError};

/// Main entry point for desktop automation
#[napi]
pub struct NodeDesktop {
    inner: Desktop,
}

#[napi]
impl NodeDesktop {
    /// Create a new Desktop automation instance
    #[napi(constructor)]
    pub fn new() -> napi::Result<Self> {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let _ = tracing_subscriber::fmt()
                .with_env_filter("info")
                .try_init();
        });
        // For now, use default args: use_background_apps=false, activate_app=false
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| napi::Error::from_reason(format!("Tokio runtime error: {e}")))?;
        let desktop = rt.block_on(Desktop::new(false, false))
            .map_err(|e| napi::Error::from_reason(format!("{e}")))?;
        Ok(NodeDesktop { inner: desktop })
    }

    /// Get the root UI element
    #[napi]
    pub fn root(&self) -> NodeUIElement {
        let root = self.inner.root();
        NodeUIElement::from(root)
    }

    /// List all running applications
    #[napi]
    pub fn applications(&self) -> napi::Result<Vec<NodeUIElement>> {
        self.inner.applications()
            .map(|apps| apps.into_iter().map(NodeUIElement::from).collect())
            .map_err(|e| napi::Error::from_reason(format!("{e}")))
    }

    /// Get a running application by name
    #[napi]
    pub fn application(&self, name: String) -> napi::Result<NodeUIElement> {
        self.inner.application(&name)
            .map(NodeUIElement::from)
            .map_err(|e| napi::Error::from_reason(format!("{e}")))
    }

    /// Open an application by name
    #[napi]
    pub fn open_application(&self, name: String) -> napi::Result<()> {
        self.inner.open_application(&name)
            .map_err(|e| napi::Error::from_reason(format!("{e}")))
    }

    /// Activate an application by name
    #[napi]
    pub fn activate_application(&self, name: String) -> napi::Result<()> {
        self.inner.activate_application(&name)
            .map_err(|e| napi::Error::from_reason(format!("{e}")))
    }

    /// Capture a screenshot of the primary monitor
    #[napi]
    pub async fn capture_screen(&self) -> napi::Result<NodeScreenshotResult> {
        self.inner.capture_screen().await
            .map(|r| NodeScreenshotResult {
                width: r.width,
                height: r.height,
                image_data: r.image_data,
            })
            .map_err(|e| napi::Error::from_reason(format!("{e}")))
    }

    /// Run a shell command
    #[napi]
    pub async fn run_command(&self, windows_command: Option<String>, unix_command: Option<String>) -> napi::Result<NodeCommandOutput> {
        self.inner.run_command(windows_command.as_deref(), unix_command.as_deref()).await
            .map(|r| NodeCommandOutput {
                exit_status: r.exit_status,
                stdout: r.stdout,
                stderr: r.stderr,
            })
            .map_err(|e| napi::Error::from_reason(format!("{e}")))
    }

    /// Capture a screenshot of a specific monitor
    #[napi]
    pub async fn capture_monitor_by_name(&self, name: String) -> napi::Result<NodeScreenshotResult> {
        self.inner.capture_monitor_by_name(&name).await
            .map(|r| NodeScreenshotResult {
                width: r.width,
                height: r.height,
                image_data: r.image_data,
            })
            .map_err(|e| napi::Error::from_reason(format!("{e}")))
    }

    /// Perform OCR on an image file
    #[napi]
    pub async fn ocr_image_path(&self, image_path: String) -> napi::Result<String> {
        self.inner.ocr_image_path(&image_path).await
            .map_err(|e| napi::Error::from_reason(format!("{e}")))
    }

    /// Perform OCR on a screenshot
    #[napi]
    pub async fn ocr_screenshot(&self, screenshot: NodeScreenshotResult) -> napi::Result<String> {
        let rust_screenshot = terminator::ScreenshotResult {
            image_data: screenshot.image_data,
            width: screenshot.width,
            height: screenshot.height,
        };
        self.inner.ocr_screenshot(&rust_screenshot).await
            .map_err(|e| napi::Error::from_reason(format!("{e}")))
    }

    /// Find a window by criteria
    #[napi]
    pub async fn find_window_by_criteria(&self, title_contains: Option<String>, timeout_ms: Option<f64>) -> napi::Result<NodeUIElement> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.find_window_by_criteria(title_contains.as_deref(), timeout).await
            .map(NodeUIElement::from)
            .map_err(|e| napi::Error::from_reason(format!("{e}")))
    }

    /// Get the currently focused browser window
    #[napi]
    pub async fn get_current_browser_window(&self) -> napi::Result<NodeUIElement> {
        self.inner.get_current_browser_window().await
            .map(NodeUIElement::from)
            .map_err(|e| napi::Error::from_reason(format!("{e}")))
    }

    /// Create a locator for advanced queries
    #[napi]
    pub fn locator(&self, selector: String) -> napi::Result<NodeLocator> {
        let sel: terminator::selector::Selector = selector.as_str().into();
        let loc = self.inner.locator(sel);
        Ok(NodeLocator::from(loc))
    }
}

/// A UI element in the accessibility tree
#[napi]
pub struct NodeUIElement {
    inner: Arc<Mutex<UIElement>>,
}

impl From<UIElement> for NodeUIElement {
    fn from(e: UIElement) -> Self {
        NodeUIElement {
            inner: Arc::new(Mutex::new(e)),
        }
    }
}

#[napi]
impl NodeUIElement {
    /// The accessibility role
    #[napi(getter)]
    pub fn role(&self) -> String {
        self.inner.lock().unwrap().role()
    }

    /// The accessibility name
    #[napi(getter)]
    pub fn name(&self) -> Option<String> {
        self.inner.lock().unwrap().name()
    }

    /// Get children of this element
    #[napi]
    pub fn children(&self) -> napi::Result<Vec<NodeUIElement>> {
        self.inner.lock().unwrap().children()
            .map(|kids| kids.into_iter().map(NodeUIElement::from).collect())
            .map_err(map_error)
    }

    /// Get the parent element
    #[napi]
    pub fn parent(&self) -> napi::Result<Option<NodeUIElement>> {
        self.inner.lock().unwrap().parent()
            .map(|opt| opt.map(NodeUIElement::from))
            .map_err(map_error)
    }

    /// The bounding rectangle
    #[napi(getter)]
    pub fn bounds(&self) -> napi::Result<NodeBounds> {
        self.inner.lock().unwrap().bounds()
            .map(NodeBounds::from)
            .map_err(map_error)
    }

    /// Click the element (returns click result)
    #[napi]
    pub fn click(&self) -> napi::Result<NodeClickResult> {
        self.inner.lock().unwrap().click()
            .map(NodeClickResult::from)
            .map_err(map_error)
    }

    /// Is the element visible?
    #[napi]
    pub fn is_visible(&self) -> napi::Result<bool> {
        self.inner.lock().unwrap().is_visible().map_err(map_error)
    }

    /// Is the element enabled?
    #[napi]
    pub fn is_enabled(&self) -> napi::Result<bool> {
        self.inner.lock().unwrap().is_enabled().map_err(map_error)
    }

    /// Focus the element
    #[napi]
    pub fn focus(&self) -> napi::Result<()> {
        self.inner.lock().unwrap().focus().map_err(map_error)
    }

    /// Get the text content
    #[napi]
    pub fn text(&self, max_depth: Option<u32>) -> napi::Result<String> {
        self.inner.lock().unwrap().text(max_depth.unwrap_or(1) as usize).map_err(map_error)
    }

    /// Type text into the element
    #[napi]
    pub fn type_text(&self, text: String, use_clipboard: Option<bool>) -> napi::Result<()> {
        self.inner.lock().unwrap().type_text(&text, use_clipboard.unwrap_or(false)).map_err(map_error)
    }

    /// Press a key on the element
    #[napi]
    pub fn press_key(&self, key: String) -> napi::Result<()> {
        self.inner.lock().unwrap().press_key(&key).map_err(map_error)
    }

    /// Set the value of the element
    #[napi]
    pub fn set_value(&self, value: String) -> napi::Result<()> {
        self.inner.lock().unwrap().set_value(&value).map_err(map_error)
    }

    /// Perform a custom action
    #[napi]
    pub fn perform_action(&self, action: String) -> napi::Result<()> {
        self.inner.lock().unwrap().perform_action(&action).map_err(map_error)
    }

    /// Scroll the element
    #[napi]
    pub fn scroll(&self, direction: String, amount: f64) -> napi::Result<()> {
        self.inner.lock().unwrap().scroll(&direction, amount).map_err(map_error)
    }

    /// Activate the window containing this element
    #[napi]
    pub fn activate_window(&self) -> napi::Result<()> {
        self.inner.lock().unwrap().activate_window().map_err(map_error)
    }

    /// Is the element focused?
    #[napi]
    pub fn is_focused(&self) -> napi::Result<bool> {
        self.inner.lock().unwrap().is_focused().map_err(map_error)
    }

    /// Is the element keyboard focusable?
    #[napi]
    pub fn is_keyboard_focusable(&self) -> napi::Result<bool> {
        self.inner.lock().unwrap().is_keyboard_focusable().map_err(map_error)
    }

    /// Mouse drag from/to coordinates
    #[napi]
    pub fn mouse_drag(&self, start_x: f64, start_y: f64, end_x: f64, end_y: f64) -> napi::Result<()> {
        self.inner.lock().unwrap().mouse_drag(start_x, start_y, end_x, end_y).map_err(map_error)
    }

    /// Mouse click and hold
    #[napi]
    pub fn mouse_click_and_hold(&self, x: f64, y: f64) -> napi::Result<()> {
        self.inner.lock().unwrap().mouse_click_and_hold(x, y).map_err(map_error)
    }

    /// Mouse move
    #[napi]
    pub fn mouse_move(&self, x: f64, y: f64) -> napi::Result<()> {
        self.inner.lock().unwrap().mouse_move(x, y).map_err(map_error)
    }

    /// Mouse release
    #[napi]
    pub fn mouse_release(&self) -> napi::Result<()> {
        self.inner.lock().unwrap().mouse_release().map_err(map_error)
    }

    /// Create a locator from this element
    #[napi]
    pub fn locator(&self, selector: String) -> napi::Result<NodeLocator> {
        let sel: terminator::selector::Selector = selector.as_str().into();
        let loc = self.inner.lock().unwrap().locator(sel).map_err(map_error)?;
        Ok(NodeLocator::from(loc))
    }
}

/// Locator for advanced queries (chainable)
#[napi]
pub struct NodeLocator {
    inner: Arc<Mutex<Locator>>,
}

impl From<Locator> for NodeLocator {
    fn from(l: Locator) -> Self {
        NodeLocator {
            inner: Arc::new(Mutex::new(l)),
        }
    }
}

#[napi]
impl NodeLocator {
    /// Get the first matching element (async)
    #[napi]
    pub async fn first(&self) -> napi::Result<NodeUIElement> {
        let loc = self.inner.lock().unwrap().clone();
        loc.first(None).await.map(NodeUIElement::from).map_err(map_error)
    }

    /// Get all matching elements (async)
    #[napi]
    pub async fn all(&self, timeout_ms: Option<f64>, depth: Option<u32>) -> napi::Result<Vec<NodeUIElement>> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        let depth = depth.map(|d| d as usize);
        let loc = self.inner.lock().unwrap().clone();
        loc.all(timeout, depth).await.map(|els| els.into_iter().map(NodeUIElement::from).collect()).map_err(map_error)
    }

    /// Wait for the first matching element (async)
    #[napi]
    pub async fn wait(&self, timeout_ms: Option<f64>) -> napi::Result<NodeUIElement> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        let loc = self.inner.lock().unwrap().clone();
        loc.wait(timeout).await.map(NodeUIElement::from).map_err(map_error)
    }

    /// Set a default timeout for this locator (returns a new locator)
    #[napi]
    pub fn timeout(&self, timeout_ms: f64) -> NodeLocator {
        let loc = self.inner.lock().unwrap().clone().set_default_timeout(std::time::Duration::from_millis(timeout_ms as u64));
        NodeLocator::from(loc)
    }

    /// Chain another selector
    #[napi]
    pub fn locator(&self, selector: String) -> napi::Result<NodeLocator> {
        let sel: terminator::selector::Selector = selector.as_str().into();
        let loc = self.inner.lock().unwrap().clone().locator(sel);
        Ok(NodeLocator::from(loc))
    }
}

// --- Result types ---

#[napi(object)]
pub struct NodeBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[napi(object)]
pub struct NodeCoordinates {
    pub x: f64,
    pub y: f64,
}

#[napi(object)]
pub struct NodeClickResult {
    pub method: String,
    pub coordinates: Option<NodeCoordinates>,
    pub details: String,
}

#[napi(object)]
pub struct NodeCommandOutput {
    pub exit_status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[napi(object)]
pub struct NodeScreenshotResult {
    pub width: u32,
    pub height: u32,
    pub image_data: Vec<u8>,
}

impl From<(f64, f64, f64, f64)> for NodeBounds {
    fn from(t: (f64, f64, f64, f64)) -> Self {
        NodeBounds { x: t.0, y: t.1, width: t.2, height: t.3 }
    }
}

impl From<(f64, f64)> for NodeCoordinates {
    fn from(t: (f64, f64)) -> Self {
        NodeCoordinates { x: t.0, y: t.1 }
    }
}

impl From<terminator::ClickResult> for NodeClickResult {
    fn from(r: terminator::ClickResult) -> Self {
        NodeClickResult {
            method: r.method,
            coordinates: r.coordinates.map(NodeCoordinates::from),
            details: r.details,
        }
    }
}

// --- Custom JS error classes for AutomationError variants ---

/// Thrown when an element is not found.
#[napi(js_name = "ElementNotFoundError")]
pub struct JsElementNotFoundError(pub String);

/// Thrown when an operation times out.
#[napi(js_name = "TimeoutError")]
pub struct JsTimeoutError(pub String);

/// Thrown when permission is denied.
#[napi(js_name = "PermissionDeniedError")]
pub struct JsPermissionDeniedError(pub String);

/// Thrown for platform-specific errors.
#[napi(js_name = "PlatformError")]
pub struct JsPlatformError(pub String);

/// Thrown for unsupported operations.
#[napi(js_name = "UnsupportedOperationError")]
pub struct JsUnsupportedOperationError(pub String);

/// Thrown for unsupported platforms.
#[napi(js_name = "UnsupportedPlatformError")]
pub struct JsUnsupportedPlatformError(pub String);

/// Thrown for invalid arguments.
#[napi(js_name = "InvalidArgumentError")]
pub struct JsInvalidArgumentError(pub String);

/// Thrown for internal errors.
#[napi(js_name = "InternalError")]
pub struct JsInternalError(pub String);

// Implement Display and Error for all error classes, and make them extend JS Error
macro_rules! impl_js_error {
    ($name:ident) => {
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}: {}", stringify!($name), self.0)
            }
        }
        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}: {}", stringify!($name), self.0)
            }
        }
        impl std::error::Error for $name {}
    };
}
impl_js_error!(JsElementNotFoundError);
impl_js_error!(JsTimeoutError);
impl_js_error!(JsPermissionDeniedError);
impl_js_error!(JsPlatformError);
impl_js_error!(JsUnsupportedOperationError);
impl_js_error!(JsUnsupportedPlatformError);
impl_js_error!(JsInvalidArgumentError);
impl_js_error!(JsInternalError);

fn map_error(e: AutomationError) -> napi::Error {
    use AutomationError::*;
    match e {
        ElementNotFound(msg) => napi::Error::new(Status::GenericFailure, format!("ElementNotFoundError: {msg}")),
        Timeout(msg) => napi::Error::new(Status::GenericFailure, format!("TimeoutError: {msg}")),
        PermissionDenied(msg) => napi::Error::new(Status::GenericFailure, format!("PermissionDeniedError: {msg}")),
        PlatformError(msg) => napi::Error::new(Status::GenericFailure, format!("PlatformError: {msg}")),
        UnsupportedOperation(msg) => napi::Error::new(Status::GenericFailure, format!("UnsupportedOperationError: {msg}")),
        UnsupportedPlatform(msg) => napi::Error::new(Status::GenericFailure, format!("UnsupportedPlatformError: {msg}")),
        InvalidArgument(msg) => napi::Error::new(Status::GenericFailure, format!("InvalidArgumentError: {msg}")),
        Internal(msg) => napi::Error::new(Status::GenericFailure, format!("InternalError: {msg}")),
    }
}
