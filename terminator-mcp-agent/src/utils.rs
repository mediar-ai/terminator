use crate::mcp_types::{FontStyle, TextPosition};
use anyhow::Result;
use rmcp::{schemars, schemars::JsonSchema};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::Duration;
use terminator::{AutomationError, Desktop, UIElement};
use tokio::sync::Mutex;
use tracing::{warn, Level};
use tracing_subscriber::EnvFilter;

// Validation helpers for better type safety
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct EmptyArgs {}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DelayArgs {
    #[schemars(description = "Number of milliseconds to delay")]
    pub delay_ms: u64,
}

fn default_desktop() -> Arc<Desktop> {
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    let desktop = Desktop::new(false, false).expect("Failed to create default desktop");
    #[cfg(target_os = "macos")]
    let desktop = Desktop::new(true, true).expect("Failed to create default desktop");
    Arc::new(desktop)
}

fn default_scroll_amount() -> f64 {
    3.0
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DesktopWrapper {
    #[serde(skip, default = "default_desktop")]
    pub desktop: Arc<Desktop>,
    #[serde(skip)]
    pub tool_router: rmcp::handler::server::tool::ToolRouter<Self>,
    #[serde(skip)]
    pub recorder: Arc<Mutex<Option<terminator_workflow_recorder::WorkflowRecorder>>>,
}

impl Default for DesktopWrapper {
    fn default() -> Self {
        // Can't use Default::default() because we need the tool_router from the macro
        // So we'll construct it properly in server.rs
        panic!("DesktopWrapper::default() should not be used directly. Use DesktopWrapper::new() instead.");
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetWindowTreeArgs {
    #[schemars(description = "Process ID of the target application")]
    pub pid: u32,
    #[schemars(description = "Optional window title filter")]
    pub title: Option<String>,
    #[schemars(
        description = "Whether to include detailed element attributes (enabled, focused, selected, etc.). Defaults to true for comprehensive LLM context."
    )]
    pub include_detailed_attributes: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetFocusedWindowTreeArgs {
    #[schemars(
        description = "Whether to include detailed element attributes (enabled, focused, selected, etc.). Defaults to true for comprehensive LLM context."
    )]
    pub include_detailed_attributes: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetApplicationsArgs {
    #[schemars(
        description = "Whether to include the full UI tree for each application. Defaults to false."
    )]
    pub include_tree: Option<bool>,
    #[schemars(
        description = "Whether to include detailed element attributes (enabled, focused, selected, etc.) when include_tree is true. Defaults to true for comprehensive LLM context."
    )]
    pub include_detailed_attributes: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LocatorArgs {
    #[schemars(
        description = "A string selector to locate the element. Can be chained with ` >> `."
    )]
    pub selector: String,
    #[schemars(
        description = "Optional alternative selectors to try in parallel. The first selector that finds an element will be used."
    )]
    pub alternative_selectors: Option<String>,
    #[schemars(
        description = "Optional fallback selectors to try sequentially if the primary selector fails.  These selectors are **only** attempted after the primary selector (and any parallel alternatives) time-out.  List can be comma-separated."
    )]
    pub fallback_selectors: Option<String>,
    #[schemars(description = "Optional timeout in milliseconds for the action")]
    pub timeout_ms: Option<u64>,
    #[schemars(description = "Whether to include full UI tree in the response.")]
    pub include_tree: Option<bool>,
    #[schemars(
        description = "Whether to include detailed element attributes (enabled, focused, selected, etc.) when include_tree is true. Defaults to true for comprehensive LLM context."
    )]
    pub include_detailed_attributes: Option<bool>,
    pub retries: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ClickElementArgs {
    #[schemars(
        description = "A string selector to locate the element. Can be chained with ` >> `."
    )]
    pub selector: String,
    #[schemars(
        description = "Optional alternative selectors to try in parallel. The first selector that finds an element will be used."
    )]
    pub alternative_selectors: Option<String>,
    #[schemars(
        description = "Optional fallback selectors to try sequentially if the primary selector fails.  These selectors are **only** attempted after the primary selector (and any parallel alternatives) time-out.  List can be comma-separated."
    )]
    pub fallback_selectors: Option<String>,
    #[schemars(description = "Optional timeout in milliseconds for the action")]
    pub timeout_ms: Option<u64>,
    #[schemars(description = "Whether to include full UI tree in the response. Defaults to true.")]
    pub include_tree: Option<bool>,
    #[schemars(
        description = "Whether to include detailed element attributes (enabled, focused, selected, etc.) when include_tree is true. Defaults to true for comprehensive LLM context."
    )]
    pub include_detailed_attributes: Option<bool>,
    pub retries: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TypeIntoElementArgs {
    #[schemars(
        description = "A string selector to locate the element. Can be chained with ` >> `."
    )]
    pub selector: String,
    #[schemars(
        description = "Optional alternative selectors to try in parallel. The first selector that finds an element will be used."
    )]
    pub alternative_selectors: Option<String>,
    #[schemars(
        description = "Optional fallback selectors to try sequentially if the primary selector fails.  These selectors are **only** attempted after the primary selector (and any parallel alternatives) time-out.  List can be comma-separated."
    )]
    pub fallback_selectors: Option<String>,
    #[schemars(description = "The text to type into the element")]
    pub text_to_type: String,
    #[schemars(description = "Optional timeout in milliseconds for the action (default: 3000ms)")]
    pub timeout_ms: Option<u64>,
    #[schemars(description = "Whether to verify the action succeeded (default: true)")]
    pub verify_action: Option<bool>,
    #[schemars(description = "Whether to clear the element before typing (default: true)")]
    pub clear_before_typing: Option<bool>,
    #[schemars(description = "Whether to include full UI tree in the response. Defaults to true.")]
    pub include_tree: Option<bool>,
    #[schemars(
        description = "Whether to include detailed element attributes (enabled, focused, selected, etc.) when include_tree is true. Defaults to true for comprehensive LLM context."
    )]
    pub include_detailed_attributes: Option<bool>,
    pub retries: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PressKeyArgs {
    #[schemars(description = "The key or key combination to press (e.g., 'Enter', 'Ctrl+A')")]
    pub key: String,
    #[schemars(
        description = "A string selector to locate the element. Can be chained with ` >> `."
    )]
    pub selector: String,
    #[schemars(
        description = "Optional alternative selectors to try in parallel. The first selector that finds an element will be used."
    )]
    pub alternative_selectors: Option<String>,
    #[schemars(
        description = "Optional fallback selectors to try sequentially if the primary selector fails.  These selectors are **only** attempted after the primary selector (and any parallel alternatives) time-out.  List can be comma-separated."
    )]
    pub fallback_selectors: Option<String>,
    #[schemars(description = "Optional timeout in milliseconds for the action")]
    pub timeout_ms: Option<u64>,
    #[schemars(description = "Whether to include full UI tree in the response. Defaults to true.")]
    pub include_tree: Option<bool>,
    #[schemars(
        description = "Whether to include detailed element attributes (enabled, focused, selected, etc.) when include_tree is true. Defaults to true for comprehensive LLM context."
    )]
    pub include_detailed_attributes: Option<bool>,
    pub retries: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GlobalKeyArgs {
    #[schemars(
        description = "The key or key combination to press (e.g., '{PageDown}', '{Ctrl}{V}')"
    )]
    pub key: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RunCommandArgs {
    #[schemars(description = "The command to run on Windows")]
    pub windows_command: Option<String>,
    #[schemars(description = "The command to run on Linux/macOS")]
    pub unix_command: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MouseDragArgs {
    #[schemars(
        description = "A string selector to locate the element. Can be chained with ` >> `."
    )]
    pub selector: String,
    #[schemars(description = "Start X coordinate")]
    pub start_x: f64,
    #[schemars(description = "Start Y coordinate")]
    pub start_y: f64,
    #[schemars(description = "End X coordinate")]
    pub end_x: f64,
    #[schemars(description = "End Y coordinate")]
    pub end_y: f64,
    #[schemars(
        description = "Optional alternative selectors to try in parallel. The first selector that finds an element will be used."
    )]
    pub alternative_selectors: Option<String>,
    #[schemars(
        description = "Optional fallback selectors to try sequentially if the primary selector fails.  These selectors are **only** attempted after the primary selector (and any parallel alternatives) time-out.  List can be comma-separated."
    )]
    pub fallback_selectors: Option<String>,
    #[schemars(description = "Optional timeout in milliseconds")]
    pub timeout_ms: Option<u64>,
    #[schemars(description = "Whether to include full UI tree in the response (verbose mode)")]
    pub include_tree: Option<bool>,
    #[schemars(
        description = "Whether to include detailed element attributes (enabled, focused, selected, etc.) when include_tree is true. Defaults to true for comprehensive LLM context."
    )]
    pub include_detailed_attributes: Option<bool>,
    pub retries: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ValidateElementArgs {
    #[schemars(
        description = "A string selector to locate the element. Can be chained with ` >> `."
    )]
    pub selector: String,
    #[schemars(
        description = "Optional alternative selectors to try in parallel. The first selector that finds an element will be used."
    )]
    pub alternative_selectors: Option<String>,
    #[schemars(
        description = "Optional fallback selectors to try sequentially if the primary selector fails.  These selectors are **only** attempted after the primary selector (and any parallel alternatives) time-out.  List can be comma-separated."
    )]
    pub fallback_selectors: Option<String>,
    #[schemars(description = "Optional timeout in milliseconds")]
    pub timeout_ms: Option<u64>,
    #[schemars(description = "Whether to include full UI tree in the response (verbose mode)")]
    pub include_tree: Option<bool>,
    #[schemars(
        description = "Whether to include detailed element attributes (enabled, focused, selected, etc.) when include_tree is true. Defaults to true for comprehensive LLM context."
    )]
    pub include_detailed_attributes: Option<bool>,
    pub retries: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct HighlightElementArgs {
    #[schemars(
        description = "A string selector to locate the element. Can be chained with ` >> `."
    )]
    pub selector: String,
    #[schemars(
        description = "Optional alternative selectors to try in parallel. The first selector that finds an element will be used."
    )]
    pub alternative_selectors: Option<String>,
    #[schemars(
        description = "Optional fallback selectors to try sequentially if the primary selector fails.  These selectors are **only** attempted after the primary selector (and any parallel alternatives) time-out.  List can be comma-separated."
    )]
    pub fallback_selectors: Option<String>,
    #[schemars(description = "BGR color code (optional, default red)")]
    pub color: Option<u32>,
    #[schemars(description = "Duration in milliseconds (optional, default 1000ms)")]
    pub duration_ms: Option<u64>,
    #[schemars(
        description = "Optional text to display as overlay. Text will be truncated to 10 characters."
    )]
    pub text: Option<String>,
    #[schemars(description = "Position of text overlay relative to the highlighted element")]
    pub text_position: Option<TextPosition>,
    #[schemars(description = "Font styling options for text overlay")]
    pub font_style: Option<FontStyle>,
    #[schemars(description = "Optional timeout in milliseconds")]
    pub timeout_ms: Option<u64>,
    #[schemars(description = "Whether to include full UI tree in the response (verbose mode)")]
    pub include_tree: Option<bool>,
    #[schemars(
        description = "Whether to include detailed element attributes (enabled, focused, selected, etc.) when include_tree is true. Defaults to true for comprehensive LLM context."
    )]
    pub include_detailed_attributes: Option<bool>,
    pub retries: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WaitForElementArgs {
    #[schemars(
        description = "A string selector to locate the element. Can be chained with ` >> `."
    )]
    pub selector: String,
    #[schemars(
        description = "Optional alternative selectors to try in parallel. The first selector that finds an element will be used."
    )]
    pub alternative_selectors: Option<String>,
    #[schemars(
        description = "Optional fallback selectors to try sequentially if the primary selector fails.  These selectors are **only** attempted after the primary selector (and any parallel alternatives) time-out.  List can be comma-separated."
    )]
    pub fallback_selectors: Option<String>,
    #[schemars(description = "Condition to wait for: 'visible', 'enabled', 'focused', 'exists'")]
    pub condition: String,
    #[schemars(description = "Optional timeout in milliseconds")]
    pub timeout_ms: Option<u64>,
    #[schemars(description = "Maximum depth for finding an ui element")]
    pub depth: Option<usize>,
    #[schemars(description = "Whether to include full UI tree in the response (verbose mode)")]
    pub include_tree: Option<bool>,
    #[schemars(
        description = "Whether to include detailed element attributes (enabled, focused, selected, etc.) when include_tree is true. Defaults to true for comprehensive LLM context."
    )]
    pub include_detailed_attributes: Option<bool>,
    pub retries: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NavigateBrowserArgs {
    #[schemars(description = "URL to navigate to")]
    pub url: String,
    #[schemars(description = "Optional browser name")]
    pub browser: Option<String>,
    #[schemars(description = "Whether to include full UI tree in the response (verbose mode)")]
    pub include_tree: Option<bool>,
    #[schemars(
        description = "Whether to include detailed element attributes (enabled, focused, selected, etc.) when include_tree is true. Defaults to true for comprehensive LLM context."
    )]
    pub include_detailed_attributes: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ExecuteBrowserScriptArgs {
    #[schemars(
        description = "A string selector to locate the browser element. Can be chained with ` >> `."
    )]
    pub selector: String,
    #[schemars(description = "The JavaScript code to execute in the browser console")]
    pub script: String,
    #[schemars(
        description = "Optional alternative selectors to try in parallel. The first selector that finds an element will be used."
    )]
    pub alternative_selectors: Option<String>,
    #[schemars(
        description = "Optional fallback selectors to try sequentially if the primary selector fails.  These selectors are **only** attempted after the primary selector (and any parallel alternatives) time-out.  List can be comma-separated."
    )]
    pub fallback_selectors: Option<String>,
    #[schemars(
        description = "Whether to include full UI tree in the response. Defaults to false."
    )]
    pub include_tree: Option<bool>,
    #[schemars(
        description = "Whether to include detailed element attributes (enabled, focused, selected, etc.) when include_tree is true. Defaults to true for comprehensive LLM context."
    )]
    pub include_detailed_attributes: Option<bool>,
    #[schemars(description = "Optional timeout in milliseconds for the action")]
    pub timeout_ms: Option<u64>,
    #[schemars(description = "Number of times to retry this step on failure.")]
    pub retries: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct OpenApplicationArgs {
    #[schemars(description = "Name of the application to open")]
    pub app_name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SelectOptionArgs {
    #[schemars(description = "A string selector to locate the dropdown/combobox element.")]
    pub selector: String,
    #[schemars(description = "The visible text of the option to select.")]
    pub option_name: String,
    #[schemars(description = "Optional alternative selectors.")]
    pub alternative_selectors: Option<String>,
    #[schemars(
        description = "Optional fallback selectors to try sequentially if the primary selector fails.  These selectors are **only** attempted after the primary selector (and any parallel alternatives) time-out.  List can be comma-separated."
    )]
    pub fallback_selectors: Option<String>,
    #[schemars(description = "Optional timeout in milliseconds.")]
    pub timeout_ms: Option<u64>,
    #[schemars(description = "Whether to include full UI tree in the response.")]
    pub include_tree: Option<bool>,
    #[schemars(
        description = "Whether to include detailed element attributes (enabled, focused, selected, etc.) when include_tree is true. Defaults to true for comprehensive LLM context."
    )]
    pub include_detailed_attributes: Option<bool>,
    pub retries: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SetToggledArgs {
    #[schemars(description = "A string selector to locate the toggleable element.")]
    pub selector: String,
    #[schemars(description = "The desired state: true for on, false for off.")]
    pub state: bool,
    #[schemars(description = "Optional alternative selectors.")]
    pub alternative_selectors: Option<String>,
    #[schemars(
        description = "Optional fallback selectors to try sequentially if the primary selector fails.  These selectors are **only** attempted after the primary selector (and any parallel alternatives) time-out.  List can be comma-separated."
    )]
    pub fallback_selectors: Option<String>,
    #[schemars(description = "Optional timeout in milliseconds.")]
    pub timeout_ms: Option<u64>,
    #[schemars(description = "Whether to include full UI tree in the response.")]
    pub include_tree: Option<bool>,
    #[schemars(
        description = "Whether to include detailed element attributes (enabled, focused, selected, etc.) when include_tree is true. Defaults to true for comprehensive LLM context."
    )]
    pub include_detailed_attributes: Option<bool>,
    pub retries: Option<u32>,
}

