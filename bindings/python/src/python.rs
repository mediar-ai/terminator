#![allow(non_local_definitions)]
#![allow(unsafe_op_in_unsafe_fn)]

use pyo3::prelude::*;
use pyo3::exceptions::PyRuntimeError;
use pyo3_asyncio::tokio as pyo3_tokio;
use terminator::{Desktop, element::UIElement, selector::Selector, ScreenshotResult, locator::Locator, ClickResult, CommandOutput, errors::AutomationError};
use pyo3::create_exception;
use std::sync::Once;

/// Main entry point for desktop automation.
#[pyclass]
pub struct PyDesktop {
    inner: Desktop,
}

/// Represents a UI element in the desktop UI tree.
#[pyclass]
#[derive(Clone)]
pub struct PyUIElement {
    inner: UIElement,
}

/// Result of a screenshot operation.
#[pyclass]
pub struct PyScreenshotResult {
    #[pyo3(get)]
    pub width: u32,
    #[pyo3(get)]
    pub height: u32,
    #[pyo3(get)]
    pub image_data: Vec<u8>,
}

/// Result of a click operation.
#[pyclass]
pub struct PyClickResult {
    #[pyo3(get)]
    pub method: String,
    #[pyo3(get)]
    pub coordinates: Option<(f64, f64)>,
    #[pyo3(get)]
    pub details: String,
}

/// Result of a command execution.
#[pyclass]
pub struct PyCommandOutput {
    #[pyo3(get)]
    pub exit_status: Option<i32>,
    #[pyo3(get)]
    pub stdout: String,
    #[pyo3(get)]
    pub stderr: String,
}

/// Locator for finding UI elements by selector.
#[pyclass]
pub struct PyLocator {
    inner: Locator,
}

// Custom Python exceptions for advanced error mapping
create_exception!(terminator, ElementNotFoundError, pyo3::exceptions::PyRuntimeError);
create_exception!(terminator, TimeoutError, pyo3::exceptions::PyRuntimeError);
create_exception!(terminator, PermissionDeniedError, pyo3::exceptions::PyRuntimeError);
create_exception!(terminator, PlatformError, pyo3::exceptions::PyRuntimeError);
create_exception!(terminator, UnsupportedOperationError, pyo3::exceptions::PyRuntimeError);
create_exception!(terminator, UnsupportedPlatformError, pyo3::exceptions::PyRuntimeError);
create_exception!(terminator, InvalidArgumentError, pyo3::exceptions::PyRuntimeError);
create_exception!(terminator, InternalError, pyo3::exceptions::PyRuntimeError);

impl From<ScreenshotResult> for PyScreenshotResult {
    fn from(r: ScreenshotResult) -> Self {
        PyScreenshotResult {
            width: r.width,
            height: r.height,
            image_data: r.image_data,
        }
    }
}

impl From<ClickResult> for PyClickResult {
    fn from(r: ClickResult) -> Self {
        PyClickResult {
            method: r.method,
            coordinates: r.coordinates,
            details: r.details,
        }
    }
}

impl From<CommandOutput> for PyCommandOutput {
    fn from(r: CommandOutput) -> Self {
        PyCommandOutput {
            exit_status: r.exit_status,
            stdout: r.stdout,
            stderr: r.stderr,
        }
    }
}

// Advanced error mapping
fn automation_error_to_pyerr(e: AutomationError) -> pyo3::PyErr {
    use terminator::AutomationError::*;
    let msg = format!("{e}");
    match e {
        ElementNotFound(_) => crate::python::ElementNotFoundError::new_err(msg),
        Timeout(_) => crate::python::TimeoutError::new_err(msg),
        PermissionDenied(_) => crate::python::PermissionDeniedError::new_err(msg),
        PlatformError(_) => crate::python::PlatformError::new_err(msg),
        UnsupportedOperation(_) => crate::python::UnsupportedOperationError::new_err(msg),
        UnsupportedPlatform(_) => crate::python::UnsupportedPlatformError::new_err(msg),
        InvalidArgument(_) => crate::python::InvalidArgumentError::new_err(msg),
        Internal(_) => crate::python::InternalError::new_err(msg),
    }
}

