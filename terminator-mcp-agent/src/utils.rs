use anyhow::Result;
use rmcp::{schemars, schemars::JsonSchema};
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;
use terminator::Desktop;
use tracing::Level;
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
pub struct DesktopWrapper {
    pub desktop: Desktop,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetWindowTreeArgs {
    #[schemars(description = "Process ID of the target application")]
    pub pid: u32,
    #[schemars(description = "Optional window title filter")]
    pub title: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetWindowsArgs {
    #[schemars(description = "Name of the application to get windows for")]
    pub app_name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LocatorArgs {
    #[schemars(description = "An array of selector strings to locate the element")]
    pub selector_chain: Vec<String>,
    #[schemars(description = "Optional timeout in milliseconds for the action")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TypeIntoElementArgs {
    #[schemars(description = "An array of selector strings to locate the element")]
    pub selector_chain: Vec<String>,
    #[schemars(description = "The text to type into the element")]
    pub text_to_type: String,
    #[schemars(description = "Optional timeout in milliseconds for the action")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PressKeyArgs {
    #[schemars(description = "The key or key combination to press (e.g., 'Enter', 'Ctrl+A')")]
    pub key: String,
    #[schemars(description = "An array of selector strings to locate the element")]
    pub selector_chain: Vec<String>,
    #[schemars(description = "Optional timeout in milliseconds for the action")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RunCommandArgs {
    #[schemars(description = "The command to run on Windows")]
    pub windows_command: Option<String>,
    #[schemars(description = "The command to run on Linux/macOS")]
    pub unix_command: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EmptyArgs {}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetClipboardArgs {
    #[schemars(description = "Optional timeout in milliseconds")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MouseDragArgs {
    #[schemars(description = "An array of selector strings to locate the element")]
    pub selector_chain: Vec<String>,
    #[schemars(description = "Start X coordinate")]
    pub start_x: f64,
    #[schemars(description = "Start Y coordinate")]
    pub start_y: f64,
    #[schemars(description = "End X coordinate")]
    pub end_x: f64,
    #[schemars(description = "End Y coordinate")]
    pub end_y: f64,
    #[schemars(description = "Optional timeout in milliseconds")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ValidateElementArgs {
    #[schemars(description = "An array of selector strings to locate the element")]
    pub selector_chain: Vec<String>,
    #[schemars(description = "Optional timeout in milliseconds")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct HighlightElementArgs {
    #[schemars(description = "An array of selector strings to locate the element")]
    pub selector_chain: Vec<String>,
    #[schemars(description = "BGR color code (optional, default red)")]
    pub color: Option<u32>,
    #[schemars(description = "Duration in milliseconds (optional, default 1000ms)")]
    pub duration_ms: Option<u64>,
    #[schemars(description = "Optional timeout in milliseconds")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WaitForElementArgs {
    #[schemars(description = "An array of selector strings to locate the element")]
    pub selector_chain: Vec<String>,
    #[schemars(description = "Condition to wait for: 'visible', 'enabled', 'focused', 'exists'")]
    pub condition: String,
    #[schemars(description = "Optional timeout in milliseconds")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NavigateBrowserArgs {
    #[schemars(description = "URL to navigate to")]
    pub url: String,
    #[schemars(description = "Optional browser name")]
    pub browser: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct OpenApplicationArgs {
    #[schemars(description = "Name of the application to open")]
    pub app_name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ClipboardArgs {
    #[schemars(description = "Text to set to clipboard")]
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[schemars(description = "Arguments for scrolling an element")]
pub struct ScrollElementArgs {
    pub selector_chain: Vec<String>,
    #[serde(default)]
    #[schemars(description = "Direction to scroll: 'up', 'down', 'left', 'right'")]
    pub direction: String,
    #[serde(default)]
    #[schemars(description = "Amount to scroll (number of lines or pages)")]
    pub amount: f64,
    pub timeout_ms: Option<u64>,
}

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
