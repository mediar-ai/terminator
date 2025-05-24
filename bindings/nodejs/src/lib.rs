use napi::Status;
use napi_derive::napi;
use std::sync::{Arc, Mutex, Once};
use terminator::{Desktop as TerminatorDesktop, element::UIElement as TerminatorUIElement, locator::Locator as TerminatorLocator, errors::AutomationError};

/// Main entry point for desktop automation
#[napi(js_name = "Desktop")]
pub struct Desktop {
    inner: TerminatorDesktop,
}

#[napi]
impl Desktop {
    /// Create a new Desktop automation instance with default settings
    #[napi(constructor)]
    pub fn new() -> napi::Result<Self> {
        Self::with_options(false, false)
    }

    /// Create a new Desktop automation instance with background apps enabled
    #[napi(factory)]
    pub fn with_background_apps() -> napi::Result<Self> {
        Self::with_options(true, false)
    }

    /// Create a new Desktop automation instance with app activation enabled
    #[napi(factory)]
    pub fn with_app_activation() -> napi::Result<Self> {
        Self::with_options(false, true)
    }

    /// Create a new Desktop automation instance with both background apps and app activation enabled
    #[napi(factory)]
    pub fn with_all_features() -> napi::Result<Self> {
        Self::with_options(true, true)
    }

