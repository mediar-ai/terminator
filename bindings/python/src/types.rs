use pyo3::prelude::*;
use pyo3_stub_gen::derive::*;
use std::collections::HashMap;
use ::terminator_core::{
    ScreenshotResult as CoreScreenshotResult,
    ClickResult as CoreClickResult,
    CommandOutput as CoreCommandOutput,
};
use serde_json;
use serde::Serialize;

/// Result of a screenshot operation.
#[gen_stub_pyclass]
#[pyclass(name = "ScreenshotResult")]
#[derive(Serialize)]
pub struct ScreenshotResult {
    #[pyo3(get)]
    pub width: u32,
    #[pyo3(get)]
    pub height: u32,
    #[pyo3(get)]
    pub image_data: Vec<u8>,
}

/// Result of a click operation.
#[gen_stub_pyclass]
#[pyclass(name = "ClickResult")]
#[derive(Serialize)]
pub struct ClickResult {
    #[pyo3(get)]
    pub method: String,
    #[pyo3(get)]
    pub coordinates: Option<Coordinates>,
    #[pyo3(get)]
    pub details: String,
}

/// Result of a command execution.
#[gen_stub_pyclass]
#[pyclass(name = "CommandOutput")]
#[derive(Serialize)]
pub struct CommandOutput {
    #[pyo3(get)]
    pub exit_status: Option<i32>,
    #[pyo3(get)]
    pub stdout: String,
    #[pyo3(get)]
    pub stderr: String,
}

/// UI Element attributes
#[gen_stub_pyclass]
#[pyclass(name = "UIElementAttributes")]
#[derive(Serialize)]
pub struct UIElementAttributes {
    #[pyo3(get)]
    pub role: String,
    #[pyo3(get)]
    pub name: Option<String>,
    #[pyo3(get)]
    pub label: Option<String>,
    #[pyo3(get)]
    pub value: Option<String>,
    #[pyo3(get)]
    pub description: Option<String>,
    #[pyo3(get)]
    pub properties: HashMap<String, Option<String>>,
    #[pyo3(get)]
    pub is_keyboard_focusable: Option<bool>,
}

/// Coordinates for mouse operations
#[gen_stub_pyclass]
#[pyclass(name = "Coordinates")]
#[derive(Clone, Serialize)]
pub struct Coordinates {
    #[pyo3(get)]
    pub x: f64,
    #[pyo3(get)]
    pub y: f64,
}

/// Bounds for element coordinates
#[gen_stub_pyclass]
#[pyclass(name = "Bounds")]
#[derive(Clone, Serialize)]
pub struct Bounds {
    #[pyo3(get)]
    pub x: f64,
    #[pyo3(get)]
    pub y: f64,
    #[pyo3(get)]
    pub width: f64,
    #[pyo3(get)]
    pub height: f64,
}

/// Details about an explored element
#[gen_stub_pyclass]
#[pyclass(name = "ExploredElementDetail")]
#[derive(Clone, Serialize)]
pub struct ExploredElementDetail {
    #[pyo3(get)]
    pub role: String,
    #[pyo3(get)]
    pub name: Option<String>,
    #[pyo3(get)]
    pub id: Option<String>,
    #[pyo3(get)]
    pub bounds: Option<Bounds>,
    #[pyo3(get)]
    pub value: Option<String>,
    #[pyo3(get)]
    pub description: Option<String>,
    #[pyo3(get)]
    pub text: Option<String>,
    #[pyo3(get)]
    pub parent_id: Option<String>,
    #[pyo3(get)]
    pub children_ids: Vec<String>,
    #[pyo3(get)]
    pub suggested_selector: String,
}

/// Response from exploring an element
#[gen_stub_pyclass]
#[pyclass(name = "ExploreResponse")]
#[derive(Clone, Serialize)]
pub struct ExploreResponse {
    #[pyo3(get)]
    pub parent: crate::element::UIElement,
    #[pyo3(get)]
    pub children: Vec<ExploredElementDetail>,
}

impl From<CoreScreenshotResult> for ScreenshotResult {
    fn from(r: CoreScreenshotResult) -> Self {
        ScreenshotResult {
            width: r.width,
            height: r.height,
            image_data: r.image_data,
        }
    }
}

impl From<CoreClickResult> for ClickResult {
    fn from(r: CoreClickResult) -> Self {
        ClickResult {
            method: r.method,
            coordinates: r.coordinates.map(|(x, y)| Coordinates { x, y }),
            details: r.details,
        }
    }
}

impl From<CoreCommandOutput> for CommandOutput {
    fn from(r: CoreCommandOutput) -> Self {
        CommandOutput {
            exit_status: r.exit_status,
            stdout: r.stdout,
            stderr: r.stderr,
        }
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl ExploreResponse {
    fn __repr__(&self) -> PyResult<String> {
        serde_json::to_string(self)
            .map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()))
    }
    fn __str__(&self) -> PyResult<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()))
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl ClickResult {
    fn __repr__(&self) -> PyResult<String> {
        serde_json::to_string(self)
            .map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()))
    }
    fn __str__(&self) -> PyResult<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()))
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl UIElementAttributes {
    fn __repr__(&self) -> PyResult<String> {
        serde_json::to_string(self)
            .map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()))
    }
    fn __str__(&self) -> PyResult<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()))
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl ScreenshotResult {
    fn __repr__(&self) -> PyResult<String> {
        serde_json::to_string(self)
            .map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()))
    }
    fn __str__(&self) -> PyResult<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()))
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl CommandOutput {
    fn __repr__(&self) -> PyResult<String> {
        serde_json::to_string(self)
            .map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()))
    }
    fn __str__(&self) -> PyResult<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()))
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl Coordinates {
    fn __repr__(&self) -> PyResult<String> {
        serde_json::to_string(self)
            .map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()))
    }
    fn __str__(&self) -> PyResult<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()))
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl Bounds {
    fn __repr__(&self) -> PyResult<String> {
        serde_json::to_string(self)
            .map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()))
    }
    fn __str__(&self) -> PyResult<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()))
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl ExploredElementDetail {
    fn __repr__(&self) -> PyResult<String> {
        serde_json::to_string(self)
            .map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()))
    }
    fn __str__(&self) -> PyResult<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()))
    }
} 