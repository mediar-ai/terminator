use anyhow::Result;
use rmcp::{schemars, schemars::JsonSchema};
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;
use terminator::{Desktop, UIElement};
use tracing::Level;
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
pub struct DesktopWrapper {
    pub desktop: Desktop,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct GetWindowTreeArgs {
    /// Process ID of the application.
    pub pid: u32,
    /// Optional window title to filter by.
    pub title: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct EmptyArgs {}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct GetWindowsArgs {
    /// Name of the application to get windows for.
    pub app_name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct LocatorArgs {
    /// An array of selector strings to locate the element.
    pub selector_chain: Vec<String>,
    /// Optional timeout in milliseconds for the action.
    pub timeout_ms: Option<u64>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct TypeIntoElementArgs {
    /// An array of selector strings to locate the element.
    pub selector_chain: Vec<String>,
    /// Optional timeout in milliseconds for the action.
    pub timeout_ms: Option<u64>,
    /// The text to type into the element.
    pub text_to_type: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct PressKeyArgs {
    /// An array of selector strings to locate the element.
    pub selector_chain: Vec<String>,
    /// Optional timeout in milliseconds for the action.
    pub timeout_ms: Option<u64>,
    /// The key or key combination to press (e.g., 'Enter', 'Ctrl+A').
    pub key: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct RunCommandArgs {
    /// The command to run on Windows.
    pub windows_command: Option<String>,
    /// The command to run on Linux/macOS.
    pub unix_command: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ExploreArgs {
    /// Optional selector chain to explore from a specific element.
    pub selector_chain: Option<Vec<String>>,
    /// Optional timeout in milliseconds for the action.
    pub timeout_ms: Option<u64>,
}

/// Response structure for exploration result
#[derive(Serialize)]
pub struct ExploredElementDetail {
    pub role: String,
    pub name: Option<String>, // Use 'name' consistently for the primary label/text
    pub id: Option<String>,
    pub bounds: Option<(f64, f64, f64, f64)>, // Include bounds for spatial context
    pub value: Option<String>,
    pub description: Option<String>,
    pub text: Option<String>,
    pub parent_id: Option<String>,
    pub children_ids: Vec<String>,
    pub suggested_selector: String,
}

#[derive(Serialize)]
pub struct ExploreResponse {
    pub parent: UIElement,                    // The parent element explored
    pub children: Vec<ExploredElementDetail>, // List of direct children details
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct CaptureScreenArgs {}

pub fn init_logging() -> Result<()> {
    let log_level = env::var("LOG_LEVEL")
        .map(|level| match level.to_lowercase().as_str() {
            "error" => Level::ERROR,
            "warn" => Level::WARN,
            "info" => Level::INFO,
            "debug" => Level::DEBUG,
            _ => Level::INFO,
        })
        .unwrap_or(Level::INFO);

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(log_level.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    Ok(())
}

pub fn get_timeout(timeout_ms: Option<u64>) -> Option<Duration> {
    timeout_ms.map(Duration::from_millis)
}
