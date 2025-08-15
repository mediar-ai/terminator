use ::terminator_core::{
    ClickResult as CoreClickResult, CommandOutput as CoreCommandOutput,
    ScreenshotResult as CoreScreenshotResult,
};
use pyo3::prelude::*;
use pyo3_stub_gen::derive::*;
use serde::Serialize;
use std::collections::HashMap;

/// Monitor/display information.
#[gen_stub_pyclass]
#[pyclass(name = "Monitor")]
#[derive(Serialize, Clone)]
pub struct Monitor {
    #[pyo3(get)]
    pub id: String,
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub is_primary: bool,
    #[pyo3(get)]
    pub width: u32,
    #[pyo3(get)]
    pub height: u32,
    #[pyo3(get)]
    pub x: i32,
    #[pyo3(get)]
    pub y: i32,
    #[pyo3(get)]
    pub scale_factor: f64,
}

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
    #[pyo3(get)]
    pub monitor: Option<Monitor>,
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
#[derive(Clone, Serialize)]
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
    #[pyo3(get)]
    pub bounds: Option<Bounds>,
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

/// UI Node representing a tree structure of UI elements
#[gen_stub_pyclass]
#[pyclass(name = "UINode")]
#[derive(Clone, Serialize)]
pub struct UINode {
    #[pyo3(get)]
    pub id: Option<String>,
    #[pyo3(get)]
    pub attributes: UIElementAttributes,
    #[pyo3(get)]
    pub children: Vec<UINode>,
}

/// Property loading strategy for tree building
#[gen_stub_pyclass]
#[pyclass(name = "PropertyLoadingMode")]
#[derive(Clone, Serialize)]
pub struct PropertyLoadingMode {
    #[pyo3(get)]
    pub mode: String,
}

impl PropertyLoadingMode {
    pub fn fast() -> Self {
        PropertyLoadingMode {
            mode: "Fast".to_string(),
        }
    }

    pub fn complete() -> Self {
        PropertyLoadingMode {
            mode: "Complete".to_string(),
        }
    }

    pub fn smart() -> Self {
        PropertyLoadingMode {
            mode: "Smart".to_string(),
        }
    }
}

/// Configuration for tree building performance and completeness
#[gen_stub_pyclass]
#[pyclass(name = "TreeBuildConfig")]
#[derive(Clone, Serialize)]
pub struct TreeBuildConfig {
    #[pyo3(get)]
    pub property_mode: PropertyLoadingMode,
    #[pyo3(get)]
    pub timeout_per_operation_ms: Option<u64>,
    #[pyo3(get)]
    pub yield_every_n_elements: Option<usize>,
    #[pyo3(get)]
    pub batch_size: Option<usize>,
}

/// Position options for text overlays in highlighting
#[gen_stub_pyclass]
#[pyclass(name = "TextPosition")]
#[derive(Clone, Serialize)]
pub struct TextPosition {
    #[pyo3(get)]
    pub position: String,
}

#[gen_stub_pymethods]
#[pymethods]
impl TextPosition {
    #[classmethod]
    pub fn top(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        TextPosition {
            position: "Top".to_string(),
        }
    }

    #[classmethod]
    pub fn top_right(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        TextPosition {
            position: "TopRight".to_string(),
        }
    }

    #[classmethod]
    pub fn right(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        TextPosition {
            position: "Right".to_string(),
        }
    }

    #[classmethod]
    pub fn bottom_right(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        TextPosition {
            position: "BottomRight".to_string(),
        }
    }

    #[classmethod]
    pub fn bottom(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        TextPosition {
            position: "Bottom".to_string(),
        }
    }

    #[classmethod]
    pub fn bottom_left(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        TextPosition {
            position: "BottomLeft".to_string(),
        }
    }

    #[classmethod]
    pub fn left(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        TextPosition {
            position: "Left".to_string(),
        }
    }

    #[classmethod]
    pub fn top_left(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        TextPosition {
            position: "TopLeft".to_string(),
        }
    }

    #[classmethod]
    pub fn inside(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        TextPosition {
            position: "Inside".to_string(),
        }
    }

    fn __repr__(&self) -> PyResult<String> {
        serde_json::to_string(self)
            .map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()))
    }
    fn __str__(&self) -> PyResult<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()))
    }
}

/// Font styling options for text overlays
#[gen_stub_pyclass]
#[pyclass(name = "FontStyle")]
#[derive(Clone, Serialize)]
pub struct FontStyle {
    #[pyo3(get)]
    pub size: u32,
    #[pyo3(get)]
    pub bold: bool,
    #[pyo3(get)]
    pub color: u32,
}

#[gen_stub_pymethods]
#[pymethods]
impl FontStyle {
    #[new]
    pub fn new(size: Option<u32>, bold: Option<bool>, color: Option<u32>) -> Self {
        FontStyle {
            size: size.unwrap_or(12),
            bold: bold.unwrap_or(false),
            color: color.unwrap_or(0x000000), // Black
        }
    }