#[derive(Debug, serde::Deserialize, JsonSchema)]
pub struct MaximizeWindowArgs {
    pub selector: String,
    pub alternative_selectors: Option<String>,
    #[schemars(
        description = "Optional fallback selectors to try sequentially if the primary selector fails.  These selectors are **only** attempted after the primary selector (and any parallel alternatives) time-out.  List can be comma-separated."
    )]
    pub fallback_selectors: Option<String>,
    pub include_tree: Option<bool>,
    #[schemars(
        description = "Whether to include detailed element attributes (enabled, focused, selected, etc.) when include_tree is true. Defaults to true for comprehensive LLM context."
    )]
    pub include_detailed_attributes: Option<bool>,
    pub timeout_ms: Option<u64>,
    pub retries: Option<u32>,
}

#[derive(Debug, serde::Deserialize, JsonSchema)]
pub struct MinimizeWindowArgs {
    pub selector: String,
    pub alternative_selectors: Option<String>,
    #[schemars(
        description = "Optional fallback selectors to try sequentially if the primary selector fails.  These selectors are **only** attempted after the primary selector (and any parallel alternatives) time-out.  List can be comma-separated."
    )]
    pub fallback_selectors: Option<String>,
    pub include_tree: Option<bool>,
    #[schemars(
        description = "Whether to include detailed element attributes (enabled, focused, selected, etc.) when include_tree is true. Defaults to true for comprehensive LLM context."
    )]
    pub include_detailed_attributes: Option<bool>,
    pub timeout_ms: Option<u64>,
    pub retries: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SetRangeValueArgs {
    #[schemars(description = "A string selector to locate the range-based element.")]
    pub selector: String,
    #[schemars(description = "The numerical value to set.")]
    pub value: f64,
    #[schemars(description = "Optional alternative selectors.")]
    pub alternative_selectors: Option<String>,
    #[schemars(
        description = "Optional fallback selectors to try sequentially if the primary selector fails.  These selectors are **only** attempted after the primary selector (and any parallel alternatives) time-out.  List can be comma-separated."
    )]
    pub fallback_selectors: Option<String>,
    #[schemars(description = "Optional timeout in milliseconds.")]
    pub timeout_ms: Option<u64>,
    #[schemars(description = "Whether to include full UI tree in the response.")]
    pub include_tree: Option<bool>,
    #[schemars(
        description = "Whether to include detailed element attributes (enabled, focused, selected, etc.) when include_tree is true. Defaults to true for comprehensive LLM context."
    )]
    pub include_detailed_attributes: Option<bool>,
    pub retries: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SetValueArgs {
    #[schemars(description = "A string selector to locate the element whose value will be set.")]
    pub selector: String,
    #[schemars(description = "The text value to set.")]
    pub value: String,
    #[schemars(description = "Optional alternative selectors.")]
    pub alternative_selectors: Option<String>,
    #[schemars(
        description = "Optional fallback selectors to try sequentially if the primary selector fails.  These selectors are **only** attempted after the primary selector (and any parallel alternatives) time-out.  List can be comma-separated."
    )]
    pub fallback_selectors: Option<String>,
    #[schemars(description = "Optional timeout in milliseconds.")]
    pub timeout_ms: Option<u64>,
    #[schemars(description = "Whether to include full UI tree in the response.")]
    pub include_tree: Option<bool>,
    #[schemars(
        description = "Whether to include detailed element attributes (enabled, focused, selected, etc.) when include_tree is true. Defaults to true for comprehensive LLM context."
    )]
    pub include_detailed_attributes: Option<bool>,
    pub retries: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SetSelectedArgs {
    #[schemars(description = "A string selector to locate the selectable element.")]
    pub selector: String,
    #[schemars(description = "The desired state: true for selected, false for deselected.")]
    pub state: bool,
    #[schemars(description = "Optional alternative selectors.")]
    pub alternative_selectors: Option<String>,
    #[schemars(
        description = "Optional fallback selectors to try sequentially if the primary selector fails.  These selectors are **only** attempted after the primary selector (and any parallel alternatives) time-out.  List can be comma-separated."
    )]
    pub fallback_selectors: Option<String>,
    #[schemars(description = "Optional timeout in milliseconds.")]
    pub timeout_ms: Option<u64>,
    #[schemars(description = "Whether to include full UI tree in the response.")]
    pub include_tree: Option<bool>,
    #[schemars(
        description = "Whether to include detailed element attributes (enabled, focused, selected, etc.) when include_tree is true. Defaults to true for comprehensive LLM context."
    )]
    pub include_detailed_attributes: Option<bool>,
    pub retries: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[schemars(description = "Arguments for scrolling an element")]