    /// Internal helper to create a Desktop instance with specific options
    fn with_options(use_background_apps: bool, activate_app: bool) -> napi::Result<Self> {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let _ = tracing_subscriber::fmt()
                .with_env_filter("info")
                .try_init();
        });
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| napi::Error::from_reason(format!("Tokio runtime error: {e}")))?;
        let desktop = rt.block_on(TerminatorDesktop::new(use_background_apps, activate_app))
            .map_err(|e| napi::Error::from_reason(format!("{e}")))?;
        Ok(Desktop { inner: desktop })
    }

    /// Get the root UI element
    #[napi]
    pub fn root(&self) -> Element {
        let root = self.inner.root();
        Element::from(root)
    }

    /// List all running applications
    #[napi]
    pub fn applications(&self) -> napi::Result<Vec<Element>> {
        self.inner.applications()
            .map(|apps| apps.into_iter().map(Element::from).collect())
            .map_err(|e| napi::Error::from_reason(format!("{e}")))
    }

    /// Get a running application by name
    #[napi]
    pub fn application(&self, name: String) -> napi::Result<Element> {
        self.inner.application(&name)
            .map(Element::from)
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
    pub async fn capture_screen(&self) -> napi::Result<Screenshot> {
        self.inner.capture_screen().await
            .map(|r| Screenshot {
                width: r.width,
                height: r.height,
                image_data: r.image_data,
            })
            .map_err(|e| napi::Error::from_reason(format!("{e}")))
    }

    /// Run a shell command
    #[napi]
    pub async fn run_command(&self, options: RunCommandOptions) -> napi::Result<CommandOutput> {
        self.inner.run_command(options.windows_command.as_deref(), options.unix_command.as_deref()).await
            .map(|r| CommandOutput {
                exit_status: r.exit_status,
                stdout: r.stdout,
                stderr: r.stderr,
            })
            .map_err(|e| napi::Error::from_reason(format!("{e}")))
    }

    /// Capture a screenshot of a specific monitor
    #[napi]
    pub async fn capture_monitor_by_name(&self, name: String) -> napi::Result<Screenshot> {
        self.inner.capture_monitor_by_name(&name).await
            .map(|r| Screenshot {
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
    pub async fn ocr_screenshot(&self, screenshot: Screenshot) -> napi::Result<String> {
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
    pub async fn find_window_by_criteria(&self, title_contains: Option<String>, timeout_ms: Option<f64>) -> napi::Result<Element> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner.find_window_by_criteria(title_contains.as_deref(), timeout).await
            .map(Element::from)
            .map_err(|e| napi::Error::from_reason(format!("{e}")))
    }

    /// Get the currently focused browser window
    #[napi]
    pub async fn get_current_browser_window(&self) -> napi::Result<Element> {
        self.inner.get_current_browser_window().await
            .map(Element::from)
            .map_err(|e| napi::Error::from_reason(format!("{e}")))
    }

    /// Create a locator for advanced queries
    #[napi]
    pub fn locator(&self, selector: String) -> napi::Result<Locator> {
        let sel: terminator::selector::Selector = selector.as_str().into();
        let loc = self.inner.locator(sel);
        Ok(Locator::from(loc))
    }

    /// Get the currently focused window
    #[napi]
    pub async fn get_current_window(&self) -> napi::Result<Element> {
        self.inner.get_current_window().await
            .map(Element::from)
            .map_err(|e| napi::Error::from_reason(format!("{e}")))
    }

    /// Get the currently focused application
    #[napi]
    pub async fn get_current_application(&self) -> napi::Result<Element> {
        self.inner.get_current_application().await
            .map(Element::from)
            .map_err(|e| napi::Error::from_reason(format!("{e}")))
    }
}

/// A UI element in the accessibility tree
#[napi(js_name = "Element")]
pub struct Element {
    inner: Arc<Mutex<TerminatorUIElement>>,
}

impl From<TerminatorUIElement> for Element {
    fn from(e: TerminatorUIElement) -> Self {
        Element {
            inner: Arc::new(Mutex::new(e)),
        }
    }
}

#[napi]
impl Element {
    /// Create a new Element from a selector string
    #[napi(factory)]
    pub async fn from_selector(desktop: &Desktop, selector: String) -> napi::Result<Self> {
        let sel: terminator::selector::Selector = selector.as_str().into();
        let loc = desktop.inner.locator(sel);
        loc.first(None).await
            .map(Element::from)
            .map_err(map_error)
    }

    /// Create a new Element from a selector string with timeout
    #[napi(factory)]
    pub async fn from_selector_with_timeout(desktop: &Desktop, selector: String, timeout_ms: f64) -> napi::Result<Self> {
        let sel: terminator::selector::Selector = selector.as_str().into();
        let loc = desktop.inner.locator(sel);
        use std::time::Duration;
        let timeout = Duration::from_millis(timeout_ms as u64);
        loc.first(Some(timeout)).await
            .map(Element::from)
            .map_err(map_error)
    }

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
    pub fn children(&self) -> napi::Result<Vec<Element>> {
        self.inner.lock().unwrap().children()
            .map(|kids| kids.into_iter().map(Element::from).collect())
            .map_err(map_error)
    }

    /// Get the parent element
    #[napi]
    pub fn parent(&self) -> napi::Result<Option<Element>> {
        self.inner.lock().unwrap().parent()
            .map(|opt| opt.map(Element::from))
            .map_err(map_error)
    }

    /// The bounding rectangle
    #[napi(getter)]
    pub fn bounds(&self) -> napi::Result<Bounds> {
        self.inner.lock().unwrap().bounds()
            .map(Bounds::from)
            .map_err(map_error)
    }

    /// Click the element (returns click result)
    #[napi]
    pub fn click(&self) -> napi::Result<ClickResult> {
        self.inner.lock().unwrap().click()
            .map(ClickResult::from)
            .map_err(map_error)
    }

    /// Is the element visible?
    #[napi(getter)]
    pub fn is_visible(&self) -> napi::Result<bool> {
        self.inner.lock().unwrap().is_visible().map_err(map_error)
    }

    /// Is the element enabled?
    #[napi(getter)]
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
    #[napi(getter)]
    pub fn is_focused(&self) -> napi::Result<bool> {
        self.inner.lock().unwrap().is_focused().map_err(map_error)
    }

    /// Is the element keyboard focusable?
    #[napi(getter)]
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
    pub fn locator(&self, selector: String) -> napi::Result<Locator> {
        let sel: terminator::selector::Selector = selector.as_str().into();
        let loc = self.inner.lock().unwrap().locator(sel).map_err(map_error)?;
        Ok(Locator::from(loc))
    }

    /// Get the containing application element
    #[napi]
    pub fn application(&self) -> napi::Result<Option<Element>> {
        self.inner.lock().unwrap().application()
            .map(|opt| opt.map(Element::from))
            .map_err(map_error)
    }

    /// Get the containing window element (e.g., tab, dialog)
    #[napi]
    pub fn window(&self) -> napi::Result<Option<Element>> {
        self.inner.lock().unwrap().window()
            .map(|opt| opt.map(Element::from))
            .map_err(map_error)
    }
}

/// Locator for advanced queries (chainable)
#[napi(js_name = "Locator")]
pub struct Locator {
    inner: Arc<Mutex<TerminatorLocator>>,
}

impl From<TerminatorLocator> for Locator {
    fn from(l: TerminatorLocator) -> Self {
        Locator {
            inner: Arc::new(Mutex::new(l)),
        }
    }
}

#[napi]
impl Locator {
    /// Create a new Locator with a selector
    #[napi(factory)]
    pub fn with_selector(desktop: &Desktop, selector: String) -> napi::Result<Self> {
        let sel: terminator::selector::Selector = selector.as_str().into();
        let loc = desktop.inner.locator(sel);
        Ok(Locator::from(loc))
    }

    /// Create a new Locator with a selector and timeout
    #[napi(factory)]
    pub fn with_selector_and_timeout(desktop: &Desktop, selector: String, timeout_ms: f64) -> napi::Result<Self> {
        let sel: terminator::selector::Selector = selector.as_str().into();
        let loc = desktop.inner.locator(sel);
        let loc = loc.set_default_timeout(std::time::Duration::from_millis(timeout_ms as u64));
        Ok(Locator::from(loc))
    }

    /// Get the first matching element (async)
    #[napi]
    pub async fn first(&self) -> napi::Result<Element> {
        let loc = self.inner.lock().unwrap().clone();
        loc.first(None).await.map(Element::from).map_err(map_error)
    }

    /// Get all matching elements (async)
    #[napi]
    pub async fn all(&self, timeout_ms: Option<f64>, depth: Option<u32>) -> napi::Result<Vec<Element>> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        let depth = depth.map(|d| d as usize);
        let loc = self.inner.lock().unwrap().clone();
        loc.all(timeout, depth).await.map(|els| els.into_iter().map(Element::from).collect()).map_err(map_error)
    }

    /// Wait for the first matching element (async)
    #[napi]
    pub async fn wait(&self, timeout_ms: Option<f64>) -> napi::Result<Element> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        let loc = self.inner.lock().unwrap().clone();
        loc.wait(timeout).await.map(Element::from).map_err(map_error)
    }

    /// Set a default timeout for this locator (returns a new locator)
    #[napi]
    pub fn timeout(&self, timeout_ms: f64) -> Locator {
        let loc = self.inner.lock().unwrap().clone().set_default_timeout(std::time::Duration::from_millis(timeout_ms as u64));
        Locator::from(loc)
    }

    /// Chain another selector
    #[napi]
    pub fn locator(&self, selector: String) -> napi::Result<Locator> {
        let sel: terminator::selector::Selector = selector.as_str().into();
        let loc = self.inner.lock().unwrap().clone().locator(sel);
        Ok(Locator::from(loc))
    }
}

// --- Result types ---

#[napi(object, js_name = "Bounds")]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[napi(object, js_name = "Coordinates")]
pub struct Coordinates {
    pub x: f64,
    pub y: f64,
}

#[napi(object, js_name = "ClickResult")]
pub struct ClickResult {
    pub method: String,
    pub coordinates: Option<Coordinates>,
    pub details: String,
}

#[napi(object, js_name = "CommandOutput")]
pub struct CommandOutput {
    pub exit_status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[napi(object, js_name = "Screenshot")]
pub struct Screenshot {
    pub width: u32,
    pub height: u32,
    pub image_data: Vec<u8>,
}

#[napi(object)]
pub struct RunCommandOptions {
    pub windows_command: Option<String>,
    pub unix_command: Option<String>,
}

impl From<(f64, f64, f64, f64)> for Bounds {
    fn from(t: (f64, f64, f64, f64)) -> Self {
        Bounds { x: t.0, y: t.1, width: t.2, height: t.3 }
    }
}

impl From<(f64, f64)> for Coordinates {
    fn from(t: (f64, f64)) -> Self {
        Coordinates { x: t.0, y: t.1 }
    }
}

impl From<terminator::ClickResult> for ClickResult {
    fn from(r: terminator::ClickResult) -> Self {
        ClickResult {
            method: r.method,
            coordinates: r.coordinates.map(Coordinates::from),
            details: r.details,
        }
    }
}

// --- Error types ---

/// Thrown when an element is not found.
#[napi(js_name = "ElementNotFoundError")]
pub struct ElementNotFoundError(pub String);

/// Thrown when an operation times out.
#[napi(js_name = "TimeoutError")]
pub struct TimeoutError(pub String);

/// Thrown when permission is denied.
#[napi(js_name = "PermissionDeniedError")]
pub struct PermissionDeniedError(pub String);

/// Thrown for platform-specific errors.
#[napi(js_name = "PlatformError")]
pub struct PlatformError(pub String);

/// Thrown for unsupported operations.
#[napi(js_name = "UnsupportedOperationError")]
pub struct UnsupportedOperationError(pub String);

/// Thrown for unsupported platforms.
#[napi(js_name = "UnsupportedPlatformError")]
pub struct UnsupportedPlatformError(pub String);

/// Thrown for invalid arguments.
#[napi(js_name = "InvalidArgumentError")]
pub struct InvalidArgumentError(pub String);

/// Thrown for internal errors.
#[napi(js_name = "InternalError")]
pub struct InternalError(pub String);

// Implement Display and Error for all error classes
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
impl_js_error!(ElementNotFoundError);
impl_js_error!(TimeoutError);
impl_js_error!(PermissionDeniedError);
impl_js_error!(PlatformError);
impl_js_error!(UnsupportedOperationError);
impl_js_error!(UnsupportedPlatformError);
impl_js_error!(InvalidArgumentError);
impl_js_error!(InternalError);

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