    fn __repr__(&self) -> PyResult<String> {
        serde_json::to_string(self)
            .map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()))
    }
    fn __str__(&self) -> PyResult<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()))
    }
}

/// Handle for managing active highlights with cleanup
#[gen_stub_pyclass]
#[pyclass(name = "HighlightHandle")]
pub struct HighlightHandle {
    inner: Option<::terminator_core::HighlightHandle>,
}

#[gen_stub_pymethods]
#[pymethods]
impl HighlightHandle {
    /// Manually close the highlight
    pub fn close(&mut self) {
        if let Some(handle) = self.inner.take() {
            handle.close();
        }
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok("HighlightHandle".to_string())
    }
    fn __str__(&self) -> PyResult<String> {
        Ok("HighlightHandle".to_string())
    }
}

impl HighlightHandle {
    pub fn new(handle: ::terminator_core::HighlightHandle) -> Self {
        Self {
            inner: Some(handle),
        }
    }

    pub fn new_dummy() -> Self {
        Self { inner: None }
    }
}

impl From<::terminator_core::Monitor> for Monitor {
    fn from(m: ::terminator_core::Monitor) -> Self {
        Monitor {
            id: m.id,
            name: m.name,
            is_primary: m.is_primary,
            width: m.width,
            height: m.height,
            x: m.x,
            y: m.y,
            scale_factor: m.scale_factor,
        }
    }
}

impl From<CoreScreenshotResult> for ScreenshotResult {
    fn from(r: CoreScreenshotResult) -> Self {
        ScreenshotResult {
            width: r.width,
            height: r.height,
            image_data: r.image_data,
            monitor: r.monitor.map(Monitor::from),
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

impl From<::terminator_core::UINode> for UINode {
    fn from(node: ::terminator_core::UINode) -> Self {
        UINode {
            id: node.id,
            attributes: UIElementAttributes::from(node.attributes),
            children: node.children.into_iter().map(UINode::from).collect(),
        }
    }
}

impl From<::terminator_core::UIElementAttributes> for UIElementAttributes {
    fn from(attrs: ::terminator_core::UIElementAttributes) -> Self {
        // Convert HashMap<String, Option<serde_json::Value>> to HashMap<String, Option<String>>
        let properties = attrs
            .properties
            .into_iter()
            .map(|(k, v)| (k, v.map(|val| val.to_string())))
            .collect();

        UIElementAttributes {
            role: attrs.role,
            name: attrs.name,
            label: attrs.label,
            value: attrs.value,
            description: attrs.description,
            properties,
            is_keyboard_focusable: attrs.is_keyboard_focusable,
            bounds: attrs.bounds.map(|(x, y, width, height)| Bounds {
                x,
                y,
                width,
                height,
            }),
        }
    }
}

impl From<TreeBuildConfig> for ::terminator_core::platforms::TreeBuildConfig {
    fn from(config: TreeBuildConfig) -> Self {
        let property_mode = match config.property_mode.mode.as_str() {
            "Fast" => ::terminator_core::platforms::PropertyLoadingMode::Fast,
            "Complete" => ::terminator_core::platforms::PropertyLoadingMode::Complete,
            "Smart" => ::terminator_core::platforms::PropertyLoadingMode::Smart,
            _ => ::terminator_core::platforms::PropertyLoadingMode::Fast, // default
        };

        ::terminator_core::platforms::TreeBuildConfig {
            property_mode,
            timeout_per_operation_ms: config.timeout_per_operation_ms,
            yield_every_n_elements: config.yield_every_n_elements,
            batch_size: config.batch_size,
        }
    }
}

impl From<TextPosition> for ::terminator_core::TextPosition {
    fn from(pos: TextPosition) -> Self {
        match pos.position.as_str() {
            "Top" => ::terminator_core::TextPosition::Top,
            "TopRight" => ::terminator_core::TextPosition::TopRight,
            "Right" => ::terminator_core::TextPosition::Right,
            "BottomRight" => ::terminator_core::TextPosition::BottomRight,
            "Bottom" => ::terminator_core::TextPosition::Bottom,
            "BottomLeft" => ::terminator_core::TextPosition::BottomLeft,
            "Left" => ::terminator_core::TextPosition::Left,
            "TopLeft" => ::terminator_core::TextPosition::TopLeft,
            "Inside" => ::terminator_core::TextPosition::Inside,
            _ => ::terminator_core::TextPosition::Top, // default
        }
    }
}

impl From<FontStyle> for ::terminator_core::FontStyle {
    fn from(style: FontStyle) -> Self {
        ::terminator_core::FontStyle {
            size: style.size,
            bold: style.bold,
            color: style.color,
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

#[gen_stub_pymethods]
#[pymethods]
impl UINode {
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
impl PropertyLoadingMode {
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
impl Monitor {
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
impl TreeBuildConfig {
    fn __repr__(&self) -> PyResult<String> {
        serde_json::to_string(self)
            .map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()))
    }
    fn __str__(&self) -> PyResult<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()))
    }
}