pub struct ScrollElementArgs {
    pub selector: String,
    #[schemars(
        description = "Optional alternative selectors to try in parallel. The first selector that finds an element will be used."
    )]
    pub alternative_selectors: Option<String>,
    #[schemars(
        description = "Optional fallback selectors to try sequentially if the primary selector fails.  These selectors are **only** attempted after the primary selector (and any parallel alternatives) time-out.  List can be comma-separated."
    )]
    pub fallback_selectors: Option<String>,
    #[serde(default)]
    #[schemars(description = "Direction to scroll: 'up', 'down', 'left', 'right'")]
    pub direction: String,
    #[serde(default = "default_scroll_amount")]
    #[schemars(description = "Amount to scroll (number of lines or pages, default: 3)")]
    pub amount: f64,
    pub timeout_ms: Option<u64>,
    #[schemars(description = "Whether to include full UI tree in the response (verbose mode)")]
    pub include_tree: Option<bool>,
    #[schemars(
        description = "Whether to include detailed element attributes (enabled, focused, selected, etc.) when include_tree is true. Defaults to true for comprehensive LLM context."
    )]
    pub include_detailed_attributes: Option<bool>,
    pub retries: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ActivateElementArgs {
    #[schemars(
        description = "A string selector to locate the element. Can be chained with ` >> `."
    )]
    pub selector: String,
    #[schemars(
        description = "Optional fallback selectors to try sequentially if the primary selector fails.  These selectors are **only** attempted after the primary selector (and any parallel alternatives) time-out.  List can be comma-separated."
    )]
    pub fallback_selectors: Option<String>,
    #[schemars(description = "Optional timeout in milliseconds for the action")]
    pub timeout_ms: Option<u64>,
    #[schemars(description = "Whether to include full UI tree in the response. Defaults to true.")]
    pub include_tree: Option<bool>,
    #[schemars(
        description = "Whether to include detailed element attributes (enabled, focused, selected, etc.) when include_tree is true. Defaults to true for comprehensive LLM context."
    )]
    pub include_detailed_attributes: Option<bool>,
    pub retries: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone)]
