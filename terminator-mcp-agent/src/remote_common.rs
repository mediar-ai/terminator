// Common types shared between remote server and client
// This avoids code duplication and ensures consistency

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteRequest {
    pub action: RemoteAction,
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RemoteAction {
    GetWindowTree {
        pid: Option<u32>,
        include_detailed_attributes: Option<bool>,
    },
    GetApplications,
    Click {
        selector: String,
        button: Option<MouseButton>,
    },
    TypeText {
        selector: String,
        text: String,
    },
    PressKey {
        selector: String,
        key: String,
    },
    GetElementProperties {
        selector: String,
    },
    WaitForElement {
        selector: String,
        condition: WaitCondition,
        timeout_ms: Option<u64>,
    },
    TakeScreenshot {
        selector: Option<String>,
        full_page: Option<bool>,
    },
    SetValue {
        selector: String,
        value: String,
    },
    InvokeElement {
        selector: String,
    },
    ValidateElement {
        selector: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WaitCondition {
    Visible,
    Enabled,
    Focused,
    Exists,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteResponse {
    pub request_id: String,
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub client_address: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementProperties {
    pub name: Option<String>,
    pub role: String,
    pub is_enabled: bool,
    pub is_visible: bool,
    pub bounds: Option<serde_json::Value>,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub exists: bool,
    pub name: Option<String>,
    pub role: Option<String>,
    pub is_enabled: Option<bool>,
    pub is_visible: Option<bool>,
}