#[pymethods]
impl PyDesktop {
    #[new]
    #[pyo3(text_signature = "()")]
    /// Create a new PyDesktop instance.
    pub fn new() -> PyResult<Self> {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let _ = tracing_subscriber::fmt()
                .with_env_filter("info")
                .try_init();
        });
        let desktop = tokio::runtime::Runtime::new()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?
            .block_on(Desktop::new(false, false))
            .map_err(|e| automation_error_to_pyerr(e))?;
        Ok(PyDesktop { inner: desktop })
    }

    #[pyo3(text_signature = "($self)")]
    /// Returns the root UI element.
    pub fn root(&self) -> PyResult<PyUIElement> {
        let root = self.inner.root();
        Ok(PyUIElement { inner: root })
    }

    #[pyo3(text_signature = "($self)")]
    /// Returns a list of top-level application UI elements.
    pub fn applications(&self) -> PyResult<Vec<PyUIElement>> {
        self.inner.applications()
            .map(|apps| apps.into_iter().map(|e| PyUIElement { inner: e }).collect())
            .map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(text_signature = "($self, name)")]
    /// Returns the UI element for the given application name.
    pub fn application(&self, name: &str) -> PyResult<PyUIElement> {
        self.inner.application(name)
            .map(|e| PyUIElement { inner: e })
            .map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(text_signature = "($self, name)")]
    /// Opens an application by name.
    pub fn open_application(&self, name: &str) -> PyResult<()> {
        self.inner.open_application(name)
            .map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(text_signature = "($self, name)")]
    /// Activates an application by name.
    pub fn activate_application(&self, name: &str) -> PyResult<()> {
        self.inner.activate_application(name)
            .map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "locator", text_signature = "($self, selector)")]
    /// Returns a PyLocator for the given selector string.
    pub fn locator_py(&self, selector: &str) -> PyResult<PyLocator> {
        let locator = self.inner.locator(selector);
        Ok(PyLocator { inner: locator })
    }

    #[pyo3(name = "capture_screen", text_signature = "($self)")]
    /// Async: Capture a screenshot of the primary monitor.
    pub fn capture_screen_py<'py>(&self, py: Python<'py>) -> PyResult<&'py pyo3::types::PyAny> {
        let desktop = self.inner.clone();
        pyo3_tokio::future_into_py(py, async move {
            let result = desktop.capture_screen().await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(PyScreenshotResult::from(result))
        })
    }

    #[pyo3(name = "run_command", text_signature = "($self, windows_command, unix_command)")]
    pub fn run_command_py<'py>(&self, py: Python<'py>, windows_command: Option<&str>, unix_command: Option<&str>) -> PyResult<&'py pyo3::types::PyAny> {
        let desktop = self.inner.clone();
        let windows_command = windows_command.map(|s| s.to_string());
        let unix_command = unix_command.map(|s| s.to_string());
        pyo3_tokio::future_into_py(py, async move {
            let result = desktop.run_command(windows_command.as_deref(), unix_command.as_deref()).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(PyCommandOutput::from(result))
        })
    }

    #[pyo3(name = "capture_monitor_by_name", text_signature = "($self, name)")]
    pub fn capture_monitor_by_name_py<'py>(&self, py: Python<'py>, name: &str) -> PyResult<&'py pyo3::types::PyAny> {
        let desktop = self.inner.clone();
        let name = name.to_string();
        pyo3_tokio::future_into_py(py, async move {
            let result = desktop.capture_monitor_by_name(&name).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(PyScreenshotResult::from(result))
        })
    }

    #[pyo3(name = "ocr_image_path", text_signature = "($self, image_path)")]
    pub fn ocr_image_path_py<'py>(&self, py: Python<'py>, image_path: &str) -> PyResult<&'py pyo3::types::PyAny> {
        let desktop = self.inner.clone();
        let image_path = image_path.to_string();
        pyo3_tokio::future_into_py(py, async move {
            let result = desktop.ocr_image_path(&image_path).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(result)
        })
    }

    #[pyo3(name = "ocr_screenshot", text_signature = "($self, screenshot)")]
    pub fn ocr_screenshot_py<'py>(&self, py: Python<'py>, screenshot: &PyScreenshotResult) -> PyResult<&'py pyo3::types::PyAny> {
        let desktop = self.inner.clone();
        let screenshot = ScreenshotResult {
            image_data: screenshot.image_data.clone(),
            width: screenshot.width,
            height: screenshot.height,
        };
        pyo3_tokio::future_into_py(py, async move {
            let result = desktop.ocr_screenshot(&screenshot).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(result)
        })
    }

    #[pyo3(name = "find_window_by_criteria", text_signature = "($self, title_contains, timeout_ms)")]
    pub fn find_window_by_criteria_py<'py>(&self, py: Python<'py>, title_contains: Option<&str>, timeout_ms: Option<u64>) -> PyResult<&'py pyo3::types::PyAny> {
        let desktop = self.inner.clone();
        let title_contains = title_contains.map(|s| s.to_string());
        pyo3_tokio::future_into_py(py, async move {
            let result = desktop.find_window_by_criteria(title_contains.as_deref(), timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(PyUIElement { inner: result })
        })
    }

    #[pyo3(name = "get_current_browser_window", text_signature = "($self)")]
    pub fn get_current_browser_window_py<'py>(&self, py: Python<'py>) -> PyResult<&'py pyo3::types::PyAny> {
        let desktop = self.inner.clone();
        pyo3_tokio::future_into_py(py, async move {
            let result = desktop.get_current_browser_window().await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(PyUIElement { inner: result })
        })
    }
}