pub struct ToolCall {
    #[schemars(description = "The name of the tool to be executed.")]
    pub tool_name: String,
    #[schemars(description = "The arguments for the tool, as a JSON object.")]
    pub arguments: serde_json::Value,
    #[schemars(
        description = "If true, the sequence will continue even if this tool call fails. Defaults to false."
    )]
    pub continue_on_error: Option<bool>,
    #[schemars(
        description = "An optional delay in milliseconds to wait after this tool call completes."
    )]
    pub delay_ms: Option<u64>,
    #[schemars(description = "Optional unique identifier for this step.")]
    pub id: Option<String>,
}

// Simplified structure for Gemini compatibility
#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone, Default)]
pub struct SequenceStep {
    #[schemars(description = "The name of the tool to execute (for single tool steps)")]
    pub tool_name: Option<String>,
    #[schemars(description = "The arguments for the tool (for single tool steps)")]
    pub arguments: Option<serde_json::Value>,
    #[schemars(description = "Continue on error flag (for single tool steps)")]
    pub continue_on_error: Option<bool>,
    #[schemars(description = "Delay after execution (for single tool steps)")]
    pub delay_ms: Option<u64>,
    #[schemars(description = "Group name (for grouped steps)")]
    pub group_name: Option<String>,
    #[schemars(description = "Steps in the group (for grouped steps)")]
    pub steps: Option<Vec<ToolCall>>,
    #[schemars(description = "Whether the group is skippable on error (for grouped steps)")]
    pub skippable: Option<bool>,
    #[serde(rename = "if", skip_serializing_if = "Option::is_none")]
    #[schemars(
        description = "An optional expression to determine if this step should run. e.g., \"policy.use_max_budget == true\" or \"contains(policy.product_types, 'FEX')\""
    )]
    pub r#if: Option<String>,
    #[schemars(description = "Number of times to retry this step or group on failure.")]
    pub retries: Option<u32>,
    #[schemars(
        description = "Optional unique identifier for this step (string). If provided, it can be a target for other steps' fallback_id."
    )]
    pub id: Option<String>,
    #[schemars(
        description = "Optional id of the step to jump to if this step ultimately fails after all retries. This enables robust fallback flows without relying on numeric indices."
    )]
    pub fallback_id: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default, JsonSchema)]
