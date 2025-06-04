use pyo3::prelude::*;
use pyo3_stub_gen::derive::*;
use pyo3_async_runtimes::tokio as pyo3_tokio;
use pyo3_async_runtimes::TaskLocals;
use std::sync::Once;
use ::terminator_core::Desktop as TerminatorDesktop;
use crate::exceptions::automation_error_to_pyerr;
use crate::types::{ScreenshotResult, CommandOutput};
use crate::element::UIElement;
use crate::locator::Locator;

/// Main entry point for desktop automation.
#[gen_stub_pyclass]
#[pyclass(name = "Desktop")]
pub struct Desktop {
    inner: TerminatorDesktop,
}

#[gen_stub_pymethods]
#[pymethods]
impl Desktop {
    #[new]
    #[pyo3(signature = (use_background_apps=None, activate_app=None, log_level=None))]
    #[pyo3(text_signature = "(use_background_apps=False, activate_app=False, log_level=None)")]
    /// Create a new Desktop automation instance with configurable options.
    ///
    /// Args:
    ///     use_background_apps (bool, optional): Enable background apps support. Defaults to False.
    ///     activate_app (bool, optional): Enable app activation support. Defaults to False.
    ///     log_level (str, optional): Logging level (e.g., 'info', 'debug', 'warn', 'error'). Defaults to 'info'.
    ///
    /// Returns:
    ///     Desktop: A new Desktop automation instance.
    pub fn new(
        use_background_apps: Option<bool>,
        activate_app: Option<bool>,
        log_level: Option<String>
    ) -> PyResult<Self> {
        static INIT: Once = Once::new();
        let log_level = log_level.unwrap_or_else(|| "info".to_string());
        INIT.call_once(|| {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(log_level)
                .try_init();
        });
        let use_background_apps = use_background_apps.unwrap_or(false);
        let activate_app = activate_app.unwrap_or(false);
        let desktop = TerminatorDesktop::new(use_background_apps, activate_app)
            .map_err(|e| automation_error_to_pyerr(e))?;
        Ok(Desktop { inner: desktop })
    }

    #[pyo3(text_signature = "($self)")]
    /// Get the root UI element of the desktop.
    /// 
    /// Returns:
    ///     UIElement: The root UI element.
    pub fn root(&self) -> PyResult<UIElement> {
        let root = self.inner.root();
        Ok(UIElement { inner: root })
    }

