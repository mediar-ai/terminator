use napi_derive::napi;
use std::collections::HashMap;
use crate::Element;

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
pub struct ScreenshotResult {
    pub width: u32,
    pub height: u32,
    pub image_data: Vec<u8>,
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