pub struct ExecuteSequenceArgs {
    #[schemars(description = "The steps of the workflow to execute in order.")]
    pub steps: Vec<SequenceStep>,
    #[schemars(
        description = "A key-value map defining the schema for dynamic variables (e.g., for UI generation)."
    )]
    pub variables: Option<HashMap<String, VariableDefinition>>,
    #[schemars(
        description = "A key-value map of the actual input values for the variables defined in the schema. **Must be an object**, not a string."
    )]
    pub inputs: Option<serde_json::Value>,
    #[schemars(
        description = "A key-value map of static UI element selectors for the workflow. **Must be an object with string values**, not a string. Example: {\"button\": \"role:Button|name:Submit\", \"field\": \"role:Edit|name:Email\"}"
    )]
    pub selectors: Option<serde_json::Value>,
    #[schemars(description = "Whether to stop the entire sequence on first error (default: true)")]
    pub stop_on_error: Option<bool>,
    #[schemars(
        description = "Whether to include detailed results from each tool execution (default: true)"
    )]
    pub include_detailed_results: Option<bool>,
    #[schemars(
        description = "An optional, structured parser to process the final tool output and extract structured data."
    )]
    pub output_parser: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone)]
#[serde(rename_all = "camelCase")]
pub enum VariableType {
    String,
    Number,
    Boolean,
    Enum,
    Array,
    Object,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone)]