    #[pyo3(text_signature = "($self)")]
    /// Get a list of all running applications.
    /// 
    /// Returns:
    ///     List[UIElement]: List of application UI elements.
    pub fn applications(&self) -> PyResult<Vec<UIElement>> {
        self.inner.applications()
            .map(|apps| apps.into_iter().map(|e| UIElement { inner: e }).collect())
            .map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(text_signature = "($self, name)")]
    /// Get a running application by name.
    /// 
    /// Args:
    ///     name (str): The name of the application to find.
    /// 
    /// Returns:
    ///     UIElement: The application UI element.
    pub fn application(&self, name: &str) -> PyResult<UIElement> {
        self.inner.application(name)
            .map(|e| UIElement { inner: e })
            .map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "open_application", text_signature = "($self, name)")]
    /// Open an application by name.
    /// 
    /// Args:
    ///     name (str): The name of the application to open.
    pub fn open_application(&self, name: &str) -> PyResult<UIElement> {
        self.inner.open_application(name)
            .map(|e| UIElement { inner: e })
            .map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "activate_application", text_signature = "($self, name)")]
    /// Activate an application by name.
    /// 
    /// Args:
    ///     name (str): The name of the application to activate.
    pub fn activate_application(&self, name: &str) -> PyResult<()> {
        self.inner.activate_application(name)
            .map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "locator", text_signature = "($self, selector)")]
    /// Create a locator for finding UI elements.
    /// 
    /// Args:
    ///     selector (str): The selector string to find elements.
    /// 
    /// Returns:
    ///     Locator: A locator for finding elements.
    pub fn locator(&self, selector: &str) -> PyResult<Locator> {
        let locator = self.inner.locator(selector);
        Ok(Locator { inner: locator })
    }

    #[pyo3(name = "capture_screen", text_signature = "($self)")]
    /// (async) Capture a screenshot of the primary monitor.
    /// 
    /// Returns:
    ///     ScreenshotResult: The screenshot data.
    pub fn capture_screen<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let desktop = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let result = desktop.capture_screen().await.map_err(|e| automation_error_to_pyerr(e))?;
            let py_result = ScreenshotResult::from(result);
            Ok(py_result)
        })
    }

    #[pyo3(name = "run_command", signature = (windows_command=None, unix_command=None))]
    #[pyo3(text_signature = "($self, windows_command, unix_command)")]
    /// (async) Run a shell command.
    /// 
    /// Args:
    ///     windows_command (Optional[str]): Command to run on Windows.
    ///     unix_command (Optional[str]): Command to run on Unix.
    /// 
    /// Returns:
    ///     CommandOutput: The command output.
    pub fn run_command<'py>(&self, py: Python<'py>, windows_command: Option<String>, unix_command: Option<String>) -> PyResult<Bound<'py, PyAny>> {
        let desktop = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let result = desktop.run_command(windows_command.as_deref(), unix_command.as_deref()).await.map_err(|e| automation_error_to_pyerr(e))?;
            let py_result = CommandOutput::from(result);
            Ok(py_result)
        })
    }

    #[pyo3(name = "get_active_monitor_name", text_signature = "($self)")]
    /// (async) Get the name of the currently active monitor.
    /// 
    /// Returns:
    ///     str: The name of the active monitor.
    pub fn get_active_monitor_name<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let desktop = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let result = desktop.get_active_monitor_name().await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(result)
        })
    }

    #[pyo3(name = "capture_monitor_by_name", text_signature = "($self, name)")]
    /// (async) Capture a screenshot of a specific monitor.
    /// 
    /// Args:
    ///     name (str): The name of the monitor to capture.
    /// 
    /// Returns:
    ///     ScreenshotResult: The screenshot data.
    pub fn capture_monitor_by_name<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        let desktop = self.inner.clone();
        let name = name.to_string();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let result = desktop.capture_monitor_by_name(&name).await.map_err(|e| automation_error_to_pyerr(e))?;
            let py_result = ScreenshotResult::from(result);
            Ok(py_result)
        })
    }

    #[pyo3(name = "ocr_image_path", text_signature = "($self, image_path)")]
    /// (async) Perform OCR on an image file.
    /// 
    /// Args:
    ///     image_path (str): Path to the image file.
    /// 
    /// Returns:
    ///     str: The extracted text.
    pub fn ocr_image_path<'py>(&self, py: Python<'py>, image_path: &str) -> PyResult<Bound<'py, PyAny>> {
        let desktop = self.inner.clone();
        let image_path = image_path.to_string();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let result = desktop.ocr_image_path(&image_path).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(result)
        })
    }

    #[pyo3(name = "ocr_screenshot", text_signature = "($self, screenshot)")]
    /// (async) Perform OCR on a screenshot.
    /// 
    /// Args:
    ///     screenshot (ScreenshotResult): The screenshot to process.
    /// 
    /// Returns:
    ///     str: The extracted text.
    pub fn ocr_screenshot<'py>(&self, py: Python<'py>, screenshot: &ScreenshotResult) -> PyResult<Bound<'py, PyAny>> {
        let desktop = self.inner.clone();
        let core_screenshot = ::terminator_core::ScreenshotResult {
            image_data: screenshot.image_data.clone(),
            width: screenshot.width,
            height: screenshot.height,
        };
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let result = desktop.ocr_screenshot(&core_screenshot).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(result)
        })
    }

    #[pyo3(name = "find_window_by_criteria", signature = (title_contains=None, timeout_ms=None))]
    #[pyo3(text_signature = "($self, title_contains, timeout_ms)")]
    /// (async) Find a window by criteria.
    /// 
    /// Args:
    ///     title_contains (Optional[str]): Text that should be in the window title.
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    /// 
    /// Returns:
    ///     UIElement: The found window element.
    pub fn find_window_by_criteria<'py>(&self, py: Python<'py>, title_contains: Option<&str>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let desktop = self.inner.clone();
        let title_contains = title_contains.map(|s| s.to_string());
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let result = desktop.find_window_by_criteria(title_contains.as_deref(), timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            let py_result = UIElement { inner: result };
            Ok(py_result)
        })
    }

    #[pyo3(name = "get_current_browser_window", text_signature = "($self)")]
    /// (async) Get the currently focused browser window.
    /// 
    /// Returns:
    ///     UIElement: The current browser window element.
    pub fn get_current_browser_window<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let desktop = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let result = desktop.get_current_browser_window().await.map_err(|e| automation_error_to_pyerr(e))?;
            let py_result = UIElement { inner: result };
            Ok(py_result)
        })
    }

    #[pyo3(name = "get_current_window", text_signature = "($self)")]
    /// (async) Get the currently focused window.
    /// 
    /// Returns:
    ///     UIElement: The current window element.
    pub fn get_current_window<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let desktop = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let result = desktop.get_current_window().await.map_err(|e| automation_error_to_pyerr(e))?;
            let py_result = UIElement { inner: result };
            Ok(py_result)
        })
    }

    #[pyo3(name = "get_current_application", text_signature = "($self)")]
    /// (async) Get the currently focused application.
    /// 
    /// Returns:
    ///     UIElement: The current application element.
    pub fn get_current_application<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let desktop = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let result = desktop.get_current_application().await.map_err(|e| automation_error_to_pyerr(e))?;
            let py_result = UIElement { inner: result };
            Ok(py_result)
        })
    }

    #[pyo3(name = "open_url", signature = (url, browser=None))]
    #[pyo3(text_signature = "($self, url, browser)")]
    /// Open a URL in a browser.
    /// 
    /// Args:
    ///     url (str): The URL to open.
    ///     browser (Optional[str]): The browser to use.
    pub fn open_url(&self, url: &str, browser: Option<&str>) -> PyResult<()> {
        self.inner.open_url(url, browser)
            .map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "open_file", text_signature = "($self, file_path)")]
    /// Open a file with its default application.
    /// 
    /// Args:
    ///     file_path (str): Path to the file to open.
    pub fn open_file(&self, file_path: &str) -> PyResult<()> {
        self.inner.open_file(file_path)
            .map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "activate_browser_window_by_title", text_signature = "($self, title)")]
    /// Activate a browser window by title.
    /// 
    /// Args:
    ///     title (str): The window title to match.
    pub fn activate_browser_window_by_title(&self, title: &str) -> PyResult<()> {
        self.inner.activate_browser_window_by_title(title)
            .map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "focused_element", text_signature = "($self)")]
    /// Get the currently focused element.
    /// 
    /// Returns:
    ///     UIElement: The focused element.
    pub fn focused_element(&self) -> PyResult<UIElement> {
        self.inner.focused_element()
            .map(|e| UIElement { inner: e })
            .map_err(|e| automation_error_to_pyerr(e))
    }
} 