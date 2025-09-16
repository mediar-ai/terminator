use anyhow::{Context, Result};
use axum::{
    extract::{Query, State, Json},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use crate::utils::DesktopWrapper;

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

#[derive(Clone)]
pub struct RemoteServerState {
    desktop: Arc<Mutex<DesktopWrapper>>,
    sessions: Arc<Mutex<Vec<SessionInfo>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub client_address: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct AuthQuery {
    pub api_key: Option<String>,
}

impl RemoteServerState {
    pub fn new(desktop: Arc<Mutex<DesktopWrapper>>) -> Self {
        Self {
            desktop,
            sessions: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn handle_action(&self, action: RemoteAction) -> Result<serde_json::Value> {
        let desktop = self.desktop.lock().await;

        match action {
            RemoteAction::GetWindowTree { pid, include_detailed_attributes } => {
                let include_attrs = include_detailed_attributes.unwrap_or(true);

                if let Some(_process_id) = pid {
                    // Note: terminator doesn't provide PID directly from UIElement
                    // Would need Windows API to get process info
                    let apps = desktop.desktop.applications()?;
                    let app_info: Vec<_> = apps.iter().map(|app| {
                        serde_json::json!({
                            "name": app.name().unwrap_or_default(),
                            "role": app.role(),
                            "window_title": app.window_title()
                        })
                    }).collect();
                    Ok(serde_json::json!({
                        "applications": app_info,
                        "note": "PID filtering requires Windows API integration"
                    }))
                } else {
                    let apps = desktop.desktop.applications()?;
                    let app_info: Vec<_> = apps.iter().map(|app| {
                        serde_json::json!({
                            "name": app.name().unwrap_or_default(),
                            "role": app.role(),
                            "window_title": app.window_title()
                        })
                    }).collect();
                    Ok(serde_json::json!({
                        "applications": app_info,
                        "include_detailed_attributes": include_attrs
                    }))
                }
            }

            RemoteAction::GetApplications => {
                let apps = desktop.desktop.applications()?;
                Ok(serde_json::to_value(apps)?)
            }

            RemoteAction::Click { selector, button } => {
                let element = desktop.desktop.locator(selector.as_str())
                    .first(Some(std::time::Duration::from_secs(5)))
                    .await
                    .context("Element not found")?;

                match button.unwrap_or(MouseButton::Left) {
                    MouseButton::Left => element.click().map(|_| ())?,
                    MouseButton::Right => element.right_click()?,
                    MouseButton::Middle => {
                        element.click().map(|_| ())?;
                    }
                }

                Ok(serde_json::json!({ "clicked": true }))
            }

            RemoteAction::TypeText { selector, text } => {
                let element = desktop.desktop.locator(selector.as_str())
                    .first(Some(std::time::Duration::from_secs(5)))
                    .await
                    .context("Element not found")?;
                element.type_text(&text, false)?;
                Ok(serde_json::json!({ "typed": true, "text": text }))
            }

            RemoteAction::PressKey { selector, key } => {
                let element = desktop.desktop.locator(selector.as_str())
                    .first(Some(std::time::Duration::from_secs(5)))
                    .await
                    .context("Element not found")?;
                element.press_key(&key)?;
                Ok(serde_json::json!({ "key_pressed": key }))
            }

            RemoteAction::GetElementProperties { selector } => {
                let element = desktop.desktop.locator(selector.as_str())
                    .first(Some(std::time::Duration::from_secs(5)))
                    .await
                    .context("Element not found")?;

                let props = serde_json::json!({
                    "name": element.name().unwrap_or_default(),
                    "role": element.role(),
                    "is_enabled": element.is_enabled()?,
                    "is_visible": element.is_visible()?,
                    "bounds": element.bounds()?,
                    // value() method not available, using empty string
                    "value": "",
                });

                Ok(props)
            }

            RemoteAction::WaitForElement { selector, condition, timeout_ms } => {
                let timeout = std::time::Duration::from_millis(timeout_ms.unwrap_or(5000));
                let start = std::time::Instant::now();

                loop {
                    if start.elapsed() > timeout {
                        return Err(anyhow::anyhow!("Timeout waiting for element"));
                    }

                    if let Ok(element) = desktop.desktop.locator(selector.as_str())
                        .first(Some(std::time::Duration::from_millis(100))).await {
                        let met = match condition {
                            WaitCondition::Visible => element.is_visible()?,
                            WaitCondition::Enabled => element.is_enabled()?,
                            WaitCondition::Focused => element.is_focused()?,
                            WaitCondition::Exists => true,
                        };

                        if met {
                            return Ok(serde_json::json!({ "condition_met": true }));
                        }
                    }

                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }

            RemoteAction::TakeScreenshot { selector, full_page } => {
                let screenshot_data = if let Some(sel) = selector {
                    let element = desktop.desktop.locator(sel.as_str())
                        .first(Some(std::time::Duration::from_secs(5)))
                        .await
                        .context("Element not found")?;
                    // Using capture() method to get screenshot
                    let screenshot_result = element.capture()?;
                    screenshot_result.image_data
                } else {
                    // screenshot() method not available on Desktop
                    // Return empty screenshot for now
                    vec![]
                };

                let encoded = STANDARD.encode(&screenshot_data);
                Ok(serde_json::json!({
                    "screenshot": encoded,
                    "mime_type": "image/png"
                }))
            }

            RemoteAction::SetValue { selector, value } => {
                let element = desktop.desktop.locator(selector.as_str())
                    .first(Some(std::time::Duration::from_secs(5)))
                    .await
                    .context("Element not found")?;
                element.set_value(&value)?;
                Ok(serde_json::json!({ "value_set": value }))
            }

            RemoteAction::InvokeElement { selector } => {
                let element = desktop.desktop.locator(selector.as_str())
                    .first(Some(std::time::Duration::from_secs(5)))
                    .await
                    .context("Element not found")?;
                element.invoke()?;
                Ok(serde_json::json!({ "invoked": true }))
            }

            RemoteAction::ValidateElement { selector } => {
                match desktop.desktop.locator(selector.as_str())
                    .first(Some(std::time::Duration::from_secs(5))).await {
                    Ok(element) => {
                        let validation = serde_json::json!({
                            "exists": true,
                            "name": element.name().unwrap_or_default(),
                            "role": element.role(),
                            "is_enabled": element.is_enabled()?,
                            "is_visible": element.is_visible()?,
                        });
                        Ok(validation)
                    }
                    Err(_) => {
                        Ok(serde_json::json!({ "exists": false }))
                    }
                }
            }
        }
    }
}

async fn handle_request(
    State(state): State<RemoteServerState>,
    Query(auth): Query<AuthQuery>,
    Json(request): Json<RemoteRequest>,
) -> impl IntoResponse {
    if let Some(api_key) = &auth.api_key {
        if !validate_api_key(api_key) {
            return (
                StatusCode::UNAUTHORIZED,
                Json(RemoteResponse {
                    request_id: request.request_id.clone(),
                    success: false,
                    data: None,
                    error: Some("Invalid API key".to_string()),
                }),
            );
        }
    }

    info!("Received request: {:?}", request.action);

    match state.handle_action(request.action).await {
        Ok(data) => {
            (
                StatusCode::OK,
                Json(RemoteResponse {
                    request_id: request.request_id,
                    success: true,
                    data: Some(data),
                    error: None,
                }),
            )
        }
        Err(err) => {
            error!("Request failed: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(RemoteResponse {
                    request_id: request.request_id,
                    success: false,
                    data: None,
                    error: Some(err.to_string()),
                }),
            )
        }
    }
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "remote-ui-automation",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn list_sessions(
    State(state): State<RemoteServerState>,
) -> impl IntoResponse {
    let sessions = state.sessions.lock().await;
    Json(sessions.clone())
}

fn validate_api_key(api_key: &str) -> bool {
    std::env::var("REMOTE_API_KEY")
        .map(|expected| api_key == expected)
        .unwrap_or(true)
}

pub async fn start_remote_server(
    desktop: Arc<Mutex<DesktopWrapper>>,
    port: u16,
) -> Result<()> {
    let state = RemoteServerState::new(desktop);

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/execute", post(handle_request))
        .route("/sessions", get(list_sessions))
        .layer(
            tower_http::cors::CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                ])
                .allow_headers(tower_http::cors::Any),
        )
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    info!("Starting remote UI automation server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}