pub struct VariableDefinition {
    #[schemars(description = "The data type of the variable.")]
    pub r#type: VariableType,
    #[schemars(description = "A user-friendly label for the variable, for UI generation.")]
    pub label: String,
    #[schemars(description = "A detailed description of what the variable is for.")]
    pub description: Option<String>,
    #[schemars(description = "The default value for the variable if not provided in the inputs.")]
    pub default: Option<serde_json::Value>,
    #[schemars(description = "For string types, a regex pattern for validation.")]
    pub regex: Option<String>,
    #[schemars(description = "For enum types, a list of allowed string values.")]
    pub options: Option<Vec<String>>,
    #[schemars(description = "Whether this variable is required. Defaults to true.")]
    pub required: Option<bool>,
}

// Keep the old structures for internal use
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolGroup {
    pub group_name: String,
    pub steps: Vec<ToolCall>,
    pub skippable: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SequenceItem {
    Tool { tool_call: ToolCall },
    Group { tool_group: ToolGroup },
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CloseElementArgs {
    pub selector: String,
    pub alternative_selectors: Option<String>,
    #[schemars(
        description = "Optional fallback selectors to try sequentially if the primary selector fails.  These selectors are **only** attempted after the primary selector (and any parallel alternatives) time-out.  List can be comma-separated."
    )]
    pub fallback_selectors: Option<String>,
    pub timeout_ms: Option<u64>,
    pub include_tree: Option<bool>,
    #[schemars(
        description = "Whether to include detailed element attributes (enabled, focused, selected, etc.) when include_tree is true. Defaults to true for comprehensive LLM context."
    )]
    pub include_detailed_attributes: Option<bool>,
    pub retries: Option<u32>,
}

#[derive(Deserialize, JsonSchema, Debug, Clone)]
pub struct ZoomArgs {
    pub level: u32,
}

#[derive(Deserialize, JsonSchema, Debug, Clone)]
pub struct SetZoomArgs {
    #[schemars(
        description = "The zoom percentage to set (e.g., 100 for 100%, 150 for 150%, 50 for 50%)"
    )]
    pub percentage: u32,
    #[schemars(description = "Whether to include full UI tree in the response (verbose mode)")]
    pub include_tree: Option<bool>,
    #[schemars(
        description = "Whether to include detailed element attributes (enabled, focused, selected, etc.) when include_tree is true. Defaults to true for comprehensive LLM context."
    )]
    pub include_detailed_attributes: Option<bool>,
}

#[derive(Debug)]
pub struct ValidationError {
    pub field: String,
    pub expected: String,
    pub actual: String,
}

impl ValidationError {
    pub fn new(field: &str, expected: &str, actual: &str) -> Self {
        Self {
            field: field.to_string(),
            expected: expected.to_string(),
            actual: actual.to_string(),
        }
    }
}

pub fn validate_inputs(inputs: &serde_json::Value) -> Result<(), ValidationError> {
    if !inputs.is_object() {
        return Err(ValidationError::new(
            "inputs",
            "object",
            &format!("{inputs:?}"),
        ));
    }
    Ok(())
}

pub fn validate_selectors(selectors: &serde_json::Value) -> Result<(), ValidationError> {
    match selectors {
        serde_json::Value::Object(obj) => {
            // Check that all values are strings
            for (key, value) in obj {
                if !value.is_string() {
                    return Err(ValidationError::new(
                        &format!("selectors.{key}"),
                        "string",
                        match value {
                            serde_json::Value::Number(_) => "number",
                            serde_json::Value::Bool(_) => "boolean",
                            serde_json::Value::Array(_) => "array",
                            serde_json::Value::Object(_) => "object",
                            serde_json::Value::Null => "null",
                            _ => "unknown",
                        },
                    ));
                }
            }
            Ok(())
        }
        serde_json::Value::String(s) => {
            // Try to parse as JSON object first
            match serde_json::from_str::<serde_json::Value>(s) {
                Ok(parsed) => validate_selectors(&parsed),
                Err(_) => Err(ValidationError::new(
                    "selectors",
                    "object or valid JSON string",
                    "invalid JSON string",
                )),
            }
        }
        _ => Err(ValidationError::new(
            "selectors",
            "object or JSON string",
            &format!("{selectors:?}"),
        )),
    }
}

pub fn validate_output_parser(parser: &serde_json::Value) -> Result<(), ValidationError> {
    let obj = parser
        .as_object()
        .ok_or_else(|| ValidationError::new("output_parser", "object", &format!("{parser:?}")))?;

    // Check required fields
    if !obj.contains_key("uiTreeJsonPath") {
        return Err(ValidationError::new(
            "output_parser.uiTreeJsonPath",
            "string",
            "missing",
        ));
    }

    if !obj.contains_key("itemContainerDefinition") {
        return Err(ValidationError::new(
            "output_parser.itemContainerDefinition",
            "object",
            "missing",
        ));
    }

    if !obj.contains_key("fieldsToExtract") {
        return Err(ValidationError::new(
            "output_parser.fieldsToExtract",
            "object",
            "missing",
        ));
    }

    Ok(())
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RunJavascriptArgs {
    #[schemars(
        description = "JavaScript source code to execute inside the embedded engine. Either this or script_file_path must be provided."
    )]
    pub script: Option<String>,
    #[schemars(
        description = "Path to a JavaScript file to execute. Either this or script must be provided."
    )]
    pub script_file_path: Option<String>,
    #[schemars(
        description = "Optional timeout in milliseconds before the script execution is aborted."
    )]
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
    // Default to 3 seconds instead of indefinite wait to prevent hanging
    let timeout = timeout_ms.unwrap_or(3000);
    Some(Duration::from_millis(timeout))
}