#[pymethods]
impl PyUIElement {
    #[getter]
    pub fn role(&self) -> String {
        self.inner.role()
    }

    #[getter]
    pub fn name(&self) -> Option<String> {
        self.inner.name()
    }

    pub fn children(&self) -> PyResult<Vec<PyUIElement>> {
        self.inner.children()
            .map(|kids| kids.into_iter().map(|e| PyUIElement { inner: e }).collect())
            .map_err(|e| automation_error_to_pyerr(e))
    }

    pub fn parent(&self) -> PyResult<Option<PyUIElement>> {
        self.inner.parent()
            .map(|opt| opt.map(|e| PyUIElement { inner: e }))
            .map_err(|e| automation_error_to_pyerr(e))
    }

    #[getter]
    pub fn bounds(&self) -> PyResult<(f64, f64, f64, f64)> {
        self.inner.bounds().map_err(|e| automation_error_to_pyerr(e))
    }

    pub fn click(&self) -> PyResult<()> {
        self.inner.click().map(|_| ()).map_err(|e| automation_error_to_pyerr(e))
    }

    pub fn is_visible(&self) -> PyResult<bool> {
        self.inner.is_visible().map_err(|e| automation_error_to_pyerr(e))
    }

    pub fn is_enabled(&self) -> PyResult<bool> {
        self.inner.is_enabled().map_err(|e| automation_error_to_pyerr(e))
    }

    pub fn focus(&self) -> PyResult<()> {
        self.inner.focus().map_err(|e| automation_error_to_pyerr(e))
    }

    pub fn text(&self, max_depth: Option<usize>) -> PyResult<String> {
        self.inner.text(max_depth.unwrap_or(1)).map_err(|e| automation_error_to_pyerr(e))
    }

    pub fn type_text(&self, text: &str, use_clipboard: Option<bool>) -> PyResult<()> {
        self.inner.type_text(text, use_clipboard.unwrap_or(false)).map_err(|e| automation_error_to_pyerr(e))
    }

    pub fn press_key(&self, key: &str) -> PyResult<()> {
        self.inner.press_key(key).map_err(|e| automation_error_to_pyerr(e))
    }

