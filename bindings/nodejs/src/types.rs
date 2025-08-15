use crate::Element;
use napi_derive::napi;
use std::collections::HashMap;

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

#[napi(object)]
pub struct Monitor {
    pub id: String,
    pub name: String,
    pub is_primary: bool,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub scale_factor: f64,
}

#[napi(object)]
pub struct MonitorScreenshotPair {
    pub monitor: Monitor,
    pub screenshot: ScreenshotResult,
}

#[napi(object)]
pub struct ScreenshotResult {
    pub width: u32,
    pub height: u32,
    pub image_data: Vec<u8>,
    pub monitor: Option<Monitor>,
}

#[napi(object, js_name = "UIElementAttributes")]
pub struct UIElementAttributes {
    pub role: String,
    pub name: Option<String>,
    pub label: Option<String>,
    pub value: Option<String>,
    pub description: Option<String>,
    pub properties: HashMap<String, Option<String>>,
    pub is_keyboard_focusable: Option<bool>,
    pub bounds: Option<Bounds>,
}

#[napi(object, js_name = "ExploredElementDetail")]
pub struct ExploredElementDetail {
    pub role: String,
    pub name: Option<String>,
    pub id: Option<String>,
    pub bounds: Option<Bounds>,
    pub value: Option<String>,
    pub description: Option<String>,
    pub text: Option<String>,
    pub parent_id: Option<String>,
    pub children_ids: Vec<String>,
    pub suggested_selector: String,
}

#[napi(object, js_name = "ExploreResponse")]
pub struct ExploreResponse {
    pub parent: Element,
    pub children: Vec<ExploredElementDetail>,
}

#[napi(object, js_name = "UINode")]
pub struct UINode {
    pub id: Option<String>,
    pub attributes: UIElementAttributes,
    pub children: Vec<UINode>,
}

#[napi(string_enum)]
pub enum PropertyLoadingMode {
    /// Only load essential properties (role + name) - fastest
    Fast,
    /// Load all properties for complete element data - slower but comprehensive
    Complete,
    /// Load specific properties based on element type - balanced approach
    Smart,
}

#[napi(object, js_name = "TreeBuildConfig")]
pub struct TreeBuildConfig {
    /// Property loading strategy
    pub property_mode: PropertyLoadingMode,
    /// Optional timeout per operation in milliseconds
    pub timeout_per_operation_ms: Option<i64>,
    /// Optional yield frequency for responsiveness
    pub yield_every_n_elements: Option<i32>,
    /// Optional batch size for processing elements
    pub batch_size: Option<i32>,
}

impl From<(f64, f64, f64, f64)> for Bounds {
    fn from(t: (f64, f64, f64, f64)) -> Self {
        Bounds {
            x: t.0,
            y: t.1,
            width: t.2,
            height: t.3,
        }
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

impl From<terminator::Monitor> for Monitor {
    fn from(m: terminator::Monitor) -> Self {
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

impl From<terminator::UINode> for UINode {
    fn from(node: terminator::UINode) -> Self {
        UINode {
            id: node.id,
            attributes: UIElementAttributes::from(node.attributes),
            children: node.children.into_iter().map(UINode::from).collect(),
        }
    }
}

impl From<terminator::UIElementAttributes> for UIElementAttributes {
    fn from(attrs: terminator::UIElementAttributes) -> Self {
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

#[napi(string_enum)]
pub enum TextPosition {
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
    TopLeft,
    Inside,
}

#[napi(object)]
pub struct FontStyle {
    pub size: u32,
    pub bold: bool,
    pub color: u32,
}

#[napi]
pub struct HighlightHandle {
    inner: Option<terminator::HighlightHandle>,
}

#[napi]
impl HighlightHandle {
    #[napi]
    pub fn close(&mut self) {
        if let Some(handle) = self.inner.take() {
            handle.close();
        }
    }
}

impl HighlightHandle {
    pub fn new(handle: terminator::HighlightHandle) -> Self {
        Self {
            inner: Some(handle),
        }
    }

    pub fn new_dummy() -> Self {
        Self { inner: None }
    }
}

impl From<TextPosition> for terminator::TextPosition {
    fn from(pos: TextPosition) -> Self {
        match pos {
            TextPosition::Top => terminator::TextPosition::Top,
            TextPosition::TopRight => terminator::TextPosition::TopRight,
            TextPosition::Right => terminator::TextPosition::Right,
            TextPosition::BottomRight => terminator::TextPosition::BottomRight,
            TextPosition::Bottom => terminator::TextPosition::Bottom,
            TextPosition::BottomLeft => terminator::TextPosition::BottomLeft,
            TextPosition::Left => terminator::TextPosition::Left,
            TextPosition::TopLeft => terminator::TextPosition::TopLeft,
            TextPosition::Inside => terminator::TextPosition::Inside,
        }
    }
}

impl From<FontStyle> for terminator::FontStyle {
    fn from(style: FontStyle) -> Self {
        terminator::FontStyle {
            size: style.size,
            bold: style.bold,
            color: style.color,
        }
    }
}

impl Default for FontStyle {
    fn default() -> Self {
        Self {
            size: 12,
            bold: false,
            color: 0x000000,
        }
    }
}

impl From<TreeBuildConfig> for terminator::platforms::TreeBuildConfig {
    fn from(config: TreeBuildConfig) -> Self {
        terminator::platforms::TreeBuildConfig {
            property_mode: match config.property_mode {
                PropertyLoadingMode::Fast => terminator::platforms::PropertyLoadingMode::Fast,
                PropertyLoadingMode::Complete => {
                    terminator::platforms::PropertyLoadingMode::Complete
                }
                PropertyLoadingMode::Smart => terminator::platforms::PropertyLoadingMode::Smart,
            },
            timeout_per_operation_ms: config.timeout_per_operation_ms.map(|x| x as u64),
            yield_every_n_elements: config.yield_every_n_elements.map(|x| x as usize),
            batch_size: config.batch_size.map(|x| x as usize),
        }
    }
}