/// Try multiple selectors with primary selector priority
/// The primary selector is always preferred if it succeeds, even if alternatives also succeed
pub async fn find_element_with_fallbacks(
    desktop: &Desktop,
    primary_selector: &str,
    alternative_selectors: Option<&str>,
    fallback_selectors: Option<&str>,
    timeout_ms: Option<u64>,
) -> Result<(terminator::UIElement, String), terminator::AutomationError> {
    use tokio::time::Duration;

    let timeout_duration = get_timeout(timeout_ms).unwrap_or(Duration::from_millis(3000));

    // FAST PATH: If no alternatives or fallbacks are provided, just use the primary selector directly.
    if alternative_selectors.is_none() && fallback_selectors.is_none() {
        let locator = desktop.locator(terminator::Selector::from(primary_selector));
        return match locator.first(Some(timeout_duration), Some(50_usize)).await {    // default depth
            Ok(element) => Ok((element, primary_selector.to_string())),
            Err(e) => Err(terminator::AutomationError::ElementNotFound(format!(
                "Primary selector '{primary_selector}' failed: {e}"
            ))),
        };
    }

    // Parse comma-separated alternative selectors
    let alternative_selectors_vec: Option<Vec<String>> = alternative_selectors.map(|alts| {
        alts.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    });

    // Parse comma-separated fallback selectors
    let fallback_selectors_vec: Option<Vec<String>> = fallback_selectors.map(|alts| {
        alts.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    });

    // Create primary task
    let desktop_clone = desktop.clone();
    let primary_clone = primary_selector.to_string();
    let primary_task = tokio::spawn(async move {
        let locator = desktop_clone.locator(terminator::Selector::from(primary_clone.as_str()));
        match locator.first(Some(timeout_duration), Some(50_usize)).await {
            Ok(element) => Ok((element, primary_clone)),
            Err(e) => Err((primary_clone, e)),
        }
    });

    // Create alternative tasks
    let mut alternative_tasks = Vec::new();
    if let Some(alternatives) = alternative_selectors_vec.as_ref() {
        for selector_str in alternatives {
            let desktop_clone = desktop.clone();
            let selector_clone = selector_str.clone();
            let task = tokio::spawn(async move {
                let locator =
                    desktop_clone.locator(terminator::Selector::from(selector_clone.as_str()));
                match locator.first(Some(timeout_duration), Some(50_usize)).await {
                    Ok(element) => Ok((element, selector_clone)),
                    Err(e) => Err((selector_clone, e)),
                }
            });
            alternative_tasks.push(task);
        }
    }

    // Wait for primary task first, then alternatives
    let mut errors = Vec::new();
    let mut completed_tasks = Vec::new();
    completed_tasks.push(primary_task);
    completed_tasks.extend(alternative_tasks);

    // Use select_all but prioritize primary selector if multiple succeed
    while !completed_tasks.is_empty() {
        let (result, index, remaining_tasks) = futures::future::select_all(completed_tasks).await;

        match result {
            Ok(Ok((element, selector))) => {
                // Cancel remaining tasks
                for task in remaining_tasks {
                    task.abort();
                }

                // Always prefer primary selector (index 0) if it succeeds
                if index == 0 {
                    return Ok((element, selector));
                } else {
                    // Alternative succeeded first, but give primary selector a brief grace period
                    // in case it's about to succeed too (within 10ms)
                    let desktop_clone = desktop.clone();
                    let primary_clone = primary_selector.to_string();

                    match tokio::time::timeout(Duration::from_millis(10), async move {
                        let locator = desktop_clone
                            .locator(terminator::Selector::from(primary_clone.as_str()));
                        locator.first(Some(Duration::from_millis(1)), Some(50_usize)).await
                    })
                    .await
                    {
                        Ok(Ok(primary_element)) => {
                            // Primary also succeeded within grace period - prefer it
                            return Ok((primary_element, primary_selector.to_string()));
                        }
                        _ => {
                            // Primary didn't succeed quickly, use the alternative that worked
                            return Ok((element, selector));
                        }
                    }
                }
            }
            Ok(Err((selector, error))) => {
                errors.push(format!("'{selector}': {error}"));
                completed_tasks = remaining_tasks;
            }
            Err(join_error) => {
                errors.push(format!("Task error: {join_error}"));
                completed_tasks = remaining_tasks;
            }
        }
    }

    // If we reach here, primary and alternative selectors failed. Try fallback selectors sequentially.
    if let Some(fallbacks) = fallback_selectors_vec {
        for fb_selector in fallbacks {
            let locator = desktop.locator(terminator::Selector::from(fb_selector.as_str()));
            match locator.first(Some(timeout_duration), Some(50_usize)).await {
                Ok(element) => {
                    return Ok((element, fb_selector));
                }
                Err(e) => {
                    errors.push(format!("'{fb_selector}': {e}"));
                }
            }
        }
    }

    // All selectors (primary, alternatives, fallbacks) failed
    let combined_error = if errors.is_empty() {
        "No selectors provided".to_string()
    } else {
        format!(
            "All {} selectors failed: [{}]",
            errors.len(),
            errors.join(", ")
        )
    };

    Err(terminator::AutomationError::ElementNotFound(combined_error))
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ExportWorkflowSequenceArgs {
    #[schemars(description = "Absolute path to the workflow file to create or edit")]
    pub file_path: String,

    #[schemars(description = "Text content to add to the workflow file")]
    pub content: String,

    #[schemars(
        description = "Text pattern to find and replace (optional - if not provided, content will be appended)"
    )]
    pub find_pattern: Option<String>,

    #[schemars(description = "Whether to use regex for pattern matching (default: false)")]
    pub use_regex: Option<bool>,

    #[schemars(description = "Create new file if it doesn't exist (default: true)")]
    pub create_if_missing: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ImportWorkflowSequenceArgs {
    #[schemars(description = "Path to specific YAML workflow file to load")]
    pub file_path: Option<String>,

    #[schemars(description = "Path to folder to scan for YAML workflow files")]
    pub folder_path: Option<String>,
}