    pub fn set_value(&self, value: &str) -> PyResult<()> {
        self.inner.set_value(value).map_err(|e| automation_error_to_pyerr(e))
    }

    pub fn perform_action(&self, action: &str) -> PyResult<()> {
        self.inner.perform_action(action).map_err(|e| automation_error_to_pyerr(e))
    }

    pub fn scroll(&self, direction: &str, amount: f64) -> PyResult<()> {
        self.inner.scroll(direction, amount).map_err(|e| automation_error_to_pyerr(e))
    }

    pub fn activate_window(&self) -> PyResult<()> {
        self.inner.activate_window().map_err(|e| automation_error_to_pyerr(e))
    }

    pub fn is_focused(&self) -> PyResult<bool> {
        self.inner.is_focused().map_err(|e| automation_error_to_pyerr(e))
    }

    pub fn is_keyboard_focusable(&self) -> PyResult<bool> {
        self.inner.is_keyboard_focusable().map_err(|e| automation_error_to_pyerr(e))
    }

    pub fn mouse_drag(&self, start_x: f64, start_y: f64, end_x: f64, end_y: f64) -> PyResult<()> {
        self.inner.mouse_drag(start_x, start_y, end_x, end_y).map_err(|e| automation_error_to_pyerr(e))
    }

    pub fn mouse_click_and_hold(&self, x: f64, y: f64) -> PyResult<()> {
        self.inner.mouse_click_and_hold(x, y).map_err(|e| automation_error_to_pyerr(e))
    }

    pub fn mouse_move(&self, x: f64, y: f64) -> PyResult<()> {
        self.inner.mouse_move(x, y).map_err(|e| automation_error_to_pyerr(e))
    }

    pub fn mouse_release(&self) -> PyResult<()> {
        self.inner.mouse_release().map_err(|e| automation_error_to_pyerr(e))
    }

    #[pyo3(name = "locator", text_signature = "($self, selector)")]
    pub fn locator_py<'py>(&self, py: Python<'py>, selector: &str) -> PyResult<&'py pyo3::types::PyAny> {
        let sel: Selector = selector.into();
        let locator = self.inner.locator(sel).map_err(|e| automation_error_to_pyerr(e))?;
        pyo3_tokio::future_into_py(py, async move {
            let element = locator.first(None).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(PyUIElement { inner: element })
        })
    }
}

#[pymethods]
impl PyLocator {
    #[pyo3(name = "first", text_signature = "($self)")]
    pub fn first_py<'py>(&self, py: Python<'py>) -> PyResult<&'py pyo3::types::PyAny> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py(py, async move {
            let element = locator.first(None).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(PyUIElement { inner: element })
        })
    }

    #[pyo3(name = "all", text_signature = "($self, timeout_ms, depth)")]
    pub fn all_py<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>, depth: Option<usize>) -> PyResult<&'py pyo3::types::PyAny> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py(py, async move {
            let elements = locator.all(timeout_ms.map(std::time::Duration::from_millis), depth).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(elements.into_iter().map(|e| PyUIElement { inner: e }).collect::<Vec<_>>())
        })
    }

    #[pyo3(name = "wait", text_signature = "($self, timeout_ms)")]
    pub fn wait_py<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<&'py pyo3::types::PyAny> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py(py, async move {
            let element = locator.wait(timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(PyUIElement { inner: element })
        })
    }

    #[pyo3(name = "timeout", text_signature = "($self, timeout_ms)")]
    pub fn timeout_py(&self, timeout_ms: u64) -> PyLocator {
        PyLocator { inner: self.inner.clone().set_default_timeout(std::time::Duration::from_millis(timeout_ms)) }
    }

    #[pyo3(name = "locator", text_signature = "($self, selector)")]
    pub fn locator_py(&self, selector: &str) -> PyResult<PyLocator> {
        let locator = self.inner.locator(selector);
        Ok(PyLocator { inner: locator })
    }
}