/// A robust helper that finds a UI element and executes a provided action on it,
/// with built-in retry logic for both finding the element and performing the action.
///
/// This function is the standard way to interact with elements when reliability is key.
///
/// # Arguments
/// * `desktop` - The active `Desktop` instance.
/// * `primary_selector` - The main selector for the target element.
/// * `alternatives` - A comma-separated string of fallback selectors.
/// * `timeout_ms` - The timeout for the initial element search.
/// * `retries` - The number of times to retry the *entire find-and-act sequence*.
/// * `action` - An async closure that takes the found `UIElement` and performs an action,
///
/// # Returns
/// A `Result` containing a tuple of the action's return value `T` and the `UIElement` on
/// which the action was successfully performed.
pub async fn find_and_execute_with_retry<F, Fut, T>(
    desktop: &Desktop,
    primary_selector: &str,
    alternatives: Option<&str>,
    timeout_ms: Option<u64>,
    retries: Option<u32>,
    action: F,
) -> Result<((T, UIElement), String), anyhow::Error>
where
    F: Fn(UIElement) -> Fut,
    Fut: std::future::Future<Output = Result<T, AutomationError>>,
{
    let retry_count = retries.unwrap_or(0);
    let mut last_error: Option<anyhow::Error> = None;

    for attempt in 0..=retry_count {
        match find_element_with_fallbacks(desktop, primary_selector, alternatives, None, timeout_ms)
            .await
        {
            Ok((element, successful_selector)) => match action(element.clone()).await {
                Ok(result) => return Ok(((result, element), successful_selector)),
                Err(e) => {
                    last_error = Some(e.into());
                    if attempt < retry_count {
                        warn!(
                            "Action failed on attempt {}/{}. Retrying... Error: {}",
                            attempt + 1,
                            retry_count + 1,
                            last_error.as_ref().unwrap()
                        );
                        tokio::time::sleep(Duration::from_millis(250)).await; // Wait before next retry
                    }
                }
            },
            Err(e) => {
                last_error = Some(e.into());
                if attempt < retry_count {
                    warn!(
                        "Find element failed on attempt {}/{}. Retrying... Error: {}",
                        attempt + 1,
                        retry_count + 1,
                        last_error.as_ref().unwrap()
                    );
                    // No need to sleep here, as find_element_with_fallbacks already has a timeout.
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        anyhow::anyhow!(
            "Action failed after {} retries for selector '{}'",
            retry_count + 1,
            primary_selector
        )
    }))
}

/// New helper that exposes fallback selectors as an argument. Internal implementation is shared.
pub async fn find_and_execute_with_retry_with_fallback<F, Fut, T>(
    desktop: &Desktop,
    primary_selector: &str,
    alternatives: Option<&str>,
    fallback_selectors: Option<&str>,
    timeout_ms: Option<u64>,
    retries: Option<u32>,
    action: F,
) -> Result<((T, UIElement), String), anyhow::Error>
where
    F: Fn(UIElement) -> Fut,
    Fut: std::future::Future<Output = Result<T, AutomationError>>,
{
    let retry_count = retries.unwrap_or(0);
    let mut last_error: Option<anyhow::Error> = None;

    for attempt in 0..=retry_count {
        match find_element_with_fallbacks(
            desktop,
            primary_selector,
            alternatives,
            fallback_selectors,
            timeout_ms,
        )
        .await
        {
            Ok((element, successful_selector)) => match action(element.clone()).await {
                Ok(result) => return Ok(((result, element), successful_selector)),
                Err(e) => {
                    last_error = Some(e.into());
                    if attempt < retry_count {
                        warn!(
                            "Action failed on attempt {}/{}. Retrying... Error: {}",
                            attempt + 1,
                            retry_count + 1,
                            last_error.as_ref().unwrap()
                        );
                        tokio::time::sleep(Duration::from_millis(250)).await; // Wait before next retry
                    }
                }
            },
            Err(e) => {
                last_error = Some(e.into());
                if attempt < retry_count {
                    warn!(
                        "Find element failed on attempt {}/{}. Retrying... Error: {}",
                        attempt + 1,
                        retry_count + 1,
                        last_error.as_ref().unwrap()
                    );
                    // No need to sleep here, as find_element_with_fallbacks already has a timeout.
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        anyhow::anyhow!(
            "Action failed after {} retries for selector '{}'",
            retry_count + 1,
            primary_selector
        )
    }))
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RecordWorkflowArgs {
    /// The action to perform: 'start' to begin recording, 'stop' to end and save.
    pub action: String,
    /// A descriptive name for the workflow being recorded. Required when starting.
    pub workflow_name: Option<String>,
    /// Optional file path to save the workflow. If not provided, a default path will be used.
    pub file_path: Option<String>,
    /// Sets the recording to a low-energy mode to reduce system load, which can help prevent lag on less powerful machines.
    pub low_energy_mode: Option<bool>,
}
