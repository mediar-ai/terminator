use crate::helpers::*;
use crate::scripting_engine;
use crate::telemetry::StepSpan;
use crate::utils::find_and_execute_with_retry_with_fallback;
pub use crate::utils::DesktopWrapper;
use crate::utils::{
    get_timeout, ActivateElementArgs, CaptureElementScreenshotArgs, ClickElementArgs, DelayArgs,
    ExecuteBrowserScriptArgs, ExecuteSequenceArgs,
    GetApplicationsArgs, GetWindowTreeArgs, GlobalKeyArgs, HighlightElementArgs, InvokeElementArgs,
    LocatorArgs, MaximizeWindowArgs, MinimizeWindowArgs, MouseDragArgs, NavigateBrowserArgs,
    OpenApplicationArgs, PressKeyArgs, RunCommandArgs, ScrollElementArgs,
    SetRangeValueArgs, SetSelectedArgs, SetToggledArgs, SetValueArgs, SetZoomArgs,
    StopHighlightingArgs, TypeIntoElementArgs, ValidateElementArgs, WaitForElementArgs,
};
use image::imageops::FilterType;
use image::{ExtendedColorType, ImageBuffer, ImageEncoder, Rgba};
use regex::Regex;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
};
use rmcp::{tool, ErrorData as McpError, ServerHandler};
use rmcp::{tool_handler, tool_router};
use serde_json::json;
use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use sysinfo::{ProcessesToUpdate, System};
use terminator::element::UIElementImpl;
use terminator::{AutomationError, Browser, Desktop, Selector, UIElement};
use tokio::sync::Mutex;
use tracing::{info, warn, Instrument};

// New imports for image encoding
use base64::{engine::general_purpose, Engine as _};
use image::codecs::png::PngEncoder;

use rmcp::service::{Peer, RequestContext, RoleServer};

/// Extracts JSON data from Content objects without double serialization
pub fn extract_content_json(content: &Content) -> Result<serde_json::Value, serde_json::Error> {
    // Handle the new rmcp 0.4.0 Content structure with Annotated<RawContent>
    match &content.raw {
        rmcp::model::RawContent::Text(text_content) => {
            // Try to parse the text as JSON first
            if let Ok(parsed_json) = serde_json::from_str::<serde_json::Value>(&text_content.text) {
                Ok(parsed_json)
            } else {
                // If it's not JSON, return as a text object
                Ok(json!({"type": "text", "text": text_content.text}))
            }
        }
        rmcp::model::RawContent::Image(image_content) => Ok(
            json!({"type": "image", "data": image_content.data, "mime_type": image_content.mime_type}),
        ),
        rmcp::model::RawContent::Resource(resource_content) => {
            Ok(json!({"type": "resource", "resource": resource_content}))
        }
        rmcp::model::RawContent::Audio(audio_content) => Ok(
            json!({"type": "audio", "data": audio_content.data, "mime_type": audio_content.mime_type}),
        ),
        rmcp::model::RawContent::ResourceLink(resource_link) => {
            Ok(json!({"type": "resource_link", "resource": resource_link}))
        }
    }
}

/// Capture screenshots of all monitors and return them as MCP Content objects
async fn capture_monitor_screenshots(desktop: &Desktop) -> Vec<Content> {
    let mut contents = Vec::new();

    match desktop.capture_all_monitors().await {
        Ok(screenshots) => {
            for (monitor, screenshot) in screenshots {
                // Convert RGBA bytes to PNG
                match rgba_to_png(&screenshot.image_data, screenshot.width, screenshot.height) {
                    Ok(png_data) => {
                        // Base64 encode the PNG
                        let base64_data = general_purpose::STANDARD.encode(&png_data);

                        // Use the Content::image helper method
                        contents.push(Content::image(base64_data, "image/png".to_string()));

                        info!(
                            "Captured monitor '{}' screenshot: {}x{} ({}KB)",
                            monitor.name,
                            screenshot.width,
                            screenshot.height,
                            png_data.len() / 1024
                        );
                    }
                    Err(e) => {
                        warn!(
                            "Failed to convert monitor '{}' screenshot to PNG: {}",
                            monitor.name, e
                        );
                    }
                }
            }
        }
        Err(e) => {
            warn!("Failed to capture monitor screenshots: {}", e);
        }
    }

    contents
}

/// Convert RGBA image data to PNG format
fn rgba_to_png(
    rgba_data: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut png_data = Vec::new();
    let mut cursor = Cursor::new(&mut png_data);

    let encoder = PngEncoder::new(&mut cursor);
    encoder.write_image(rgba_data, width, height, ExtendedColorType::Rgba8)?;

    Ok(png_data)
}

/// Helper to conditionally append monitor screenshots to existing content
/// Only captures screenshots if include is true (defaults to false)
async fn append_monitor_screenshots_if_enabled(
    desktop: &Desktop,
    mut contents: Vec<Content>,
    include: Option<bool>,
) -> Vec<Content> {
    // Only capture if explicitly enabled (defaults to false)
    if include.unwrap_or(false) {
        let mut screenshots = capture_monitor_screenshots(desktop).await;
        contents.append(&mut screenshots);
    }
    contents
}

#[tool_router]
impl DesktopWrapper {
    /// Check if a string is a valid JavaScript identifier and not a reserved word
    fn is_valid_js_identifier(name: &str) -> bool {
        // Reserved words and globals we don't want to override
        const RESERVED: &[&str] = &[
            "env",
            "variables",
            "desktop",
            "console",
            "log",
            "sleep",
            "require",
            "process",
            "global",
            "window",
            "document",
            "alert",
            "prompt",
            "undefined",
            "null",
            "true",
            "false",
            "NaN",
            "Infinity",
            "var",
            "let",
            "const",
            "function",
            "return",
            "if",
            "else",
            "for",
            "while",
            "do",
            "switch",
            "case",
            "break",
            "continue",
            "throw",
            "try",
            "catch",
            "finally",
            "new",
            "delete",
            "typeof",
            "instanceof",
            "in",
            "of",
            "this",
            "super",
            "class",
            "extends",
            "static",
            "async",
            "await",
            "yield",
            "import",
            "export",
        ];

        if RESERVED.contains(&name) {
            return false;
        }

        // Check if it's a valid identifier: starts with letter/underscore/$,
        // continues with letters/digits/underscore/$
        if name.is_empty() {
            return false;
        }

        let mut chars = name.chars();
        let first = chars.next().unwrap();
        if !first.is_alphabetic() && first != '_' && first != '$' {
            return false;
        }

        chars.all(|c| c.is_alphanumeric() || c == '_' || c == '$')
    }

    /// Prepare window management before tool execution
    /// Handles window cache update, state capture, minimize all, and maximize target
    async fn prepare_window_management(
        &self,
        process: &str,
        execution_context: Option<&crate::utils::ToolExecutionContext>,
        process_id: Option<u32>,
        ui_element: Option<&terminator::platforms::windows::WindowsUIElement>,
        window_mgmt_opts: &crate::utils::WindowManagementOptions,
    ) -> Result<(), String> {
        let start = std::time::Instant::now();

        // Check if window management is enabled (defaults to true for backward compatibility)
        let enabled = window_mgmt_opts.enable_window_management.unwrap_or(true);
        if !enabled {
            tracing::debug!("Window management disabled by user, skipping");
            return Ok(());
        }

        // Update window cache on-demand before managing windows
        // Initial state is captured once before sequence starts (in server_sequence.rs)
        if let Err(e) = self.window_manager.update_window_cache().await {
            tracing::warn!("Failed to update window cache: {}", e);
        }

        // Handle execution context-aware window management
        if let Some(ctx) = execution_context {
            // Detect process switch
            let process_switched = if let Some(ref prev_process) = ctx.previous_process {
                prev_process != process
            } else {
                false
            };

            if process_switched {
                tracing::info!(
                    "Process switched from '{}' to '{}' at step {}",
                    ctx.previous_process.as_ref().unwrap(),
                    process,
                    ctx.current_step
                );

                // Minimize the previous process window
                if let Some(prev_window) = self
                    .window_manager
                    .get_topmost_window_for_process(ctx.previous_process.as_ref().unwrap())
                    .await
                {
                    match self
                        .window_manager
                        .minimize_if_needed(prev_window.hwnd)
                        .await
                    {
                        Ok(true) => {
                            tracing::info!("Minimized previous process window");
                        }
                        Ok(false) => {
                            tracing::debug!("Previous process window already minimized");
                        }
                        Err(e) => {
                            tracing::warn!("Failed to minimize previous process window: {}", e);
                        }
                    }
                }
            }

            // Initial state is now captured before sequence starts (in server_sequence.rs)
            // No need to capture here anymore

            // Get topmost window for the target process
            if let Some(window) = self
                .window_manager
                .get_topmost_window_for_process(process)
                .await
            {
                tracing::info!(
                    "Managing windows for process '{}' (hwnd: {}, step {}/{})",
                    process,
                    window.hwnd,
                    ctx.current_step,
                    ctx.total_steps
                );

                // Only minimize always-on-top windows (only on first UI step or non-sequence)
                // Maximizing brings the target to front naturally, so we only need to handle
                // always-on-top windows that would otherwise cover it
                // First UI tool in sequence has no previous_process
                let should_minimize_always_on_top =
                    window_mgmt_opts.minimize_always_on_top.unwrap_or(true);
                if should_minimize_always_on_top
                    && (ctx.previous_process.is_none() || !ctx.in_sequence)
                {
                    let always_on_top_windows =
                        self.window_manager.get_always_on_top_windows().await;
                    if !always_on_top_windows.is_empty() {
                        tracing::info!(
                            "Found {} always-on-top windows that may cover target",
                            always_on_top_windows.len()
                        );
                        match self
                            .window_manager
                            .minimize_always_on_top_windows(window.hwnd)
                            .await
                        {
                            Ok(count) => {
                                tracing::info!("Minimized {} always-on-top windows", count);
                            }
                            Err(e) => {
                                tracing::warn!("Failed to minimize always-on-top windows: {}", e);
                            }
                        }
                    } else {
                        tracing::debug!("No always-on-top windows found, skipping minimization (maximize will bring target to front)");
                    }
                }

                // Maximize target if not already maximized
                let should_maximize_target = window_mgmt_opts.maximize_target.unwrap_or(false);
                let should_bring_to_front = window_mgmt_opts.bring_to_front.unwrap_or(true);

                if should_maximize_target {
                    match self.window_manager.maximize_if_needed(window.hwnd).await {
                        Ok(true) => {
                            tracing::info!("Maximized target window");
                        }
                        Ok(false) => {
                            tracing::debug!("Target window already maximized");
                        }
                        Err(e) => {
                            tracing::warn!("Failed to maximize window: {}", e);
                        }
                    }
                }

                // Bring window to front (independent of maximize)
                if should_bring_to_front {
                    match self.window_manager.bring_window_to_front(window.hwnd).await {
                        Ok(true) => {
                            tracing::info!("Brought target window to front");
                        }
                        Ok(false) => {
                            tracing::debug!(
                                "Failed to bring window to front (Windows restrictions)"
                            );
                        }
                        Err(e) => {
                            tracing::warn!("Failed to bring window to front: {}", e);
                        }
                    }
                }
            } else {
                tracing::warn!(
                    "Could not find window for process '{}' for window management",
                    process
                );
            }
        } else {
            // Simple mode: no execution context (direct MCP tool calls)
            // Always capture initial state, minimize all, maximize target

            if let Err(e) = self.window_manager.capture_initial_state().await {
                tracing::warn!("Failed to capture initial window state: {}", e);
            }

            // Check if this is a UWP app (requires process_id)
            let is_uwp = if let Some(pid) = process_id {
                self.window_manager.is_uwp_app(pid).await
            } else {
                false
            };

            if is_uwp {
                tracing::info!("[prepare_window_management] Detected UWP app - using keyboard (Win+Up) for window management");

                // Get UWP window's HWND for tracking and restoration
                // For UWP apps, we need the ApplicationFrameHost parent HWND, not the content window HWND
                let uwp_hwnd = if let Some(element) = ui_element {
                    match element.get_native_window_handle() {
                        Ok(content_hwnd) => {
                            tracing::info!(
                                "[prepare_window_management] Got UWP content window HWND: {}",
                                content_hwnd
                            );

                            // Get the parent ApplicationFrameHost window
                            use windows::Win32::Foundation::HWND;
                            use windows::Win32::UI::WindowsAndMessaging::GetAncestor;
                            use windows::Win32::UI::WindowsAndMessaging::GA_ROOT;

                            unsafe {
                                let hwnd = HWND(content_hwnd as *mut _);
                                let root_hwnd = GetAncestor(hwnd, GA_ROOT);
                                if !root_hwnd.0.is_null() {
                                    let root_hwnd_val = root_hwnd.0 as isize;
                                    tracing::info!(
                                        "[prepare_window_management] Got UWP root window HWND: {}",
                                        root_hwnd_val
                                    );
                                    Some(root_hwnd_val)
                                } else {
                                    tracing::warn!("[prepare_window_management] Failed to get root window for UWP content");
                                    Some(content_hwnd)
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                "[prepare_window_management] Failed to get UWP HWND: {}",
                                e
                            );
                            None
                        }
                    }
                } else {
                    tracing::warn!(
                        "[prepare_window_management] No ui_element provided for UWP app"
                    );
                    None
                };

                // 1. Track UWP window as target for restoration
                if let Some(hwnd) = uwp_hwnd {
                    self.window_manager.set_target_window(hwnd).await;
                } else {
                    tracing::warn!("[prepare_window_management] Cannot track UWP window for restoration (no HWND)");
                }

                // 2. Maximize UWP target window using keyboard (ShowWindow doesn't work for UWP)
                let should_maximize_target = window_mgmt_opts.maximize_target.unwrap_or(false);
                if should_maximize_target {
                    if let Some(element) = ui_element {
                        if let Err(e) = element.maximize_window_keyboard() {
                            tracing::warn!(
                                "Failed to maximize UWP window via keyboard for {}: {}",
                                process,
                                e
                            );
                        } else {
                            tracing::info!(
                                "Maximized UWP window for {} via keyboard (Win+Up)",
                                process
                            );
                        }
                    } else {
                        tracing::warn!("Cannot maximize UWP window - no ui_element provided");
                    }
                }

                // 3. Minimize Win32 always-on-top windows (if any)
                // Note: UWP always-on-top windows are not visible via Win32 enumeration
                let should_minimize_always_on_top =
                    window_mgmt_opts.minimize_always_on_top.unwrap_or(true);
                if should_minimize_always_on_top {
                    let always_on_top_windows =
                        self.window_manager.get_always_on_top_windows().await;
                    if !always_on_top_windows.is_empty() {
                        tracing::info!(
                            "Found {} always-on-top Win32 windows to minimize",
                            always_on_top_windows.len()
                        );

                        if let Some(hwnd) = uwp_hwnd {
                            match self
                                .window_manager
                                .minimize_always_on_top_windows(hwnd)
                                .await
                            {
                                Ok(count) => {
                                    tracing::info!(
                                        "Minimized {} Win32 always-on-top windows (UWP target app)",
                                        count
                                    );
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "Failed to minimize always-on-top windows for UWP app: {}",
                                        e
                                    );
                                }
                            }
                        } else {
                            tracing::warn!(
                                "Could not get HWND from UWP element to minimize always-on-top windows"
                            );
                        }
                    }
                }
            } else {
                // Win32 app: use traditional window management
                // Try PID-based lookup first if available, fallback to process name
                let window = if let Some(pid) = process_id {
                    self.window_manager.get_topmost_window_for_pid(pid).await
                } else {
                    self.window_manager
                        .get_topmost_window_for_process(process)
                        .await
                };

                if let Some(window) = window {
                    // Minimize always-on-top Win32 windows
                    let should_minimize_always_on_top =
                        window_mgmt_opts.minimize_always_on_top.unwrap_or(true);
                    if should_minimize_always_on_top {
                        let always_on_top_windows =
                            self.window_manager.get_always_on_top_windows().await;
                        if !always_on_top_windows.is_empty() {
                            tracing::info!(
                                "Found {} always-on-top Win32 windows",
                                always_on_top_windows.len()
                            );
                            match self
                                .window_manager
                                .minimize_always_on_top_windows(window.hwnd)
                                .await
                            {
                                Ok(count) => {
                                    tracing::info!(
                                        "Minimized {} always-on-top Win32 windows for {}",
                                        count,
                                        process
                                    );
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "Failed to minimize always-on-top windows: {}",
                                        e
                                    );
                                }
                            }
                        }
                    }

                    // Maximize target Win32 window
                    let should_maximize_target = window_mgmt_opts.maximize_target.unwrap_or(false);
                    let should_bring_to_front = window_mgmt_opts.bring_to_front.unwrap_or(true);

                    if should_maximize_target {
                        match self.window_manager.maximize_if_needed(window.hwnd).await {
                            Ok(true) => {
                                tracing::info!("Maximized Win32 window for {}", process);
                            }
                            Ok(false) => {
                                tracing::debug!("Win32 window already maximized");
                            }
                            Err(e) => {
                                tracing::warn!("Failed to maximize Win32 window: {}", e);
                            }
                        }
                    }

                    // Bring window to front (independent of maximize)
                    if should_bring_to_front {
                        match self.window_manager.bring_window_to_front(window.hwnd).await {
                            Ok(true) => {
                                tracing::info!("Brought Win32 window to front for {}", process);
                            }
                            Ok(false) => {
                                tracing::debug!(
                                    "Failed to bring Win32 window to front (Windows restrictions)"
                                );
                            }
                            Err(e) => {
                                tracing::warn!("Failed to bring Win32 window to front: {}", e);
                            }
                        }
                    }
                } else {
                    tracing::warn!(
                        "Could not find Win32 window for '{}' for window management",
                        process
                    );
                }
            }
        }

        let elapsed = start.elapsed();
        tracing::info!("[TIMING] prepare_window_management took {:?}", elapsed);
        Ok(())
    }

    /// Restore windows after tool execution
    /// Only restores for non-sequence calls or when explicitly needed
    async fn restore_window_management(&self, should_restore: bool) {
        if should_restore {
            if let Err(e) = self.window_manager.restore_all_windows().await {
                tracing::warn!("Failed to restore windows: {}", e);
            } else {
                tracing::info!("Restored all windows to original state");
            }
            self.window_manager.clear_captured_state().await;
        }
    }

    // Minimal, conservative parser to extract `{ set_env: {...} }` from simple scripts
    // like `return { set_env: { a: 1, b: 'x' } };`. This is only used as a fallback
    // when Node/Bun execution is unavailable, to support env propagation tests.
    #[allow(dead_code)]
    fn parse_set_env_from_script(script: &str) -> Option<serde_json::Value> {
        // Quick check for the pattern "return {" and "set_env" to avoid heavy parsing
        let lower = script.to_ascii_lowercase();
        if !lower.contains("return") || !lower.contains("set_env") {
            return None;
        }

        // Heuristic extraction: find the first '{' after 'return' and the matching '}'
        let return_pos = lower.find("return")?;
        let brace_start = script[return_pos..].find('{')? + return_pos;

        // Naive brace matching to capture the returned object
        let mut depth = 0i32;
        let mut end_idx = None;
        for (i, ch) in script[brace_start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end_idx = Some(brace_start + i + 1);
                        break;
                    }
                }
                _ => {}
            }
        }
        let end = end_idx?;
        let object_src = &script[brace_start..end];

        // Convert a very small subset of JS object syntax to JSON:
        // - wrap unquoted keys
        // - convert single quotes to double quotes
        // - allow trailing semicolon outside
        let mut jsonish = object_src.to_string();
        // Replace single quotes with double quotes
        jsonish = jsonish.replace('\'', "\"");
        // Quote bare keys using a conservative regex-like pass
        // This is not a full parser; it aims to handle simple literals used in tests
        let mut out = String::with_capacity(jsonish.len() + 16);
        let mut chars = jsonish.chars().peekable();
        let mut in_string = false;
        while let Some(c) = chars.next() {
            if c == '"' {
                in_string = !in_string;
                out.push(c);
                continue;
            }
            if !in_string && c.is_alphabetic() {
                // start of a possibly bare key
                let mut key = String::new();
                key.push(c);
                while let Some(&nc) = chars.peek() {
                    if nc.is_alphanumeric() || nc == '_' {
                        key.push(nc);
                        chars.next();
                    } else {
                        break;
                    }
                }
                // If the next non-space char is ':' then this was a key
                let mut look = chars.clone();
                let mut ws = String::new();
                while let Some(&nc) = look.peek() {
                    if nc.is_whitespace() {
                        ws.push(nc);
                        look.next();
                    } else {
                        break;
                    }
                }
                if let Some(':') = look.peek().copied() {
                    out.push('"');
                    out.push_str(&key);
                    out.push('"');
                    out.push_str(&ws);
                    out.push(':');
                    // Advance original iterator to after ws and ':'
                    for _ in 0..ws.len() {
                        chars.next();
                    }
                    chars.next();
                } else {
                    out.push_str(&key);
                }
                continue;
            }
            out.push(c);
        }

        // Try to parse as JSON
        if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&out) {
            // Only accept objects containing set_env as an object
            if let Some(obj) = val.as_object_mut() {
                if let Some(set_env_val) = obj.get("set_env").cloned() {
                    if set_env_val.is_object() {
                        return Some(val);
                    }
                }
            }
        }
        None
    }
    pub fn new() -> Result<Self, McpError> {
        Self::new_with_log_capture(None)
    }

    pub fn new_with_log_capture(
        log_capture: Option<crate::tool_logging::LogCapture>,
    ) -> Result<Self, McpError> {
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        let desktop = match Desktop::new(false, false) {
            Ok(d) => d,
            Err(e) => {
                return Err(McpError::internal_error(
                    "Failed to initialize terminator desktop",
                    serde_json::to_value(e.to_string()).ok(),
                ))
            }
        };

        #[cfg(target_os = "macos")]
        let desktop = match Desktop::new(true, true) {
            Ok(d) => d,
            Err(e) => {
                return Err(McpError::internal_error(
                    "Failed to initialize terminator desktop",
                    serde_json::to_value(e.to_string()).ok(),
                ))
            }
        };

        Ok(Self {
            desktop: Arc::new(desktop),
            tool_router: Self::tool_router(),
            request_manager: crate::cancellation::RequestManager::new(),
            active_highlights: Arc::new(Mutex::new(Vec::new())),
            log_capture,
            current_workflow_dir: Arc::new(Mutex::new(None)),
            current_scripts_base_path: Arc::new(Mutex::new(None)),
            window_manager: Arc::new(crate::window_manager::WindowManager::new()),
            in_sequence: Arc::new(std::sync::Mutex::new(false)),
            ocr_bounds: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            omniparser_items: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            uia_bounds: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            dom_bounds: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            #[cfg(target_os = "windows")]
            inspect_overlay_handle: Arc::new(std::sync::Mutex::new(None)),
        })
    }

    /// Detect if a PID belongs to a browser process
    fn detect_browser_by_pid(pid: u32) -> bool {
        const KNOWN_BROWSER_PROCESS_NAMES: &[&str] = &[
            "chrome", "firefox", "msedge", "edge", "iexplore", "opera", "brave", "vivaldi",
            "browser", "arc",
        ];

        #[cfg(target_os = "windows")]
        {
            use terminator::get_process_name_by_pid;
            if let Ok(process_name) = get_process_name_by_pid(pid as i32) {
                let process_name_lower = process_name.to_lowercase();
                return KNOWN_BROWSER_PROCESS_NAMES
                    .iter()
                    .any(|&browser| process_name_lower.contains(browser));
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = pid; // Suppress unused warning
        }

        false
    }

    /// Capture all visible DOM elements from the current browser tab
    /// Returns (elements, viewport_offset_x, viewport_offset_y) for screen coordinate conversion
    /// The viewport offset is derived from the UIA Document element's screen bounds
    async fn capture_browser_dom_elements(
        &self,
        max_elements: u32,
    ) -> Result<(Vec<serde_json::Value>, f64, f64), String> {
        // First, find the Document element to get viewport screen position
        // This is more reliable than JavaScript window properties (which break with DPI scaling)
        let viewport_offset = match self
            .desktop
            .locator("role:Document")
            .first(Some(Duration::from_millis(2000)))
            .await
        {
            Ok(doc_element) => {
                match doc_element.bounds() {
                    Ok((x, y, _w, _h)) => (x, y),
                    Err(_) => (0.0, 0.0), // Fallback
                }
            }
            Err(_) => (0.0, 0.0), // Fallback
        };
        // Script to extract ALL visible elements using TreeWalker
        let script = format!(
            r#"
(function() {{
    const elements = [];
    const maxElements = {max_elements}; // Configurable limit"#
        ) + r#"

    // Use TreeWalker to traverse ALL elements in the DOM
    const walker = document.createTreeWalker(
        document.body,
        NodeFilter.SHOW_ELEMENT,
        {
            acceptNode: function(node) {
                // Check if element is visible
                const style = window.getComputedStyle(node);
                const rect = node.getBoundingClientRect();

                if (style.display === 'none' ||
                    style.visibility === 'hidden' ||
                    style.opacity === '0' ||
                    rect.width === 0 ||
                    rect.height === 0) {
                    return NodeFilter.FILTER_SKIP;
                }

                return NodeFilter.FILTER_ACCEPT;
            }
        }
    );

    let node;
    while (node = walker.nextNode()) {
        if (elements.length >= maxElements) {
            break;
        }

        const rect = node.getBoundingClientRect();
        const text = node.innerText ? node.innerText.substring(0, 100).trim() : null;

        elements.push({
            tag: node.tagName.toLowerCase(),
            id: node.id || null,
            classes: Array.from(node.classList),
            text: text,
            href: node.href || null,
            type: node.type || null,
            name: node.name || null,
            value: node.value || null,
            placeholder: node.placeholder || null,
            aria_label: node.getAttribute('aria-label'),
            role: node.getAttribute('role'),
            // Scale by devicePixelRatio to convert CSS pixels to physical pixels
            x: Math.round(rect.x * window.devicePixelRatio),
            y: Math.round(rect.y * window.devicePixelRatio),
            width: Math.round(rect.width * window.devicePixelRatio),
            height: Math.round(rect.height * window.devicePixelRatio)
        });
    }

    return JSON.stringify({
        elements: elements,
        total_found: elements.length,
        page_url: window.location.href,
        page_title: document.title,
        devicePixelRatio: window.devicePixelRatio
    });
})()
"#;

        match self.desktop.execute_browser_script(&script).await {
            Ok(result_str) => match serde_json::from_str::<serde_json::Value>(&result_str) {
                Ok(result) => {
                    let elements = result
                        .get("elements")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    // Use UIA-based viewport offset (more reliable than JS due to DPI scaling)
                    Ok((elements, viewport_offset.0, viewport_offset.1))
                }
                Err(e) => Err(format!("Failed to parse DOM elements: {e}")),
            },
            Err(e) => Err(format!("Failed to execute browser script: {e}")),
        }
    }

    /// Perform Omniparser V2 detection on a window by its process ID
    async fn perform_omniparser_for_process(
        &self,
        pid: u32,
    ) -> Result<Vec<crate::omniparser::OmniparserItem>, String> {
        // Find the window element for this process
        let apps = self
            .desktop
            .applications()
            .map_err(|e| format!("Failed to get applications: {e}"))?;

        let window_element = apps
            .into_iter()
            .find(|app| app.process_id().unwrap_or(0) == pid)
            .ok_or_else(|| format!("No window found for PID {pid}"))?;

        // Get window bounds (absolute screen coordinates)
        let bounds = window_element
            .bounds()
            .map_err(|e| format!("Failed to get window bounds: {e}"))?;
        let (window_x, window_y, win_w, win_h) = bounds;

        // Capture screenshot of the window
        let screenshot = window_element
            .capture()
            .map_err(|e| format!("Failed to capture window screenshot: {e}"))?;

        // Get original screenshot dimensions
        let original_width = screenshot.width;
        let original_height = screenshot.height;

        // DPI DEBUG: Compare logical window bounds vs physical screenshot size
        let dpi_scale_w = original_width as f64 / win_w;
        let dpi_scale_h = original_height as f64 / win_h;
        info!(
            "OMNIPARSER DPI DEBUG: window_bounds(logical)=({:.0},{:.0},{:.0},{:.0}), screenshot(physical)={}x{}, dpi_scale=({:.3},{:.3})",
            window_x, window_y, win_w, win_h, original_width, original_height, dpi_scale_w, dpi_scale_h
        );

        // Convert BGRA to RGBA (xcap returns BGRA format)
        let rgba_data: Vec<u8> = screenshot
            .image_data
            .chunks_exact(4)
            .flat_map(|bgra| [bgra[2], bgra[1], bgra[0], bgra[3]]) // B,G,R,A -> R,G,B,A
            .collect();

        // Apply resize if needed (max 1920px to match Replicate's imgsz limit)
        const MAX_DIM: u32 = 1920;
        let (final_width, final_height, final_rgba_data, scale_factor) = if original_width > MAX_DIM
            || original_height > MAX_DIM
        {
            // Calculate new dimensions maintaining aspect ratio
            let scale = (MAX_DIM as f32 / original_width.max(original_height) as f32).min(1.0);
            let new_width = (original_width as f32 * scale).round() as u32;
            let new_height = (original_height as f32 * scale).round() as u32;

            // Create ImageBuffer from RGBA data and resize
            let img =
                ImageBuffer::<Rgba<u8>, _>::from_raw(original_width, original_height, rgba_data)
                    .ok_or_else(|| {
                        "Failed to create image buffer from screenshot data".to_string()
                    })?;

            let resized =
                image::imageops::resize(&img, new_width, new_height, FilterType::Lanczos3);

            info!(
                "OmniParser: Resized screenshot from {}x{} to {}x{} (scale: {:.2})",
                original_width, original_height, new_width, new_height, scale
            );

            (new_width, new_height, resized.into_raw(), scale as f64)
        } else {
            (original_width, original_height, rgba_data, 1.0)
        };

        // Encode to PNG
        let mut png_data = Vec::new();
        let encoder = PngEncoder::new(Cursor::new(&mut png_data));
        encoder
            .write_image(
                &final_rgba_data,
                final_width,
                final_height,
                ExtendedColorType::Rgba8,
            )
            .map_err(|e| format!("Failed to encode screenshot to PNG: {e}"))?;

        let base64_image = general_purpose::STANDARD.encode(&png_data);

        info!(
            "OmniParser: Sending {}x{} image ({} KB)",
            final_width,
            final_height,
            png_data.len() / 1024
        );

        // Call OmniParser backend with imgsz=1920 for best detection
        let (items, _raw_json) = crate::omniparser::parse_image_with_backend(
            &base64_image,
            final_width,
            final_height,
            Some(MAX_DIM), // Use max imgsz for best detection quality
        )
        .await
        .map_err(|e| format!("Omniparser failed: {e}"))?;

        // Convert coordinates to absolute screen coordinates
        // If image was resized, scale coordinates back to original size first
        let mut absolute_items = Vec::new();
        for item in items {
            let mut new_item = item.clone();
            if let Some(box_2d) = new_item.box_2d {
                // box_2d is [x_min, y_min, x_max, y_max] relative to the (possibly resized) screenshot
                // Scale back to original size if image was resized, then add window offset
                let inv_scale = 1.0 / scale_factor;
                new_item.box_2d = Some([
                    (box_2d[0] * inv_scale) + window_x,
                    (box_2d[1] * inv_scale) + window_y,
                    (box_2d[2] * inv_scale) + window_x,
                    (box_2d[3] * inv_scale) + window_y,
                ]);
            }
            absolute_items.push(new_item);
        }

        Ok(absolute_items)
    }

    /// Perform OCR on a window by its process ID and return structured results with bounding boxes
    #[cfg(target_os = "windows")]
    async fn perform_ocr_for_process(&self, pid: u32) -> Result<terminator::OcrElement, String> {
        // Find the window element for this process
        let apps = self
            .desktop
            .applications()
            .map_err(|e| format!("Failed to get applications: {e}"))?;

        let window_element = apps
            .into_iter()
            .find(|app| app.process_id().unwrap_or(0) == pid)
            .ok_or_else(|| format!("No window found for PID {pid}"))?;

        // Get window bounds (absolute screen coordinates)
        let bounds = window_element
            .bounds()
            .map_err(|e| format!("Failed to get window bounds: {e}"))?;

        let (window_x, window_y, win_w, win_h) = bounds;

        // Capture screenshot of the window
        let screenshot = window_element
            .capture()
            .map_err(|e| format!("Failed to capture window screenshot: {e}"))?;

        // DPI DEBUG: Compare logical window bounds vs physical screenshot size
        let scale_ratio_w = screenshot.width as f64 / win_w;
        let scale_ratio_h = screenshot.height as f64 / win_h;
        info!(
            "OCR DPI DEBUG: window_bounds(logical)=({:.0},{:.0},{:.0},{:.0}), screenshot(physical)={}x{}, scale_ratio=({:.3},{:.3})",
            window_x, window_y, win_w, win_h, screenshot.width, screenshot.height, scale_ratio_w, scale_ratio_h
        );

        // Perform OCR with bounding boxes using Desktop's method
        self.desktop
            .ocr_screenshot_with_bounds(&screenshot, window_x, window_y)
            .map_err(|e| format!("OCR failed: {e}"))
    }

    #[cfg(not(target_os = "windows"))]
    async fn perform_ocr_for_process(&self, _pid: u32) -> Result<terminator::OcrElement, String> {
        Err("OCR with bounding boxes is currently only supported on Windows".to_string())
    }

    #[tool(
        description = "Get the complete UI tree for an application by process name (e.g., 'chrome', 'msedge', 'notepad'). Returns tree for the first matching process found. Returns detailed element information (role, name, id, enabled state, bounds, children). This is your primary tool for understanding the application's current state. Supports tree optimization: tree_max_depth (e.g., 30) to limit tree depth when you only need shallow inspection, tree_from_selector to get subtrees starting from a specific element, include_detailed_attributes to control verbosity (defaults to true). Use `include_ocr: true` to perform OCR and get indexed words (e.g., [OcrWord] #1 \"Submit\") for click targeting with `click_ocr_index`. For browser windows (chrome, msedge, firefox), automatically captures HTML DOM elements via Chrome extension and returns them in `browser_dom` field. Use `browser_dom_max_elements` to control how many DOM elements to capture (default: 200). The DOM format follows `tree_output_format` (compact YAML by default). This is a read-only operation."
    )]
    pub async fn get_window_tree(
        &self,
        Parameters(args): Parameters<GetWindowTreeArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("get_window_tree", None);
        span.set_attribute("process", args.process.clone());
        if let Some(title) = &args.title {
            span.set_attribute("window_title", title.clone());
        }
        span.set_attribute(
            "include_detailed_attributes",
            args.tree
                .include_detailed_attributes
                .unwrap_or(true)
                .to_string(),
        );

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            !*in_sequence
        };

        if should_restore {
            tracing::info!(
                "[get_window_tree] Direct MCP call detected - performing window management"
            );
            let _ = self
                .prepare_window_management(&args.process, None, None, None, &args.window_mgmt)
                .await;
        } else {
            tracing::debug!("[get_window_tree] In sequence - skipping window management (dispatch_tool handles it)");
        }

        // Find PID for the process name
        let apps = self.desktop.applications().map_err(|e| {
            McpError::resource_not_found(
                "Failed to get applications",
                Some(json!({"reason": e.to_string()})),
            )
        })?;

        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::All, true);

        // Find first matching process
        let pid = apps
            .iter()
            .filter_map(|app| {
                let app_pid = app.process_id().unwrap_or(0);
                if app_pid > 0 {
                    system
                        .process(sysinfo::Pid::from_u32(app_pid))
                        .and_then(|p| {
                            let process_name = p.name().to_string_lossy().to_string();
                            if process_name
                                .to_lowercase()
                                .contains(&args.process.to_lowercase())
                            {
                                Some(app_pid)
                            } else {
                                None
                            }
                        })
                } else {
                    None
                }
            })
            .next()
            .ok_or_else(|| {
                McpError::resource_not_found(
                    format!(
                        "Process '{}' not found. Use open_application to start it first.",
                        args.process
                    ),
                    Some(json!({"process": args.process})),
                )
            })?;

        span.set_attribute("pid", pid.to_string());

        // Detect if this is a browser window
        let is_browser = Self::detect_browser_by_pid(pid);

        // Build the base result JSON first
        let mut result_json = json!({
            "action": "get_window_tree",
            "status": "success",
            "process": args.process,
            "pid": pid,
            "title": args.title,
            "detailed_attributes": args.tree.include_detailed_attributes.unwrap_or(true),
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "recommendation": "Prefer role|name selectors (e.g., 'button|Submit'). For large trees, use tree_from_selector: \"role:Dialog\" to focus on specific UI regions."
        });

        // Add browser detection metadata
        if is_browser {
            result_json["is_browser"] = json!(true);
            info!("Browser window detected for PID {}", pid);

            // Try to capture DOM elements from browser (if enabled)
            if args.include_browser_dom {
                let max_dom_elements = args.browser_dom_max_elements.unwrap_or(200);
                match self.capture_browser_dom_elements(max_dom_elements).await {
                    Ok((dom_elements, viewport_offset_x, viewport_offset_y))
                        if !dom_elements.is_empty() =>
                    {
                        // Format based on tree_output_format
                        let format = args
                            .tree
                            .tree_output_format
                            .unwrap_or(crate::mcp_types::TreeOutputFormat::CompactYaml);

                        match format {
                            crate::mcp_types::TreeOutputFormat::CompactYaml => {
                                let dom_result =
                                    crate::tree_formatter::format_browser_dom_as_compact_yaml(
                                        &dom_elements,
                                    );
                                result_json["browser_dom"] = json!(dom_result.formatted);

                                // Store DOM bounds with screen coordinates applied
                                if let Ok(mut cache) = self.dom_bounds.lock() {
                                    cache.clear();
                                    let mut first_logged = false;
                                    for (index, (tag, identifier, (x, y, w, h))) in
                                        dom_result.index_to_bounds
                                    {
                                        // Convert viewport-relative to screen coordinates
                                        let screen_x = x + viewport_offset_x;
                                        let screen_y = y + viewport_offset_y;
                                        // DPI DEBUG: Log first element conversion
                                        if !first_logged {
                                            info!(
                                            "DOM DPI DEBUG: viewport_offset=({:.0},{:.0}), first_elem viewport_rel=({:.0},{:.0}), screen=({:.0},{:.0})",
                                            viewport_offset_x, viewport_offset_y, x, y, screen_x, screen_y
                                        );
                                            first_logged = true;
                                        }
                                        cache.insert(
                                            index,
                                            (tag, identifier, (screen_x, screen_y, w, h)),
                                        );
                                    }
                                    info!(
                                        "Stored {} DOM element bounds for click_index",
                                        cache.len()
                                    );
                                }
                            }
                            crate::mcp_types::TreeOutputFormat::VerboseJson => {
                                result_json["browser_dom"] = json!(dom_elements);
                            }
                        }
                        result_json["browser_dom_count"] = json!(dom_elements.len());
                        info!("Captured {} DOM elements from browser", dom_elements.len());
                    }
                    Ok(_) => {
                        info!("Browser detected but no DOM elements captured (extension may not be available)");
                        result_json["browser_dom_error"] = json!("No DOM elements captured - Chrome extension may not be installed or active");
                    }
                    Err(e) => {
                        warn!("Failed to capture browser DOM: {}", e);
                        result_json["browser_dom_error"] = json!(e.to_string());
                    }
                }
            }
        }

        // Use maybe_attach_tree to handle tree extraction with from_selector support
        // Store the returned bounds cache for click_index tool
        // Enable include_all_bounds when showing ui_tree overlay (need bounds for all elements)
        let include_all_bounds = args
            .show_overlay
            .as_ref()
            .map(|s| s == "ui_tree")
            .unwrap_or(false);
        if let Some(bounds_cache) = crate::helpers::maybe_attach_tree(
            &self.desktop,
            args.tree.include_tree_after_action,
            args.tree.tree_max_depth,
            args.tree.tree_from_selector.as_deref(),
            args.tree.include_detailed_attributes,
            args.tree.tree_output_format,
            Some(pid),
            &mut result_json,
            None, // No found element for window tree
            include_all_bounds,
        )
        .await
        {
            if let Ok(mut cache) = self.uia_bounds.lock() {
                *cache = bounds_cache;
            }
        }

        // Perform OCR if requested
        if args.include_ocr {
            match self.perform_ocr_for_process(pid).await {
                Ok(ocr_result) => {
                    // Format OCR tree based on tree_output_format (same as UI tree)
                    let format = args
                        .tree
                        .tree_output_format
                        .unwrap_or(crate::mcp_types::TreeOutputFormat::CompactYaml);

                    match format {
                        crate::mcp_types::TreeOutputFormat::CompactYaml => {
                            let ocr_formatting_result =
                                crate::tree_formatter::format_ocr_tree_as_compact_yaml(
                                    &ocr_result,
                                    0,
                                );
                            // Store the index-to-bounds mapping for click_ocr_index
                            if let Ok(mut bounds) = self.ocr_bounds.lock() {
                                *bounds = ocr_formatting_result.index_to_bounds;
                            }
                            result_json["ocr_tree"] = json!(ocr_formatting_result.formatted);
                        }
                        crate::mcp_types::TreeOutputFormat::VerboseJson => {
                            result_json["ocr_tree"] =
                                serde_json::to_value(&ocr_result).unwrap_or_default();
                        }
                    }
                    info!("OCR completed for PID {}", pid);
                }
                Err(e) => {
                    warn!("OCR failed for PID {}: {}", pid, e);
                    result_json["ocr_error"] = json!(e.to_string());
                }
            }
        }

        // Perform Omniparser if requested
        if args.include_omniparser {
            match self.perform_omniparser_for_process(pid).await {
                Ok(items) => {
                    // Format Omniparser tree based on tree_output_format (same as UI tree)
                    let format = args
                        .tree
                        .tree_output_format
                        .unwrap_or(crate::mcp_types::TreeOutputFormat::CompactYaml);

                    match format {
                        crate::mcp_types::TreeOutputFormat::CompactYaml => {
                            let (formatted, cache) =
                                crate::tree_formatter::format_omniparser_tree_as_compact_yaml(
                                    &items,
                                );
                            if let Ok(mut locked_cache) = self.omniparser_items.lock() {
                                *locked_cache = cache;
                            }
                            result_json["omniparser_tree"] = json!(formatted);
                        }
                        crate::mcp_types::TreeOutputFormat::VerboseJson => {
                            let mut omniparser_tree = Vec::new();
                            let mut cache = HashMap::new();

                            for (i, item) in items.iter().enumerate() {
                                let index = (i + 1) as u32;
                                cache.insert(index, item.clone());

                                omniparser_tree.push(json!({
                                    "index": index,
                                    "label": item.label,
                                    "content": item.content,
                                    "bounds": item.box_2d,
                                }));
                            }

                            if let Ok(mut locked_cache) = self.omniparser_items.lock() {
                                *locked_cache = cache;
                            }

                            result_json["omniparser_tree"] = json!(omniparser_tree);
                        }
                    }
                    info!("Omniparser completed for PID {}", pid);
                }
                Err(e) => {
                    warn!("Omniparser failed for PID {}: {}", pid, e);
                    result_json["omniparser_error"] = json!(e.to_string());
                }
            }
        }

        // Handle show_overlay request
        #[cfg(target_os = "windows")]
        if let Some(ref overlay_type) = args.show_overlay {
            // Parse display mode from args
            let display_mode = match args.overlay_display_mode.as_deref() {
                Some("rectangles") => terminator::OverlayDisplayMode::Rectangles,
                Some("index") | None => terminator::OverlayDisplayMode::Index,
                Some("role") => terminator::OverlayDisplayMode::Role,
                Some("index_role") => terminator::OverlayDisplayMode::IndexRole,
                Some("name") => terminator::OverlayDisplayMode::Name,
                Some("index_name") => terminator::OverlayDisplayMode::IndexName,
                Some("full") => terminator::OverlayDisplayMode::Full,
                Some(other) => {
                    result_json["overlay_error"] = json!(format!("Unknown overlay_display_mode: '{}'. Valid options: rectangles, index, role, index_role, name, index_name, full", other));
                    terminator::OverlayDisplayMode::Index // fallback to default
                }
            };

            match overlay_type.as_str() {
                "ui_tree" => {
                    // Use UIA bounds from uia_bounds cache (like OCR/DOM do)
                    if let Ok(uia_bounds) = self.uia_bounds.lock() {
                        let elements: Vec<terminator::InspectElement> = uia_bounds
                            .iter()
                            .map(|(idx, (role, name, bounds))| terminator::InspectElement {
                                index: *idx,
                                role: role.clone(),
                                name: if name.is_empty() {
                                    None
                                } else {
                                    Some(name.clone())
                                },
                                bounds: *bounds,
                            })
                            .collect();

                        if !elements.is_empty() {
                            if let Ok(apps) = self.desktop.applications() {
                                if let Some(app) =
                                    apps.iter().find(|a| a.process_id().ok() == Some(pid))
                                {
                                    if let Ok((x, y, w, h)) = app.bounds() {
                                        if let Ok(mut handle) = self.inspect_overlay_handle.lock() {
                                            *handle = None;
                                        }
                                        terminator::hide_inspect_overlay();

                                        match terminator::show_inspect_overlay(
                                            elements,
                                            (x as i32, y as i32, w as i32, h as i32),
                                            display_mode,
                                        ) {
                                            Ok(new_handle) => {
                                                if let Ok(mut handle) =
                                                    self.inspect_overlay_handle.lock()
                                                {
                                                    *handle = Some(new_handle);
                                                }
                                                result_json["overlay_shown"] = json!("ui_tree");
                                                info!("Inspect overlay shown for ui_tree");
                                            }
                                            Err(e) => {
                                                warn!("Failed to show inspect overlay: {}", e);
                                                result_json["overlay_error"] = json!(e.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            result_json["overlay_error"] = json!("No UI elements with bounds in cache - ensure include_tree_after_action is true");
                        }
                    }
                }
                "ocr" => {
                    // Use OCR bounds from ocr_bounds cache
                    if let Ok(ocr_bounds) = self.ocr_bounds.lock() {
                        let elements: Vec<terminator::InspectElement> = ocr_bounds
                            .iter()
                            .map(|(idx, (text, bounds))| terminator::InspectElement {
                                index: *idx,
                                role: "OCR".to_string(),
                                name: Some(text.clone()),
                                bounds: *bounds,
                            })
                            .collect();

                        if !elements.is_empty() {
                            // DPI DEBUG: Log sample element bounds for OCR
                            if let Some(first) = elements.first() {
                                info!(
                                    "OCR OVERLAY DEBUG: first_element bounds=({:.0},{:.0},{:.0},{:.0})",
                                    first.bounds.0, first.bounds.1, first.bounds.2, first.bounds.3
                                );
                            }
                            if let Ok(apps) = self.desktop.applications() {
                                if let Some(app) =
                                    apps.iter().find(|a| a.process_id().ok() == Some(pid))
                                {
                                    if let Ok((x, y, w, h)) = app.bounds() {
                                        info!(
                                            "OCR OVERLAY DEBUG: window_bounds for overlay=({:.0},{:.0},{:.0},{:.0})",
                                            x, y, w, h
                                        );
                                        if let Ok(mut handle) = self.inspect_overlay_handle.lock() {
                                            *handle = None;
                                        }
                                        terminator::hide_inspect_overlay();

                                        match terminator::show_inspect_overlay(
                                            elements,
                                            (x as i32, y as i32, w as i32, h as i32),
                                            display_mode,
                                        ) {
                                            Ok(new_handle) => {
                                                if let Ok(mut handle) =
                                                    self.inspect_overlay_handle.lock()
                                                {
                                                    *handle = Some(new_handle);
                                                }
                                                result_json["overlay_shown"] = json!("ocr");
                                            }
                                            Err(e) => {
                                                result_json["overlay_error"] = json!(e.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                "omniparser" => {
                    // Use omniparser items from cache
                    if let Ok(omni_items) = self.omniparser_items.lock() {
                        let elements: Vec<terminator::InspectElement> = omni_items
                            .iter()
                            .filter_map(|(idx, item)| {
                                // Convert box_2d [x_min, y_min, x_max, y_max] to (x, y, width, height)
                                item.box_2d.map(|b| terminator::InspectElement {
                                    index: *idx,
                                    role: item.label.clone(),
                                    name: item.content.clone(),
                                    bounds: (b[0], b[1], b[2] - b[0], b[3] - b[1]),
                                })
                            })
                            .collect();

                        if !elements.is_empty() {
                            // DPI DEBUG: Log sample element bounds for omniparser
                            if let Some(first) = elements.first() {
                                info!(
                                    "OMNIPARSER OVERLAY DEBUG: first_element bounds=({:.0},{:.0},{:.0},{:.0})",
                                    first.bounds.0, first.bounds.1, first.bounds.2, first.bounds.3
                                );
                            }
                            if let Ok(apps) = self.desktop.applications() {
                                if let Some(app) =
                                    apps.iter().find(|a| a.process_id().ok() == Some(pid))
                                {
                                    if let Ok((x, y, w, h)) = app.bounds() {
                                        info!(
                                            "OMNIPARSER OVERLAY DEBUG: window_bounds for overlay=({:.0},{:.0},{:.0},{:.0})",
                                            x, y, w, h
                                        );
                                        if let Ok(mut handle) = self.inspect_overlay_handle.lock() {
                                            *handle = None;
                                        }
                                        terminator::hide_inspect_overlay();

                                        match terminator::show_inspect_overlay(
                                            elements,
                                            (x as i32, y as i32, w as i32, h as i32),
                                            display_mode,
                                        ) {
                                            Ok(new_handle) => {
                                                if let Ok(mut handle) =
                                                    self.inspect_overlay_handle.lock()
                                                {
                                                    *handle = Some(new_handle);
                                                }
                                                result_json["overlay_shown"] = json!("omniparser");
                                            }
                                            Err(e) => {
                                                result_json["overlay_error"] = json!(e.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                "dom" => {
                    // Use DOM bounds from dom_bounds cache (populated by include_browser_dom)
                    if let Ok(dom_bounds) = self.dom_bounds.lock() {
                        let elements: Vec<terminator::InspectElement> = dom_bounds
                            .iter()
                            .map(
                                |(idx, (tag, identifier, bounds))| terminator::InspectElement {
                                    index: *idx,
                                    role: tag.clone(),
                                    name: if identifier.is_empty() {
                                        None
                                    } else {
                                        Some(identifier.clone())
                                    },
                                    bounds: *bounds,
                                },
                            )
                            .collect();

                        if !elements.is_empty() {
                            // DPI DEBUG: Log sample element bounds for DOM
                            if let Some(first) = elements.first() {
                                info!(
                                    "DOM OVERLAY DEBUG: first_element bounds=({:.0},{:.0},{:.0},{:.0})",
                                    first.bounds.0, first.bounds.1, first.bounds.2, first.bounds.3
                                );
                            }
                            if let Ok(apps) = self.desktop.applications() {
                                if let Some(app) =
                                    apps.iter().find(|a| a.process_id().ok() == Some(pid))
                                {
                                    if let Ok((x, y, w, h)) = app.bounds() {
                                        info!(
                                            "DOM OVERLAY DEBUG: window_bounds for overlay=({:.0},{:.0},{:.0},{:.0})",
                                            x, y, w, h
                                        );
                                        if let Ok(mut handle) = self.inspect_overlay_handle.lock() {
                                            *handle = None;
                                        }
                                        terminator::hide_inspect_overlay();

                                        match terminator::show_inspect_overlay(
                                            elements,
                                            (x as i32, y as i32, w as i32, h as i32),
                                            display_mode,
                                        ) {
                                            Ok(new_handle) => {
                                                if let Ok(mut handle) =
                                                    self.inspect_overlay_handle.lock()
                                                {
                                                    *handle = Some(new_handle);
                                                }
                                                result_json["overlay_shown"] = json!("dom");
                                                info!("Inspect overlay shown for DOM elements");
                                            }
                                            Err(e) => {
                                                result_json["overlay_error"] = json!(e.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            result_json["overlay_error"] = json!("No DOM elements in cache - ensure include_browser_dom is true and browser extension is active");
                        }
                    }
                }
                _ => {
                    result_json["overlay_error"] =
                        json!(format!("Unknown overlay type: {}", overlay_type));
                }
            }
        }

        span.set_status(true, None);
        span.end();

        let contents = append_monitor_screenshots_if_enabled(
            &self.desktop,
            vec![Content::json(result_json)?],
            args.monitor.include_monitor_screenshots,
        )
        .await;

        self.restore_window_management(should_restore).await;

        Ok(CallToolResult::success(contents))
    }

    #[tool(
        description = "Get all applications and windows currently running with their process names. Returns a list with name, process_name, id, pid, and is_focused status for each application/window. Use this to check which applications are running and which window has focus before performing actions. This is a read-only operation that returns a simple list without UI trees."
    )]
    pub async fn get_applications_and_windows_list(
        &self,
        Parameters(_args): Parameters<GetApplicationsArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("get_applications_and_windows_list", None);

        let apps = self.desktop.applications().map_err(|e| {
            McpError::resource_not_found(
                "Failed to get applications",
                Some(json!({"reason": e.to_string()})),
            )
        })?;

        // Create System for process name lookup
        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::All, true);

        // Build PID -> process_name map
        let process_names: HashMap<u32, String> = apps
            .iter()
            .filter_map(|app| {
                let pid = app.process_id().unwrap_or(0);
                if pid > 0 {
                    system
                        .process(sysinfo::Pid::from_u32(pid))
                        .map(|p| (pid, p.name().to_string_lossy().to_string()))
                } else {
                    None
                }
            })
            .collect();

        // Simple iteration - no async spawning needed (no tree fetching)
        let applications: Vec<_> = apps
            .iter()
            .map(|app| {
                let app_name = app.name().unwrap_or_default();
                let app_id = app.id().unwrap_or_default();
                let app_role = app.role();
                let app_pid = app.process_id().unwrap_or(0);
                let is_focused = app.is_focused().unwrap_or(false);
                let process_name = process_names.get(&app_pid).cloned();

                let suggested_selector = if !app_name.is_empty() {
                    format!("{}|{}", &app_role, &app_name)
                } else {
                    format!("#{app_id}")
                };

                json!({
                    "name": app_name,
                    "process_name": process_name,
                    "id": app_id,
                    "role": app_role,
                    "pid": app_pid,
                    "is_focused": is_focused,
                    "suggested_selector": suggested_selector
                })
            })
            .collect();

        let result_json = json!({
            "action": "get_applications_and_windows_list",
            "status": "success",
            "applications": applications,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        span.set_status(true, None);
        span.end();

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    /// Helper function to ensure element is scrolled into view for reliable interaction
    /// Uses sophisticated scrolling logic with focus fallback and viewport positioning
    /// Returns Ok(()) if element is visible or successfully scrolled into view
    fn ensure_element_in_view(element: &UIElement) -> Result<(), String> {
        // Helper function to check if rectangles intersect
        fn rects_intersect(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
            let (ax, ay, aw, ah) = a;
            let (bx, by, bw, bh) = b;
            let a_right = ax + aw;
            let a_bottom = ay + ah;
            let b_right = bx + bw;
            let b_bottom = by + bh;
            ax < b_right && a_right > bx && ay < b_bottom && a_bottom > by
        }

        // Helper function to check if element is within work area (Windows only)
        #[cfg(target_os = "windows")]
        fn check_work_area(ex: f64, ey: f64, ew: f64, eh: f64) -> bool {
            use terminator::platforms::windows::element::WorkArea;
            if let Ok(work_area) = WorkArea::get_primary() {
                work_area.intersects(ex, ey, ew, eh)
            } else {
                true // If we can't get work area, assume visible
            }
        }

        #[cfg(not(target_os = "windows"))]
        fn check_work_area(_ex: f64, _ey: f64, _ew: f64, _eh: f64) -> bool {
            true // Non-Windows platforms don't need taskbar adjustment
        }

        // Check if element needs scrolling
        let mut need_scroll = false;

        if let Ok((ex, ey, ew, eh)) = element.bounds() {
            tracing::debug!("Element bounds: x={ex}, y={ey}, w={ew}, h={eh}");

            // First check if element is outside work area (behind taskbar)
            if !check_work_area(ex, ey, ew, eh) {
                tracing::info!("Element outside work area (possibly behind taskbar), need scroll");
                need_scroll = true;
            } else {
                // Try to get window bounds, but if that fails, use heuristics
                if let Ok(Some(win)) = element.window() {
                    if let Ok((wx, wy, ww, wh)) = win.bounds() {
                        tracing::debug!("Window bounds: x={wx}, y={wy}, w={ww}, h={wh}");

                        let e_box = (ex, ey, ew, eh);
                        let w_box = (wx, wy, ww, wh);
                        if !rects_intersect(e_box, w_box) {
                            tracing::info!("Element NOT in viewport, need scroll");
                            need_scroll = true;
                        } else {
                            tracing::debug!(
                                "Element IS in viewport and work area, no scroll needed"
                            );
                        }
                    } else {
                        // Use dynamic work area height instead of hardcoded 1080
                        #[cfg(target_os = "windows")]
                        {
                            use terminator::platforms::windows::element::WorkArea;
                            if let Ok(work_area) = WorkArea::get_primary() {
                                let work_height = work_area.height as f64;
                                if ey > work_height - 100.0 {
                                    tracing::info!("Element Y={ey} near bottom of work area, assuming needs scroll");
                                    need_scroll = true;
                                }
                            } else if ey > 1080.0 {
                                // Fallback to heuristic if we can't get work area
                                tracing::info!("Element Y={ey} > 1080, assuming needs scroll");
                                need_scroll = true;
                            }
                        }
                        #[cfg(not(target_os = "windows"))]
                        {
                            if ey > 1080.0 {
                                tracing::info!("Element Y={ey} > 1080, assuming needs scroll");
                                need_scroll = true;
                            }
                        }
                    }
                } else {
                    // Use dynamic work area height instead of hardcoded 1080
                    #[cfg(target_os = "windows")]
                    {
                        use terminator::platforms::windows::element::WorkArea;
                        if let Ok(work_area) = WorkArea::get_primary() {
                            let work_height = work_area.height as f64;
                            if ey > work_height - 100.0 {
                                tracing::info!("Element Y={ey} near bottom of work area, assuming needs scroll");
                                need_scroll = true;
                            }
                        } else if ey > 1080.0 {
                            // Fallback to heuristic if we can't get work area
                            tracing::info!("Element Y={ey} > 1080, assuming needs scroll");
                            need_scroll = true;
                        }
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        if ey > 1080.0 {
                            tracing::info!("Element Y={ey} > 1080, assuming needs scroll");
                            need_scroll = true;
                        }
                    }
                }
            }
        } else if !element.is_visible().unwrap_or(true) {
            tracing::info!("Element not visible, needs scroll");
            need_scroll = true;
        }

        if need_scroll {
            // First try focusing the element to allow the application to auto-scroll it into view
            tracing::info!("Element outside viewport; attempting focus() to auto-scroll into view");
            match element.focus() {
                Ok(()) => {
                    // Re-check visibility/intersection after focus
                    std::thread::sleep(std::time::Duration::from_millis(50));

                    let mut still_offscreen = false;
                    if let Ok((_, ey2, _, _)) = element.bounds() {
                        tracing::debug!("After focus(), element Y={ey2}");
                        // Use same heuristic as before
                        if ey2 > 1080.0 {
                            tracing::debug!("After focus(), element Y={ey2} still > 1080");
                            still_offscreen = true;
                        } else {
                            tracing::info!("Focus() brought element into view");
                        }
                    } else if !element.is_visible().unwrap_or(true) {
                        still_offscreen = true;
                    }

                    if !still_offscreen {
                        tracing::info!(
                            "Focus() brought element into view; skipping scroll_into_view"
                        );
                        need_scroll = false;
                    } else {
                        tracing::info!("Focus() did not bring element into view; will attempt scroll_into_view()");
                    }
                }
                Err(e) => {
                    tracing::debug!("Focus() failed: {e}; will attempt scroll_into_view()");
                }
            }

            if need_scroll {
                tracing::info!("Element outside viewport; attempting scroll_into_view()");
                if let Err(e) = element.scroll_into_view() {
                    tracing::warn!("scroll_into_view failed: {e}");
                    // Don't return error, scrolling is best-effort
                } else {
                    tracing::info!("scroll_into_view succeeded");

                    // After initial scroll, verify element position and adjust if needed
                    std::thread::sleep(std::time::Duration::from_millis(50)); // Let initial scroll settle

                    if let Ok((_, ey, _, eh)) = element.bounds() {
                        tracing::debug!("After scroll_into_view, element at y={ey}");

                        // Define dynamic viewport zones based on work area
                        #[cfg(target_os = "windows")]
                        let (viewport_top_edge, viewport_optimal_bottom, viewport_bottom_edge) = {
                            use terminator::platforms::windows::element::WorkArea;
                            if let Ok(work_area) = WorkArea::get_primary() {
                                let work_height = work_area.height as f64;
                                (
                                    100.0,               // Too close to top
                                    work_height * 0.65,  // Good zone ends at 65% of work area
                                    work_height - 100.0, // Too close to bottom (accounting for taskbar)
                                )
                            } else {
                                // Fallback to defaults if work area unavailable
                                (100.0, 700.0, 900.0)
                            }
                        };

                        #[cfg(not(target_os = "windows"))]
                        let (viewport_top_edge, viewport_optimal_bottom, viewport_bottom_edge) =
                            (100.0, 700.0, 900.0);

                        // Check if we have window bounds for more accurate positioning
                        let mut needs_adjustment = false;
                        let mut adjustment_direction: Option<&str> = None;
                        let adjustment_amount: f64 = 0.3; // Smaller adjustment

                        if let Ok(Some(window)) = element.window() {
                            if let Ok((_, wy, _, wh)) = window.bounds() {
                                // We have window bounds - use precise positioning
                                let element_relative_y = ey - wy;
                                let element_bottom = element_relative_y + eh;

                                tracing::debug!(
                                    "Element relative_y={element_relative_y}, window_height={wh}"
                                );

                                // Check if element is poorly positioned
                                if element_relative_y < 50.0 {
                                    // Too close to top - scroll up a bit
                                    tracing::debug!(
                                        "Element too close to top ({element_relative_y}px)"
                                    );
                                    needs_adjustment = true;
                                    adjustment_direction = Some("up");
                                } else if element_bottom > wh - 50.0 {
                                    // Too close to bottom or cut off - scroll down a bit
                                    tracing::debug!("Element too close to bottom or cut off");
                                    needs_adjustment = true;
                                    adjustment_direction = Some("down");
                                } else if element_relative_y > wh * 0.7 {
                                    // Element is in lower 30% of viewport - not ideal
                                    tracing::debug!("Element in lower portion of viewport");
                                    needs_adjustment = true;
                                    adjustment_direction = Some("down");
                                }
                            } else {
                                // No window bounds - use heuristic based on absolute Y position
                                if ey < viewport_top_edge {
                                    tracing::debug!(
                                        "Element at y={ey} < {viewport_top_edge}, too high"
                                    );
                                    needs_adjustment = true;
                                    adjustment_direction = Some("up");
                                } else if ey > viewport_bottom_edge {
                                    tracing::debug!(
                                        "Element at y={ey} > {viewport_bottom_edge}, too low"
                                    );
                                    needs_adjustment = true;
                                    adjustment_direction = Some("down");
                                } else if ey > viewport_optimal_bottom {
                                    // Element is lower than optimal but not at edge
                                    tracing::debug!("Element at y={ey} lower than optimal");
                                    needs_adjustment = true;
                                    adjustment_direction = Some("down");
                                }
                            }
                        } else {
                            // No window available - use simple heuristics
                            if !(viewport_top_edge..=viewport_bottom_edge).contains(&ey) {
                                needs_adjustment = true;
                                adjustment_direction = Some(if ey < 500.0 { "up" } else { "down" });
                                tracing::debug!("Element at y={ey} outside optimal range");
                            }
                        }

                        // Apply fine-tuning adjustment if needed
                        if needs_adjustment {
                            if let Some(dir) = adjustment_direction {
                                tracing::debug!(
                                    "Fine-tuning position: scrolling {dir} by {adjustment_amount}"
                                );
                                let _ = element.scroll(dir, adjustment_amount);
                                std::thread::sleep(std::time::Duration::from_millis(30));
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Ensures element is visible and applies highlighting before action with hardcoded defaults
    fn ensure_visible_and_apply_highlight(element: &UIElement, action_name: &str) {
        // Always ensure element is in view first (for all actions, not just when highlighting)
        if let Err(e) = Self::ensure_element_in_view(element) {
            tracing::warn!("Failed to ensure element is in view for {action_name} action: {e}");
        }

        // Hardcoded highlight configuration
        let duration = Some(std::time::Duration::from_millis(500));
        let color = Some(0x00FF00); // Green in BGR
        let role_text = element.role();
        let text = Some(role_text.as_str());

        #[cfg(target_os = "windows")]
        let text_position = Some(crate::mcp_types::TextPosition::Top.into());
        #[cfg(not(target_os = "windows"))]
        let text_position = None;

        #[cfg(target_os = "windows")]
        let font_style = Some(
            crate::mcp_types::FontStyle {
                size: 12,
                bold: true,
                color: 0xFFFFFF, // White text
            }
            .into(),
        );
        #[cfg(not(target_os = "windows"))]
        let font_style = None;

        tracing::info!(
            "HIGHLIGHT_BEFORE_{} duration={:?} role={}",
            action_name.to_uppercase(),
            duration,
            role_text
        );
        if let Ok(_highlight_handle) =
            element.highlight(color, duration, text, text_position, font_style)
        {
            // Highlight applied successfully - runs concurrently with action
        } else {
            tracing::warn!("Failed to apply highlighting before {action_name} action");
        }
    }

    #[tool(
        description = "Types text into a UI element with smart clipboard optimization and verification. Much faster than press key. REQUIRED: clear_before_typing parameter - set to true to clear existing text, false to append. This action requires the application to be focused and may change the UI."
    )]
    async fn type_into_element(
        &self,
        Parameters(args): Parameters<TypeIntoElementArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut span = StepSpan::new("type_into_element", None);

        // Add comprehensive telemetry attributes
        span.set_attribute("selector", args.selector.selector.clone());
        span.set_attribute("text.length", args.text_to_type.len().to_string());
        span.set_attribute("clear_before_typing", args.clear_before_typing.to_string());
        // Log if explicit verification is requested
        if !args.action.verify_element_exists.is_empty()
            || !args.action.verify_element_not_exists.is_empty()
        {
            span.set_attribute("verification.explicit", "true".to_string());
        }
        if let Some(timeout) = args.action.timeout_ms {
            span.set_attribute("timeout_ms", timeout.to_string());
        }
        if let Some(retries) = args.action.retries {
            span.set_attribute("retry.max_attempts", retries.to_string());
        }

        tracing::info!(
            "[type_into_element] Called with selector: '{}', process: {:?}, window_selector: {:?}",
            args.selector.selector,
            args.selector.process,
            args.selector.window_selector
        );

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            let flag_value = *in_sequence;
            let should_restore_value = !flag_value;
            tracing::info!(
                "[type_into_element] Flag check: in_sequence={}, should_restore={}",
                flag_value,
                should_restore_value
            );
            should_restore_value
        };

        if should_restore {
            tracing::info!(
                "[type_into_element] Direct MCP call detected - performing window management"
            );
            let _ = self
                .prepare_window_management(
                    &args.selector.process,
                    None,
                    None,
                    None,
                    &args.window_mgmt,
                )
                .await;
        } else {
            tracing::debug!("[type_into_element] In sequence - skipping window management (dispatch_tool handles it)");
        }

        let text_to_type = args.text_to_type.clone();
        let should_clear = args.clear_before_typing;
        let try_focus_before = args.try_focus_before;
        let try_click_before = args.try_click_before;
        let highlight_before = args.highlight.highlight_before_action;

        let action = {
            move |element: UIElement| {
                let text_to_type = text_to_type.clone();
                async move {
                    // Activate window to ensure it has keyboard focus before typing
                    if let Err(e) = element.activate_window() {
                        tracing::warn!("Failed to activate window before typing: {}", e);
                    }

                    // Apply highlighting before action if enabled
                    if highlight_before {
                        Self::ensure_visible_and_apply_highlight(&element, "type");
                    }

                    // Execute the typing action with state tracking
                    if should_clear {
                        if let Err(clear_error) = element.set_value("") {
                            warn!(
                                "Warning: Failed to clear element before typing: {}",
                                clear_error
                            );
                        }
                    }
                    element.type_text_with_state_and_focus(
                        &text_to_type,
                        true,
                        try_focus_before,
                        try_click_before,
                    )
                }
            }
        };

        let operation_start = std::time::Instant::now();

        // Store tree config to avoid move issues
        let tree_output_format = args
            .tree
            .tree_output_format
            .unwrap_or(crate::mcp_types::TreeOutputFormat::CompactYaml);

        // Build scoped selectors
        let full_selector = args.selector.build_full_selector();
        let alternative_selectors = args.selector.build_alternative_selectors();
        let fallback_selectors = args.selector.build_fallback_selectors();

        let ((result, element), successful_selector, ui_diff) =
            match crate::helpers::find_and_execute_with_ui_diff(
                &self.desktop,
                &full_selector,
                alternative_selectors.as_deref(),
                fallback_selectors.as_deref(),
                args.action.timeout_ms,
                args.action.retries,
                action,
                args.tree.ui_diff_before_after,
                args.tree.tree_max_depth,
                args.tree.include_detailed_attributes,
                tree_output_format,
            )
            .await
            {
                Ok(((result, element), selector, diff)) => {
                    let operation_time_ms = operation_start.elapsed().as_millis() as i64;
                    span.set_attribute("operation.duration_ms", operation_time_ms.to_string());
                    span.set_attribute("element.found", "true".to_string());
                    span.set_attribute("selector.successful", selector.clone());
                    if diff.is_some() {
                        span.set_attribute("ui_diff.captured", "true".to_string());
                    }

                    // Add element metadata
                    if let Some(name) = element.name() {
                        span.set_attribute("element.name", name);
                    }
                    if let Ok(focused) = element.is_focused() {
                        span.set_attribute("element.is_focused", focused.to_string());
                    }

                    Ok(((result, element), selector, diff))
                }
                Err(e) => {
                    // Note: Cannot use span here as it would be moved if we call span.end()
                    Err(build_element_not_found_error(
                        &args.selector.build_full_selector(),
                        args.selector.build_alternative_selectors().as_deref(),
                        args.selector.build_fallback_selectors().as_deref(),
                        e,
                    ))
                }
            }?;

        let mut result_json = json!({
            "action": "type_into_element",
            "status": "success",
            "text_typed": args.text_to_type,
            "cleared_before_typing": args.clear_before_typing,
            "action_result": {
                "action": result.action,
                "details": result.details,
                "data": result.data,
            },
            "element": build_element_info(&element),
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector.build_full_selector(), args.selector.build_alternative_selectors().as_deref(), args.selector.build_fallback_selectors().as_deref()),
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        // POST-ACTION VERIFICATION: Magic auto-verification or explicit verification
        // 1. If verify_element_exists/not_exists is explicitly set, use it
        // 2. Otherwise, auto-infer verification from tool arguments (magic)
        // 3. To disable auto-verification, set verify_element_exists to empty string ""

        let should_auto_verify = args.action.verify_element_exists.is_empty()
            && args.action.verify_element_not_exists.is_empty();

        let verify_exists = if should_auto_verify {
            // MAGIC AUTO-VERIFICATION: Infer from text_to_type
            // Auto-verify that typed text appears in the element
            tracing::debug!("[type_into_element] Auto-verification enabled for typed text");
            span.set_attribute("verification.auto_inferred", "true".to_string());
            format!("text:{}", args.text_to_type)
        } else {
            // Use explicit verification selector (supports variable substitution)
            args.action.verify_element_exists.clone()
        };

        let verify_not_exists = args.action.verify_element_not_exists.clone();

        // Skip verification if both are empty strings (explicit opt-out)
        let skip_verification = verify_exists.is_empty() && verify_not_exists.is_empty();

        // Perform verification if any selector is specified (auto or explicit) and not explicitly disabled
        if !skip_verification {
            span.add_event("verification_started", vec![]);

            let verify_timeout_ms = args.action.verify_timeout_ms.unwrap_or(2000);
            span.set_attribute("verification.timeout_ms", verify_timeout_ms.to_string());

            // Substitute variables in verification selectors
            let context = json!({
                "text_to_type": args.text_to_type,
                "selector": args.selector.selector,
            });

            let mut substituted_exists = verify_exists.clone();
            let mut substituted_not_exists = verify_not_exists.clone();

            if !substituted_exists.is_empty() {
                let mut val = json!(&substituted_exists);
                crate::helpers::substitute_variables(&mut val, &context);
                if let Some(s) = val.as_str() {
                    substituted_exists = s.to_string();
                }
            }

            if !substituted_not_exists.is_empty() {
                let mut val = json!(&substituted_not_exists);
                crate::helpers::substitute_variables(&mut val, &context);
                if let Some(s) = val.as_str() {
                    substituted_not_exists = s.to_string();
                }
            }

            // Call the new generic verification function (uses window-scoped search with .within())
            let verify_exists_opt = if substituted_exists.is_empty() {
                None
            } else {
                Some(substituted_exists.as_str())
            };
            let verify_not_exists_opt = if substituted_not_exists.is_empty() {
                None
            } else {
                Some(substituted_not_exists.as_str())
            };

            match crate::helpers::verify_post_action(
                &self.desktop,
                &element,
                verify_exists_opt,
                verify_not_exists_opt,
                verify_timeout_ms,
                &successful_selector,
            )
            .await
            {
                Ok(verification_result) => {
                    tracing::info!(
                        "[type_into_element] Verification passed: method={}, details={}",
                        verification_result.method,
                        verification_result.details
                    );
                    span.set_attribute("verification.passed", "true".to_string());
                    span.set_attribute("verification.method", verification_result.method.clone());
                    span.set_attribute(
                        "verification.elapsed_ms",
                        verification_result.elapsed_ms.to_string(),
                    );

                    // Add verification details to result
                    let verification_json = json!({
                        "passed": verification_result.passed,
                        "method": verification_result.method,
                        "details": verification_result.details,
                        "elapsed_ms": verification_result.elapsed_ms,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    });

                    if let Some(obj) = result_json.as_object_mut() {
                        obj.insert("verification".to_string(), verification_json);
                    }
                }
                Err(e) => {
                    tracing::error!("[type_into_element] Verification failed: {}", e);
                    span.set_attribute("verification.passed", "false".to_string());
                    span.set_status(false, Some("Verification failed"));
                    span.end();
                    return Err(McpError::internal_error(
                        format!("Post-action verification failed: {e}"),
                        Some(json!({
                            "selector_used": successful_selector,
                            "verify_exists": substituted_exists,
                            "verify_not_exists": substituted_not_exists,
                            "timeout_ms": verify_timeout_ms,
                        })),
                    ));
                }
            }
        }

        // Attach UI diff if captured (action tools only support diff, not standalone tree)
        if let Some(diff_result) = ui_diff {
            tracing::debug!(
                "[type_into_element] Attaching UI diff to result (has_changes: {})",
                diff_result.has_changes
            );
            span.set_attribute("ui_diff.has_changes", diff_result.has_changes.to_string());

            result_json["ui_diff"] = json!(diff_result.diff);
            result_json["has_ui_changes"] = json!(diff_result.has_changes);
            if args.tree.ui_diff_include_full_trees_in_response.unwrap_or(false) {
                result_json["tree_before"] = json!(diff_result.tree_before);
                result_json["tree_after"] = json!(diff_result.tree_after);
            }
        }

        // Restore windows after typing into element
        self.restore_window_management(should_restore).await;

        span.set_status(true, None);
        span.end();
        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(result_json)?],
                args.monitor.include_monitor_screenshots,
            )
            .await,
        ))
    }

    #[tool(
        description = "Clicks a UI element using Playwright-style actionability validation. Performs comprehensive pre-action checks: element must be visible (non-zero bounds), enabled, in viewport, and have stable bounds (3 consecutive checks at 16ms intervals, max ~800ms wait). Returns success with 'validated=true' in click_result.details when all checks pass. Fails explicitly with specific errors: ElementNotVisible (zero-size bounds/offscreen/not in viewport), ElementNotEnabled (disabled/grayed out), ElementNotStable (bounds still animating after 800ms), ElementDetached (no longer in UI tree), ElementObscured (covered by another element), or ScrollFailed (could not scroll into view). For buttons, prefer invoke_element (uses UI Automation's native invoke pattern, doesn't require viewport visibility). Use click_element for links, hover-sensitive elements, or UI requiring actual mouse interaction. REQUIRED: click_position parameter - use x_percentage: 50, y_percentage: 50 for center click (most common). This action requires the application to be focused and may change the UI."
    )]
    pub async fn click_element(
        &self,
        Parameters(args): Parameters<ClickElementArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("click_element", None);
        span.set_attribute("selector", args.selector.selector.clone());

        tracing::info!(
            "[click_element] Called with selector: '{}'",
            args.selector.selector
        );

        // Record retry configuration
        if let Some(retries) = args.action.retries {
            span.set_attribute("retry.max_attempts", retries.to_string());
        }

        span.set_attribute(
            "click.position_x",
            args.click_position.x_percentage.to_string(),
        );
        span.set_attribute(
            "click.position_y",
            args.click_position.y_percentage.to_string(),
        );
        tracing::info!(
            "[click_element] Click position: {}%, {}%",
            args.click_position.x_percentage,
            args.click_position.y_percentage
        );

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            let flag_value = *in_sequence;
            let should_restore_value = !flag_value;
            tracing::info!(
                "[click_element] Flag check: in_sequence={}, should_restore={}",
                flag_value,
                should_restore_value
            );
            should_restore_value
        };

        if should_restore {
            tracing::info!(
                "[click_element] Direct MCP call detected - performing window management"
            );
            let _ = self
                .prepare_window_management(
                    &args.selector.process,
                    None,
                    None,
                    None,
                    &args.window_mgmt,
                )
                .await;
        } else {
            tracing::debug!("[click_element] In sequence - skipping window management (dispatch_tool handles it)");
        }

        let highlight_before = args.highlight.highlight_before_action;
        let action = {
            let click_position = args.click_position.clone();
            move |element: UIElement| {
                let click_position = click_position.clone();
                async move {
                    // Ensure element is visible and apply highlighting if enabled
                    if highlight_before {
                        Self::ensure_visible_and_apply_highlight(&element, "click");
                    }

                    // Click at specified position
                    // Get element bounds to calculate absolute position
                    match element.bounds() {
                        Ok(bounds) => {
                            // Calculate absolute coordinates from percentages
                            let x =
                                bounds.0 + (bounds.2 * click_position.x_percentage as f64 / 100.0);
                            let y =
                                bounds.1 + (bounds.3 * click_position.y_percentage as f64 / 100.0);

                            tracing::debug!(
                                "[click_element] Clicking at absolute position ({}, {}) within bounds ({}, {}, {}, {})",
                                x, y, bounds.0, bounds.1, bounds.2, bounds.3
                            );

                            // Perform click at specific position
                            element.mouse_click_and_hold(x, y)?;
                            element.mouse_release()?;

                            // Return a ClickResult
                            use terminator::ClickResult;
                            Ok(ClickResult {
                                coordinates: Some((x, y)),
                                method: "Position Click".to_string(),
                                details: format!(
                                    "Clicked at {}%, {}%",
                                    click_position.x_percentage, click_position.y_percentage
                                ),
                            })
                        }
                        Err(e) => {
                            tracing::warn!("[click_element] Failed to get bounds for position click: {}. Falling back to center click.", e);
                            element.click()
                        }
                    }
                }
            }
        };

        // Track search and action time
        let operation_start = std::time::Instant::now();

        // Store tree config to avoid move issues (Option<TreeOutputFormat> is Copy since TreeOutputFormat is Copy)
        let tree_output_format = args
            .tree
            .tree_output_format
            .unwrap_or(crate::mcp_types::TreeOutputFormat::CompactYaml);

        // Use new wrapper that supports UI diff capture
        let result = crate::helpers::find_and_execute_with_ui_diff(
            &self.desktop,
            &args.selector.build_full_selector(),
            args.selector.build_alternative_selectors().as_deref(),
            args.selector.build_fallback_selectors().as_deref(),
            args.action.timeout_ms,
            args.action.retries,
            action,
            args.tree.ui_diff_before_after,
            args.tree.tree_max_depth,
            args.tree.include_detailed_attributes,
            tree_output_format,
        )
        .await;

        let operation_time_ms = operation_start.elapsed().as_millis() as i64;
        span.set_attribute("operation.duration_ms", operation_time_ms.to_string());

        let ((click_result, element), successful_selector, ui_diff) = match result {
            Ok(((result, element), selector, diff)) => {
                span.set_attribute("selector.used", selector.clone());
                span.set_attribute("element.found", "true".to_string());
                if diff.is_some() {
                    span.set_attribute("ui_diff.captured", "true".to_string());
                }
                ((result, element), selector, diff)
            }
            Err(e) => {
                span.set_attribute("element.found", "false".to_string());
                span.set_status(false, Some(&e.to_string()));
                span.end();
                return Err(build_element_not_found_error(
                    &args.selector.build_full_selector(),
                    args.selector.build_alternative_selectors().as_deref(),
                    args.selector.build_fallback_selectors().as_deref(),
                    e,
                ));
            }
        };

        // Track element metadata in telemetry
        span.set_attribute("element.role", element.role());
        if let Some(name) = element.name() {
            span.set_attribute("element.name", name);
        }
        let window_title = element.window_title();
        if !window_title.is_empty() {
            span.set_attribute("element.window_title", window_title.clone());
        }

        let element_info = build_element_info(&element);

        let mut result_json = json!({
            "action": "click",
            "status": "success",
            "selector_used": successful_selector,
            "click_result": {
                "method": click_result.method,
                "coordinates": click_result.coordinates,
                "details": click_result.details,
            },
            "element": element_info,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        // POST-ACTION VERIFICATION
        if !args.action.verify_element_exists.is_empty()
            || !args.action.verify_element_not_exists.is_empty()
        {
            let verify_timeout_ms = args.action.verify_timeout_ms.unwrap_or(2000);

            let verify_exists_opt = if args.action.verify_element_exists.is_empty() {
                None
            } else {
                Some(args.action.verify_element_exists.as_str())
            };
            let verify_not_exists_opt = if args.action.verify_element_not_exists.is_empty() {
                None
            } else {
                Some(args.action.verify_element_not_exists.as_str())
            };

            match crate::helpers::verify_post_action(
                &self.desktop,
                &element,
                verify_exists_opt,
                verify_not_exists_opt,
                verify_timeout_ms,
                &successful_selector,
            )
            .await
            {
                Ok(verification_result) => {
                    tracing::info!(
                        "[click_element] Verification passed: method={}, details={}",
                        verification_result.method,
                        verification_result.details
                    );
                    span.set_attribute("verification.passed", "true".to_string());
                    span.set_attribute("verification.method", verification_result.method.clone());
                    span.set_attribute(
                        "verification.elapsed_ms",
                        verification_result.elapsed_ms.to_string(),
                    );

                    let verification_json = json!({
                        "passed": verification_result.passed,
                        "method": verification_result.method,
                        "details": verification_result.details,
                        "elapsed_ms": verification_result.elapsed_ms,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    });

                    if let Some(obj) = result_json.as_object_mut() {
                        obj.insert("verification".to_string(), verification_json);
                    }
                }
                Err(e) => {
                    tracing::error!("[click_element] Verification failed: {}", e);
                    span.set_attribute("verification.passed", "false".to_string());
                    span.set_status(false, Some("Verification failed"));
                    span.end();
                    return Err(McpError::internal_error(
                        format!("Post-action verification failed: {e}"),
                        Some(json!({
                            "selector_used": successful_selector,
                            "verify_exists": args.action.verify_element_exists,
                            "verify_not_exists": args.action.verify_element_not_exists,
                            "timeout_ms": verify_timeout_ms,
                        })),
                    ));
                }
            }
        }

        // Attach UI diff if captured (action tools only support diff, not standalone tree)
        if let Some(diff_result) = ui_diff {
            tracing::debug!(
                "[click_element] Attaching UI diff to result (has_changes: {})",
                diff_result.has_changes
            );
            span.set_attribute("ui_diff.has_changes", diff_result.has_changes.to_string());

            result_json["ui_diff"] = json!(diff_result.diff);
            result_json["has_ui_changes"] = json!(diff_result.has_changes);
            if args.tree.ui_diff_include_full_trees_in_response.unwrap_or(false) {
                result_json["tree_before"] = json!(diff_result.tree_before);
                result_json["tree_after"] = json!(diff_result.tree_after);
            }
        }

        // Restore windows if this was a direct MCP call
        self.restore_window_management(should_restore).await;

        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(result_json)?],
                args.monitor.include_monitor_screenshots,
            )
            .await,
        ))
    }

    #[tool(
        description = "Sends a key press to a UI element. Use curly brace format: '{Ctrl}c', '{Alt}{F4}', '{Enter}', '{PageDown}', '{Tab}', etc. This action requires the application to be focused and may change the UI.

Note: Curly brace format (e.g., '{Tab}') is more reliable than plain format (e.g., 'Tab')."
    )]
    async fn press_key(
        &self,
        Parameters(args): Parameters<PressKeyArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut span = StepSpan::new("press_key", None);

        // Add comprehensive telemetry attributes
        span.set_attribute("selector", args.selector.selector.clone());
        span.set_attribute("key", args.key.clone());
        if let Some(timeout) = args.action.timeout_ms {
            span.set_attribute("timeout_ms", timeout.to_string());
        }
        if let Some(retries) = args.action.retries {
            span.set_attribute("retry.max_attempts", retries.to_string());
        }

        tracing::info!(
            "[press_key] Called with selector: '{}', key: '{}'",
            args.selector.selector,
            args.key
        );

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            let flag_value = *in_sequence;
            let should_restore_value = !flag_value;
            tracing::info!(
                "[press_key] Flag check: in_sequence={}, should_restore={}",
                flag_value,
                should_restore_value
            );
            should_restore_value
        };

        if should_restore {
            tracing::info!("[press_key] Direct MCP call detected - performing window management");
            let _ = self
                .prepare_window_management(
                    &args.selector.process,
                    None,
                    None,
                    None,
                    &args.window_mgmt,
                )
                .await;
        } else {
            tracing::debug!(
                "[press_key] In sequence - skipping window management (dispatch_tool handles it)"
            );
        }

        let key_to_press = args.key.clone();
        let try_focus_before = args.try_focus_before;
        let try_click_before = args.try_click_before;
        let highlight_before = args.highlight.highlight_before_action;
        let action = {
            move |element: UIElement| {
                let key_to_press = key_to_press.clone();
                async move {
                    // Activate window to ensure it has keyboard focus before pressing key
                    if let Err(e) = element.activate_window() {
                        tracing::warn!("Failed to activate window before pressing key: {}", e);
                    }

                    // Ensure element is visible and apply highlighting if enabled
                    if highlight_before {
                        Self::ensure_visible_and_apply_highlight(&element, "key");
                    }

                    // Execute the key press action with state tracking
                    element.press_key_with_state_and_focus(
                        &key_to_press,
                        try_focus_before,
                        try_click_before,
                    )
                }
            }
        };

        let operation_start = std::time::Instant::now();

        // Store tree config to avoid move issues
        let tree_output_format = args
            .tree
            .tree_output_format
            .unwrap_or(crate::mcp_types::TreeOutputFormat::CompactYaml);

        let ((result, element), successful_selector, ui_diff) =
            match crate::helpers::find_and_execute_with_ui_diff(
                &self.desktop,
                &args.selector.build_full_selector(),
                None, // PressKey doesn't have alternative selectors yet
                args.selector.build_fallback_selectors().as_deref(),
                args.action.timeout_ms,
                args.action.retries,
                action,
                args.tree.ui_diff_before_after,
                args.tree.tree_max_depth,
                args.tree.include_detailed_attributes,
                tree_output_format,
            )
            .await
            {
                Ok(((result, element), selector, diff)) => {
                    let operation_time_ms = operation_start.elapsed().as_millis() as i64;
                    span.set_attribute("operation.duration_ms", operation_time_ms.to_string());
                    span.set_attribute("element.found", "true".to_string());
                    span.set_attribute("selector.successful", selector.clone());
                    if diff.is_some() {
                        span.set_attribute("ui_diff.captured", "true".to_string());
                    }

                    // Add element metadata
                    if let Some(name) = element.name() {
                        span.set_attribute("element.name", name);
                    }

                    Ok(((result, element), selector, diff))
                }
                Err(e) => {
                    // Note: Cannot use span here as it would be moved if we call span.end()
                    Err(build_element_not_found_error(
                        &args.selector.build_full_selector(),
                        None,
                        args.selector.build_fallback_selectors().as_deref(),
                        e,
                    ))
                }
            }?;

        let element_info = build_element_info(&element);

        let mut result_json = json!({
            "action": "press_key",
            "status": "success",
            "key_pressed": args.key,
            "action_result": {
                "action": result.action,
                "details": result.details,
                "data": result.data,
            },
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector.build_full_selector(), None, args.selector.build_fallback_selectors().as_deref()),
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        // POST-ACTION VERIFICATION
        if !args.action.verify_element_exists.is_empty()
            || !args.action.verify_element_not_exists.is_empty()
        {
            let verify_timeout_ms = args.action.verify_timeout_ms.unwrap_or(2000);

            let verify_exists_opt = if args.action.verify_element_exists.is_empty() {
                None
            } else {
                Some(args.action.verify_element_exists.as_str())
            };
            let verify_not_exists_opt = if args.action.verify_element_not_exists.is_empty() {
                None
            } else {
                Some(args.action.verify_element_not_exists.as_str())
            };

            match crate::helpers::verify_post_action(
                &self.desktop,
                &element,
                verify_exists_opt,
                verify_not_exists_opt,
                verify_timeout_ms,
                &successful_selector,
            )
            .await
            {
                Ok(verification_result) => {
                    tracing::info!(
                        "[press_key] Verification passed: method={}, details={}",
                        verification_result.method,
                        verification_result.details
                    );
                    span.set_attribute("verification.passed", "true".to_string());
                    span.set_attribute("verification.method", verification_result.method.clone());
                    span.set_attribute(
                        "verification.elapsed_ms",
                        verification_result.elapsed_ms.to_string(),
                    );

                    let verification_json = json!({
                        "passed": verification_result.passed,
                        "method": verification_result.method,
                        "details": verification_result.details,
                        "elapsed_ms": verification_result.elapsed_ms,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    });

                    if let Some(obj) = result_json.as_object_mut() {
                        obj.insert("verification".to_string(), verification_json);
                    }
                }
                Err(e) => {
                    tracing::error!("[press_key] Verification failed: {}", e);
                    span.set_attribute("verification.passed", "false".to_string());
                    span.set_status(false, Some("Verification failed"));
                    span.end();
                    return Err(McpError::internal_error(
                        format!("Post-action verification failed: {e}"),
                        Some(json!({
                            "selector_used": successful_selector,
                            "verify_exists": args.action.verify_element_exists,
                            "verify_not_exists": args.action.verify_element_not_exists,
                            "timeout_ms": verify_timeout_ms,
                        })),
                    ));
                }
            }
        }

        // Attach UI diff if captured (action tools only support diff, not standalone tree)
        if let Some(diff_result) = ui_diff {
            tracing::debug!(
                "[press_key] Attaching UI diff to result (has_changes: {})",
                diff_result.has_changes
            );
            span.set_attribute("ui_diff.has_changes", diff_result.has_changes.to_string());

            result_json["ui_diff"] = json!(diff_result.diff);
            result_json["has_ui_changes"] = json!(diff_result.has_changes);
            if args.tree.ui_diff_include_full_trees_in_response.unwrap_or(false) {
                result_json["tree_before"] = json!(diff_result.tree_before);
                result_json["tree_after"] = json!(diff_result.tree_after);
            }
        }

        // Restore windows after pressing key
        self.restore_window_management(should_restore).await;

        span.set_status(true, None);
        span.end();
        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(result_json)?],
                args.monitor.include_monitor_screenshots,
            )
            .await,
        ))
    }

    #[tool(
        description = "Activates the window for the specified process and sends a key press to the focused element. Use curly brace format: '{Ctrl}c', '{Alt}{F4}', '{Enter}', '{PageDown}', '{Tab}', etc.

Note: Curly brace format (e.g., '{Tab}') is more reliable than plain format (e.g., 'Tab')."
    )]
    async fn press_key_global(
        &self,
        Parameters(args): Parameters<GlobalKeyArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut span = StepSpan::new("press_key_global", None);

        // Add telemetry attributes
        span.set_attribute("process", args.process.clone());
        span.set_attribute("key", args.key.clone());

        // Build selector to find the root window for the process
        // Using just process: prefix gets the main/root window directly
        let window_selector = format!("process:{}", args.process);
        span.set_attribute("window_selector", window_selector.clone());

        // Find the window element
        let operation_start = std::time::Instant::now();
        let window = self
            .desktop
            .locator(Selector::from(window_selector.as_str()))
            .first(None)
            .await
            .map_err(|e| {
                McpError::resource_not_found(
                    "Failed to find window for process",
                    Some(json!({
                        "reason": e.to_string(),
                        "process": args.process,
                        "selector": window_selector
                    })),
                )
            })?;

        let find_time_ms = operation_start.elapsed().as_millis() as i64;
        span.set_attribute("window.find_duration_ms", find_time_ms.to_string());

        // Activate the window to bring it to foreground
        window.activate_window().map_err(|e| {
            McpError::internal_error(
                "Failed to activate window",
                Some(json!({
                    "reason": e.to_string(),
                    "process": args.process
                })),
            )
        })?;
        span.set_attribute("window.activated", "true".to_string());

        // Get the focused element after activation
        let element = self.desktop.focused_element().map_err(|e| {
            McpError::internal_error(
                "Failed to get focused element after window activation",
                Some(json!({"reason": e.to_string()})),
            )
        })?;

        let operation_time_ms = operation_start.elapsed().as_millis() as i64;
        span.set_attribute("operation.duration_ms", operation_time_ms.to_string());
        span.set_attribute("focused_element.found", "true".to_string());

        // Add element metadata
        if let Some(name) = element.name() {
            span.set_attribute("element.name", name);
        }

        // Gather metadata for debugging / result payload
        let element_info = build_element_info(&element);
        let window_info = build_element_info(&window);

        // Perform the key press on the focused element
        element.press_key(&args.key).map_err(|e| {
            McpError::resource_not_found(
                "Failed to press key on focused element",
                Some(json!({
                    "reason": e.to_string(),
                    "key_pressed": args.key,
                    "element_info": element_info
                })),
            )
        })?;

        let mut result_json = json!({
            "action": "press_key_global",
            "status": "success",
            "process": args.process,
            "key_pressed": args.key,
            "window": window_info,
            "focused_element": element_info,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        // POST-ACTION VERIFICATION
        let verify_exists = args.verify_element_exists.clone();
        let verify_not_exists = args.verify_element_not_exists.clone();
        let verify_timeout_ms = args.verify_timeout_ms.unwrap_or(2000);

        let skip_verification = verify_exists.is_empty() && verify_not_exists.is_empty();

        if !skip_verification {
            span.add_event("verification_started", vec![]);

            let verify_exists_opt = if verify_exists.is_empty() {
                None
            } else {
                Some(verify_exists.as_str())
            };
            let verify_not_exists_opt = if verify_not_exists.is_empty() {
                None
            } else {
                Some(verify_not_exists.as_str())
            };

            match crate::helpers::verify_post_action(
                &self.desktop,
                &element,
                verify_exists_opt,
                verify_not_exists_opt,
                verify_timeout_ms,
                &window_selector,
            )
            .await
            {
                Ok(verification_result) => {
                    tracing::info!(
                        "[press_key_global] Verification passed: method={}, details={}",
                        verification_result.method,
                        verification_result.details
                    );
                    span.set_attribute("verification.passed", "true".to_string());
                    span.set_attribute("verification.method", verification_result.method.clone());
                    span.set_attribute(
                        "verification.elapsed_ms",
                        verification_result.elapsed_ms.to_string(),
                    );

                    let verification_json = json!({
                        "passed": verification_result.passed,
                        "method": verification_result.method,
                        "details": verification_result.details,
                        "elapsed_ms": verification_result.elapsed_ms,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    });
                    result_json["verification"] = verification_json;
                }
                Err(e) => {
                    tracing::error!("[press_key_global] Verification failed: {}", e);
                    span.set_attribute("verification.passed", "false".to_string());
                    span.set_attribute("verification.error", e.to_string());
                    let error_msg = format!("Verification failed: {e}");
                    span.set_status(false, Some(error_msg.as_str()));
                    span.end();

                    return Err(McpError::internal_error(
                        "Post-action verification failed",
                        Some(json!({
                            "error": e.to_string(),
                            "verify_element_exists": verify_exists,
                            "verify_element_not_exists": verify_not_exists,
                            "verify_timeout_ms": verify_timeout_ms,
                        })),
                    ));
                }
            }
        }

        // Action tools only support UI diff, not standalone tree attachment
        // (use ui_diff_before_after: true to capture before/after tree)

        span.set_status(true, None);
        span.end();
        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(result_json)?],
                args.monitor.include_monitor_screenshots,
            )
            .await,
        ))
    }

    #[tool(
        description = "IMPORTANT To know how to use this tool please call these tools to get documentation: search_terminator_api and get_terminator_api_docs.

Executes a shell command (GitHub Actions-style) OR runs inline code via an engine. Use 'run' for shell commands. Or set 'engine' to 'node'/'bun'/'javascript'/'typescript'/'ts' for JS/TS with terminator.js and provide the code in 'run' or 'script_file'. TypeScript is supported with automatic transpilation. When using engine mode, you can pass data to subsequent workflow steps by returning { set_env: { key: value } } or using console.log('::set-env name=key::value'). Access variables in later steps using direct syntax (e.g., 'key' in conditions or {{key}} in substitutions). NEW: Use 'script_file' to load scripts from files, 'env' to inject environment variables as 'var env = {...}'.

⚠️ CRITICAL: Pattern for Optional Element Detection
For optional UI elements (dialogs, popups, confirmations) that may or may not appear, use desktop.locator() with try/catch to check existence. This prevents timeout errors and enables conditional execution.

✅ RECOMMENDED Pattern - Window-Scoped (Most Accurate):
// Step 1: Check if optional element exists in specific window
try {
  // Scope to specific window first to avoid false positives
  const chromeWindow = await desktop.locator('role:Window|name:SAP Business One - Google Chrome').first();
  // Then search within that window
  await chromeWindow.locator('role:Button|name:Leave').first();
  return JSON.stringify({
    dialog_exists: 'true'
  });
} catch (e) {
  // Element not found
  return JSON.stringify({
    dialog_exists: 'false'
  });
}

✅ ALTERNATIVE Pattern - Desktop-Wide Search:
// When element could be in any window
try {
  await desktop.locator('role:Button|name:Leave').first();
  return JSON.stringify({
    dialog_exists: 'true'
  });
} catch (e) {
  return JSON.stringify({
    dialog_exists: 'false'
  });
}

// Step 2: In next workflow step, use 'if' condition:
// if: 'dialog_exists == \"true\"'

Performance Note: Using .first() with try/catch is ~8x faster than .all() for existence checks (1.3s vs 10.8s).

Important Scoping Pattern:
- desktop.locator() searches ALL windows/applications
- element.locator() searches only within that element's subtree
- Always scope to specific window when checking for window-specific dialogs

This pattern:
- Never fails the step (always returns data)
- Avoids timeout waiting for non-existent elements
- Enables conditional workflow execution
- More robust than validate_element which fails when element not found

Common use cases:
- Confirmation dialogs ('Are you sure?', 'Unsaved changes', 'Leave')
- Session/login dialogs that depend on state
- Browser restore prompts, password save dialogs
- Any conditionally-appearing UI element

⚠️ Variable Declaration Safety:
Terminator injects environment variables using 'var' - ALWAYS use typeof checks:
const myVar = (typeof env_var_name !== 'undefined') ? env_var_name : 'default';
const isActive = (typeof is_active !== 'undefined') ? is_active === 'true' : false;
const count = (typeof retry_count !== 'undefined') ? parseInt(retry_count) : 0;  // ✅ SAFE
// NEVER: const count = parseInt(retry_count || '0');  // ❌ DANGEROUS - will error if retry_count already declared

Examples:
// Primitives
const path = (typeof file_path !== 'undefined') ? file_path : './default';
const max = (typeof max_retries !== 'undefined') ? parseInt(max_retries) : 3;
// Collections (auto-parsed from JSON)
const entries = (typeof journal_entries !== 'undefined') ? journal_entries : [];
const config = (typeof app_config !== 'undefined') ? app_config : {};
// Tool results (step_id_result, step_id_status)
const apps = (typeof check_apps_result !== 'undefined') ? check_apps_result : [];

Data Passing:
Return fields (non-reserved) auto-merge to env for next steps:
return { file_path: '/data.txt', count: 42 };  // Available as file_path, count in next steps

System-reserved fields (don't auto-merge): status, error, logs, duration_ms, set_env

⚠️ Avoid collision-prone variable names: message, result, data, success, value, count, total, found, text, type, name, index
Use specific names instead: validationMessage, queryResult, tableData, entriesCount

include_logs Parameter:
Set include_logs: true to capture stdout/stderr output. Default is false for cleaner responses. On errors, logs are always included.
"
    )]
    async fn run_command(
        &self,
        Parameters(args): Parameters<RunCommandArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.run_command_impl(args, None).await
    }

    async fn run_command_impl(
        &self,
        args: RunCommandArgs,
        cancellation_token: Option<tokio_util::sync::CancellationToken>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("run_command_impl", None);

        // Engine-based execution path (provides SDK bindings)
        if let Some(engine_value) = args.engine.as_ref() {
            let engine = engine_value.to_ascii_lowercase();

            // Default timeout: 2 minutes (120000ms), 0 means no timeout
            let timeout_ms = args.timeout_ms.unwrap_or(120_000);
            let timeout_duration = std::time::Duration::from_millis(timeout_ms);

            // Track resolved script path for working directory determination
            let mut resolved_script_path: Option<PathBuf> = None;

            // Resolve script content from file or inline
            let script_content = if let Some(script_file) = &args.script_file {
                // Check that both run and script_file aren't provided
                if args.run.is_some() {
                    return Err(McpError::invalid_params(
                        "Cannot specify both 'run' and 'script_file'. Use one or the other.",
                        None,
                    ));
                }

                // Resolve script file with priority order:
                // 1. Try scripts_base_path if provided (from workflow root level)
                // 2. Fallback to workflow directory if available
                // 3. Use path as-is
                let resolved_path = {
                    let script_path = std::path::Path::new(script_file);
                    let mut resolved_path = None;
                    let mut resolution_attempts = Vec::new();

                    // Only resolve if path is relative
                    if script_path.is_relative() {
                        tracing::info!(
                            "[SCRIPTS_BASE_PATH] Resolving relative script file: '{}'",
                            script_file
                        );

                        // Priority 1: Try scripts_base_path if provided
                        let scripts_base_guard = self.current_scripts_base_path.lock().await;
                        if let Some(ref base_path) = *scripts_base_guard {
                            tracing::info!(
                                "[SCRIPTS_BASE_PATH] Checking scripts_base_path: {}",
                                base_path
                            );
                            let base = std::path::Path::new(base_path);
                            if base.exists() && base.is_dir() {
                                let candidate = base.join(script_file);
                                resolution_attempts
                                    .push(format!("scripts_base_path: {}", candidate.display()));
                                tracing::info!(
                                    "[SCRIPTS_BASE_PATH] Looking for file at: {}",
                                    candidate.display()
                                );
                                if candidate.exists() {
                                    tracing::info!(
                                        "[SCRIPTS_BASE_PATH] ✓ Found in scripts_base_path: {} -> {}",
                                        script_file,
                                        candidate.display()
                                    );
                                    resolved_path = Some(candidate);
                                } else {
                                    tracing::info!(
                                        "[SCRIPTS_BASE_PATH] ✗ Not found in scripts_base_path: {}",
                                        candidate.display()
                                    );
                                }
                            } else {
                                tracing::warn!(
                                    "[SCRIPTS_BASE_PATH] Base path does not exist or is not a directory: {}",
                                    base_path
                                );
                            }
                        } else {
                            tracing::debug!(
                                "[SCRIPTS_BASE_PATH] No scripts_base_path configured for this workflow"
                            );
                        }
                        drop(scripts_base_guard);

                        // Priority 2: Try workflow directory if not found yet
                        if resolved_path.is_none() {
                            let workflow_dir_guard = self.current_workflow_dir.lock().await;
                            if let Some(ref workflow_dir) = *workflow_dir_guard {
                                tracing::info!(
                                    "[SCRIPTS_BASE_PATH] Checking workflow directory: {}",
                                    workflow_dir.display()
                                );
                                let candidate = workflow_dir.join(script_file);
                                resolution_attempts
                                    .push(format!("workflow_dir: {}", candidate.display()));
                                tracing::info!(
                                    "[SCRIPTS_BASE_PATH] Looking for file at: {}",
                                    candidate.display()
                                );
                                if candidate.exists() {
                                    tracing::info!(
                                        "[SCRIPTS_BASE_PATH] ✓ Found in workflow directory: {} -> {}",
                                        script_file,
                                        candidate.display()
                                    );
                                    resolved_path = Some(candidate);
                                } else {
                                    tracing::info!(
                                        "[SCRIPTS_BASE_PATH] ✗ Not found in workflow directory: {}",
                                        candidate.display()
                                    );
                                }
                            } else {
                                tracing::debug!(
                                    "[SCRIPTS_BASE_PATH] No workflow directory available"
                                );
                            }
                        }

                        // Priority 3: Check current directory or use as-is
                        if resolved_path.is_none() {
                            let candidate = script_path.to_path_buf();
                            resolution_attempts.push(format!("as-is: {}", candidate.display()));

                            // Check if file exists before using it
                            if candidate.exists() {
                                tracing::info!(
                                    "[SCRIPTS_BASE_PATH] Found script file at: {}",
                                    candidate.display()
                                );
                                resolved_path = Some(candidate);
                            } else {
                                tracing::warn!(
                                    "[SCRIPTS_BASE_PATH] Script file not found: {} (tried: {:?})",
                                    script_file,
                                    resolution_attempts
                                );
                                // Return error immediately for missing file
                                return Err(McpError::invalid_params(
                                    format!("Script file '{script_file}' not found"),
                                    Some(json!({
                                        "file": script_file,
                                        "resolution_attempts": resolution_attempts,
                                        "error": "File does not exist"
                                    })),
                                ));
                            }
                        }
                    } else {
                        // Absolute path - check if exists
                        let candidate = script_path.to_path_buf();
                        if candidate.exists() {
                            tracing::info!("[run_command] Using absolute path: {}", script_file);
                            resolved_path = Some(candidate);
                        } else {
                            tracing::warn!(
                                "[run_command] Absolute script file not found: {}",
                                script_file
                            );
                            return Err(McpError::invalid_params(
                                format!("Script file '{script_file}' not found"),
                                Some(json!({
                                    "file": script_file,
                                    "error": "File does not exist at absolute path"
                                })),
                            ));
                        }
                    }

                    resolved_path.unwrap()
                };

                // Store the resolved path for later use
                resolved_script_path = Some(resolved_path.clone());

                // Read script from resolved file path
                tokio::fs::read_to_string(&resolved_path)
                    .await
                    .map_err(|e| {
                        McpError::invalid_params(
                            "Failed to read script file",
                            Some(json!({
                                "file": script_file,
                                "resolved_path": resolved_path.to_string_lossy(),
                                "error": e.to_string()
                            })),
                        )
                    })?
            } else if let Some(run) = &args.run {
                run.clone()
            } else {
                return Err(McpError::invalid_params(
                    "Either 'run' or 'script_file' must be provided when using 'engine'",
                    None,
                ));
            };

            // Build final script with env injection if provided
            let mut final_script = String::new();

            // Extract workflow variables and accumulated env from special env keys
            let mut variables_json = "{}".to_string();
            let mut accumulated_env_json = "{}".to_string();
            let mut env_data = args.env.clone();

            if let Some(env) = &env_data {
                if let Some(env_obj) = env.as_object() {
                    // Extract workflow variables
                    if let Some(vars) = env_obj.get("_workflow_variables") {
                        variables_json =
                            serde_json::to_string(vars).unwrap_or_else(|_| "{}".to_string());
                    }
                    // Extract accumulated env
                    if let Some(acc_env) = env_obj.get("_accumulated_env") {
                        accumulated_env_json =
                            serde_json::to_string(acc_env).unwrap_or_else(|_| "{}".to_string());
                    }
                }
            }

            // Remove special keys from env before normal processing
            if let Some(env) = &mut env_data {
                if let Some(env_obj) = env.as_object_mut() {
                    env_obj.remove("_workflow_variables");
                    env_obj.remove("_accumulated_env");
                }
            }

            // Prepare explicit env if provided
            let explicit_env_json = if let Some(env) = &env_data {
                if env.as_object().is_some_and(|o| !o.is_empty()) {
                    serde_json::to_string(&env).map_err(|e| {
                        McpError::internal_error(
                            "Failed to serialize env data",
                            Some(json!({"error": e.to_string()})),
                        )
                    })?
                } else {
                    "{}".to_string()
                }
            } else {
                "{}".to_string()
            };

            // Inject based on engine type
            if matches!(
                engine.as_str(),
                "node" | "bun" | "javascript" | "js" | "typescript" | "ts"
            ) {
                // First inject accumulated env
                final_script.push_str(&format!("var env = {accumulated_env_json};\n"));

                // Merge explicit env if provided
                if explicit_env_json != "{}" {
                    final_script
                        .push_str(&format!("env = Object.assign(env, {explicit_env_json});\n"));
                }

                // Inject individual variables from env
                let merged_env = if explicit_env_json != "{}" {
                    // Merge accumulated and explicit env for individual vars
                    format!("Object.assign({{}}, {accumulated_env_json}, {explicit_env_json})")
                } else {
                    accumulated_env_json.clone()
                };

                if let Ok(env_obj) =
                    serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&merged_env)
                {
                    for (key, value) in env_obj {
                        if Self::is_valid_js_identifier(&key) {
                            // Smart handling of potentially double-stringified JSON (same as browser scripts)
                            let injectable_value = if let Some(str_val) = value.as_str() {
                                let trimmed = str_val.trim();
                                // Check if it looks like JSON (object or array)
                                if (trimmed.starts_with('{') && trimmed.ends_with('}'))
                                    || (trimmed.starts_with('[') && trimmed.ends_with(']'))
                                {
                                    // Try to parse as JSON to avoid double stringification
                                    match serde_json::from_str::<serde_json::Value>(str_val) {
                                        Ok(parsed) => {
                                            tracing::debug!(
                                                "[run_command] Detected JSON string for env.{}, parsing to avoid double stringification",
                                                key
                                            );
                                            parsed
                                        }
                                        Err(_) => value.clone(),
                                    }
                                } else {
                                    value.clone()
                                }
                            } else {
                                value.clone()
                            };

                            // Now stringify for injection (single level of stringification)
                            if let Ok(value_json) = serde_json::to_string(&injectable_value) {
                                final_script.push_str(&format!("var {key} = {value_json};\n"));
                                tracing::debug!(
                                    "[run_command] Injected env.{} as individual variable",
                                    key
                                );
                            }
                        }
                    }
                }

                // Inject variables
                final_script.push_str(&format!("var variables = {variables_json};\n"));
                tracing::debug!("[run_command] Injected accumulated env, explicit env, individual vars, and workflow variables for JavaScript");
            } else if matches!(engine.as_str(), "python" | "py") {
                // For Python, inject as dictionaries
                final_script.push_str(&format!("env = {accumulated_env_json}\n"));

                // Merge explicit env if provided
                if explicit_env_json != "{}" {
                    final_script.push_str(&format!("env.update({explicit_env_json})\n"));
                }

                // Inject individual variables from env
                let merged_env = if explicit_env_json != "{}" {
                    // For Python, we need to merge differently
                    let mut base: serde_json::Map<String, serde_json::Value> =
                        serde_json::from_str(&accumulated_env_json).unwrap_or_default();
                    if let Ok(explicit) = serde_json::from_str::<
                        serde_json::Map<String, serde_json::Value>,
                    >(&explicit_env_json)
                    {
                        base.extend(explicit);
                    }
                    serde_json::to_string(&base).unwrap_or_else(|_| "{}".to_string())
                } else {
                    accumulated_env_json.clone()
                };

                if let Ok(env_obj) =
                    serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&merged_env)
                {
                    for (key, value) in env_obj {
                        if Self::is_valid_js_identifier(&key) {
                            // Smart handling of potentially double-stringified JSON (same as browser/JS scripts)
                            let injectable_value = if let Some(str_val) = value.as_str() {
                                let trimmed = str_val.trim();
                                // Check if it looks like JSON (object or array)
                                if (trimmed.starts_with('{') && trimmed.ends_with('}'))
                                    || (trimmed.starts_with('[') && trimmed.ends_with(']'))
                                {
                                    // Try to parse as JSON to avoid double stringification
                                    match serde_json::from_str::<serde_json::Value>(str_val) {
                                        Ok(parsed) => {
                                            tracing::debug!(
                                                "[run_command] Detected JSON string for env.{}, parsing to avoid double stringification",
                                                key
                                            );
                                            parsed
                                        }
                                        Err(_) => value.clone(),
                                    }
                                } else {
                                    value.clone()
                                }
                            } else {
                                value.clone()
                            };

                            // Now stringify for injection (single level of stringification)
                            if let Ok(value_json) = serde_json::to_string(&injectable_value) {
                                final_script.push_str(&format!("{key} = {value_json}\n"));
                                tracing::debug!(
                                    "[run_command] Injected env.{} as individual variable",
                                    key
                                );
                            }
                        }
                    }
                }

                final_script.push_str(&format!("variables = {variables_json}\n"));
                tracing::debug!("[run_command] Injected accumulated env, explicit env, individual vars, and workflow variables for Python");
            }

            // Append the actual script
            final_script.push_str(&script_content);

            // Map engine to executor
            let is_js = matches!(engine.as_str(), "node" | "bun" | "javascript" | "js");
            let is_ts = matches!(engine.as_str(), "typescript" | "ts");
            let is_py = matches!(engine.as_str(), "python" | "py");

            if is_js {
                // Determine the working directory for script execution
                let script_working_dir = if let Some(ref script_path) = resolved_script_path {
                    // When using script_file with scripts_base_path, change working dir to script's directory
                    let scripts_base_guard = self.current_scripts_base_path.lock().await;
                    if scripts_base_guard.is_some() {
                        // Use the resolved script path's parent directory
                        script_path.parent().map(|p| p.to_path_buf())
                    } else {
                        None
                    }
                } else {
                    None
                };

                let execution_future = scripting_engine::execute_javascript_with_nodejs(
                    final_script,
                    cancellation_token,
                    script_working_dir,
                );

                let execution_result = if timeout_ms == 0 {
                    execution_future.await?
                } else {
                    match tokio::time::timeout(timeout_duration, execution_future).await {
                        Ok(result) => result?,
                        Err(_) => {
                            return Err(McpError::internal_error(
                                "JavaScript execution timed out",
                                Some(json!({
                                    "reason": format!("Execution exceeded timeout of {}ms", timeout_ms),
                                    "engine": "javascript",
                                    "timeout_ms": timeout_ms
                                })),
                            ));
                        }
                    }
                };

                // Extract logs, stderr, and actual result
                let logs = execution_result.get("logs").cloned();
                let stderr = execution_result.get("stderr").cloned();
                let actual_result = execution_result
                    .get("result")
                    .cloned()
                    .unwrap_or(execution_result.clone());

                // Debug log extraction
                if let Some(ref log_array) = logs {
                    if let Some(arr) = log_array.as_array() {
                        info!(
                            "[run_command] Extracted {} log lines from JavaScript execution",
                            arr.len()
                        );
                    }
                }

                // Check if the JavaScript result indicates a failure
                // This makes run_command consistent with execute_browser_script behavior
                if let Some(obj) = actual_result.as_object() {
                    if let Some(status) = obj.get("status") {
                        if let Some(status_str) = status.as_str() {
                            if status_str == "failed" || status_str == "error" {
                                // Extract error message if provided
                                let message = obj
                                    .get("message")
                                    .and_then(|m| m.as_str())
                                    .unwrap_or("Script returned failure status");

                                info!(
                                    "[run_command] Script returned status: '{}', treating as error",
                                    status_str
                                );

                                // Return an error to trigger fallback_id in workflows
                                return Err(McpError::internal_error(
                                    format!("JavaScript execution failed: {message}"),
                                    Some(actual_result),
                                ));
                            }
                        }
                    }
                }

                // Build response
                let include_logs = args.include_logs.unwrap_or(false);
                let mut response = json!({
                    "action": "run_command",
                    "mode": "engine",
                    "engine": engine,
                    "status": "success",
                    "result": actual_result
                });

                // Conditionally include logs and stderr based on include_logs parameter
                if include_logs {
                    if let Some(logs) = logs {
                        response["logs"] = logs;
                    }
                    if let Some(stderr) = stderr {
                        response["stderr"] = stderr;
                    }
                }

                span.set_status(true, None);
                span.end();

                return Ok(CallToolResult::success(
                    append_monitor_screenshots_if_enabled(
                        &self.desktop,
                        vec![Content::json(response)?],
                        None,
                    )
                    .await,
                ));
            } else if is_ts {
                // Determine the working directory for script execution
                let script_working_dir = if let Some(ref script_path) = resolved_script_path {
                    // When using script_file with scripts_base_path, change working dir to script's directory
                    let scripts_base_guard = self.current_scripts_base_path.lock().await;
                    if scripts_base_guard.is_some() {
                        // Use the resolved script path's parent directory
                        script_path.parent().map(|p| p.to_path_buf())
                    } else {
                        None
                    }
                } else {
                    None
                };

                let execution_future = scripting_engine::execute_typescript_with_nodejs(
                    final_script,
                    cancellation_token,
                    script_working_dir,
                );

                let execution_result = if timeout_ms == 0 {
                    execution_future.await?
                } else {
                    match tokio::time::timeout(timeout_duration, execution_future).await {
                        Ok(result) => result?,
                        Err(_) => {
                            return Err(McpError::internal_error(
                                "TypeScript execution timed out",
                                Some(json!({
                                    "reason": format!("Execution exceeded timeout of {}ms", timeout_ms),
                                    "engine": "typescript",
                                    "timeout_ms": timeout_ms
                                })),
                            ));
                        }
                    }
                };

                // Extract logs, stderr, and actual result (same as JS)
                let logs = execution_result.get("logs").cloned();
                let stderr = execution_result.get("stderr").cloned();
                let actual_result = execution_result
                    .get("result")
                    .cloned()
                    .unwrap_or(execution_result.clone());

                // Check if the TypeScript result indicates a failure
                if let Some(obj) = actual_result.as_object() {
                    if let Some(status) = obj.get("status") {
                        if let Some(status_str) = status.as_str() {
                            if status_str == "failed" || status_str == "error" {
                                // Extract error message if provided
                                let message = obj
                                    .get("message")
                                    .and_then(|m| m.as_str())
                                    .unwrap_or("Script returned failure status");

                                info!(
                                    "[run_command] TypeScript script returned status: '{}', treating as error",
                                    status_str
                                );

                                // Return an error to trigger fallback_id in workflows
                                return Err(McpError::internal_error(
                                    format!("TypeScript execution failed: {message}"),
                                    Some(actual_result),
                                ));
                            }
                        }
                    }
                }

                // Build response
                let include_logs = args.include_logs.unwrap_or(false);
                let mut response = json!({
                    "action": "run_command",
                    "mode": "engine",
                    "engine": engine,
                    "status": "success",
                    "result": actual_result
                });

                // Conditionally include logs and stderr based on include_logs parameter
                if include_logs {
                    if let Some(logs) = logs {
                        response["logs"] = logs;
                    }
                    if let Some(stderr) = stderr {
                        response["stderr"] = stderr;
                    }
                }

                span.set_status(true, None);
                span.end();

                return Ok(CallToolResult::success(
                    append_monitor_screenshots_if_enabled(
                        &self.desktop,
                        vec![Content::json(response)?],
                        None,
                    )
                    .await,
                ));
            } else if is_py {
                // Determine the working directory for script execution
                let script_working_dir = if let Some(ref script_path) = resolved_script_path {
                    // When using script_file with scripts_base_path, change working dir to script's directory
                    let scripts_base_guard = self.current_scripts_base_path.lock().await;
                    if scripts_base_guard.is_some() {
                        // Use the resolved script path's parent directory
                        script_path.parent().map(|p| p.to_path_buf())
                    } else {
                        None
                    }
                } else {
                    None
                };

                let execution_future = scripting_engine::execute_python_with_bindings(
                    final_script,
                    script_working_dir,
                );

                let execution_result = if timeout_ms == 0 {
                    execution_future.await?
                } else {
                    match tokio::time::timeout(timeout_duration, execution_future).await {
                        Ok(result) => result?,
                        Err(_) => {
                            return Err(McpError::internal_error(
                                "Python execution timed out",
                                Some(json!({
                                    "reason": format!("Execution exceeded timeout of {}ms", timeout_ms),
                                    "engine": "python",
                                    "timeout_ms": timeout_ms
                                })),
                            ));
                        }
                    }
                };

                // Extract logs, stderr, and actual result (same structure as JS/TS now)
                let logs = execution_result.get("logs").cloned();
                let stderr = execution_result.get("stderr").cloned();
                let actual_result = execution_result
                    .get("result")
                    .cloned()
                    .unwrap_or(execution_result.clone());

                // Check if the Python result indicates a failure (same as JavaScript)
                if let Some(obj) = actual_result.as_object() {
                    if let Some(status) = obj.get("status") {
                        if let Some(status_str) = status.as_str() {
                            if status_str == "failed" || status_str == "error" {
                                // Extract error message if provided
                                let message = obj
                                    .get("message")
                                    .and_then(|m| m.as_str())
                                    .unwrap_or("Script returned failure status");

                                info!("[run_command] Python script returned status: '{}', treating as error", status_str);

                                // Return an error to trigger fallback_id in workflows
                                return Err(McpError::internal_error(
                                    format!("Python execution failed: {message}"),
                                    Some(actual_result),
                                ));
                            }
                        }
                    }
                }

                // Build response
                let include_logs = args.include_logs.unwrap_or(false);
                let mut response = json!({
                    "action": "run_command",
                    "mode": "engine",
                    "engine": engine,
                    "status": "success",
                    "result": actual_result
                });

                // Conditionally include logs and stderr based on include_logs parameter
                if include_logs {
                    if let Some(logs) = logs {
                        response["logs"] = logs;
                    }
                    if let Some(stderr) = stderr {
                        response["stderr"] = stderr;
                    }
                }

                span.set_status(true, None);
                span.end();

                return Ok(CallToolResult::success(
                    append_monitor_screenshots_if_enabled(
                        &self.desktop,
                        vec![Content::json(response)?],
                        None,
                    )
                    .await,
                ));
            } else {
                return Err(McpError::invalid_params(
                    "Unsupported engine. Use 'node'/'bun'/'javascript'/'typescript'/'ts' or 'python'",
                    Some(json!({"engine": engine_value})),
                ));
            }
        }

        // Shell-based execution path
        // For shell mode, we also support script_file but env is ignored
        let run_str = if let Some(script_file) = &args.script_file {
            // Check that both run and script_file aren't provided
            if args.run.is_some() {
                return Err(McpError::invalid_params(
                    "Cannot specify both 'run' and 'script_file'. Use one or the other.",
                    None,
                ));
            }

            // Read script from file
            // Resolve script file with priority order (same logic as engine mode)
            let resolved_path = {
                let script_path = std::path::Path::new(script_file);
                let mut resolved_path = None;
                let mut resolution_attempts = Vec::new();

                // Only resolve if path is relative
                if script_path.is_relative() {
                    tracing::info!(
                        "[SCRIPTS_BASE_PATH] Resolving relative shell script: '{}'",
                        script_file
                    );

                    // Priority 1: Try scripts_base_path if provided
                    let scripts_base_guard = self.current_scripts_base_path.lock().await;
                    if let Some(ref base_path) = *scripts_base_guard {
                        tracing::info!(
                            "[SCRIPTS_BASE_PATH] Checking scripts_base_path for shell script: {}",
                            base_path
                        );
                        let base = std::path::Path::new(base_path);
                        if base.exists() && base.is_dir() {
                            let candidate = base.join(script_file);
                            resolution_attempts
                                .push(format!("scripts_base_path: {}", candidate.display()));
                            tracing::info!(
                                "[SCRIPTS_BASE_PATH] Looking for shell script at: {}",
                                candidate.display()
                            );
                            if candidate.exists() {
                                tracing::info!(
                                    "[SCRIPTS_BASE_PATH] ✓ Found shell script in scripts_base_path: {} -> {}",
                                    script_file,
                                    candidate.display()
                                );
                                resolved_path = Some(candidate);
                            } else {
                                tracing::info!(
                                    "[SCRIPTS_BASE_PATH] ✗ Shell script not found in scripts_base_path: {}",
                                    candidate.display()
                                );
                            }
                        } else {
                            tracing::warn!(
                                "[SCRIPTS_BASE_PATH] Base path does not exist or is not a directory: {}",
                                base_path
                            );
                        }
                    } else {
                        tracing::debug!(
                            "[SCRIPTS_BASE_PATH] No scripts_base_path configured for shell script"
                        );
                    }
                    drop(scripts_base_guard);

                    // Priority 2: Try workflow directory if not found yet
                    if resolved_path.is_none() {
                        let workflow_dir_guard = self.current_workflow_dir.lock().await;
                        if let Some(ref workflow_dir) = *workflow_dir_guard {
                            let candidate = workflow_dir.join(script_file);
                            resolution_attempts
                                .push(format!("workflow_dir: {}", candidate.display()));
                            if candidate.exists() {
                                tracing::info!(
                                    "[run_command shell] Resolved via workflow directory: {} -> {}",
                                    script_file,
                                    candidate.display()
                                );
                                resolved_path = Some(candidate);
                            }
                        }
                    }

                    // Priority 3: Check current directory or use as-is
                    if resolved_path.is_none() {
                        let candidate = script_path.to_path_buf();
                        resolution_attempts.push(format!("as-is: {}", candidate.display()));

                        // Check if file exists before using it
                        if candidate.exists() {
                            tracing::info!(
                                "[run_command shell] Found script file at: {}",
                                candidate.display()
                            );
                            resolved_path = Some(candidate);
                        } else {
                            tracing::warn!(
                                "[run_command shell] Script file not found: {} (tried: {:?})",
                                script_file,
                                resolution_attempts
                            );
                            // Return error immediately for missing file
                            return Err(McpError::invalid_params(
                                format!("Script file '{script_file}' not found"),
                                Some(json!({
                                    "file": script_file,
                                    "resolution_attempts": resolution_attempts,
                                    "error": "File does not exist"
                                })),
                            ));
                        }
                    }
                } else {
                    // Absolute path - check if exists
                    let candidate = script_path.to_path_buf();
                    if candidate.exists() {
                        tracing::info!("[run_command shell] Using absolute path: {}", script_file);
                        resolved_path = Some(candidate);
                    } else {
                        tracing::warn!(
                            "[run_command shell] Absolute script file not found: {}",
                            script_file
                        );
                        return Err(McpError::invalid_params(
                            format!("Script file '{script_file}' not found"),
                            Some(json!({
                                "file": script_file,
                                "error": "File does not exist at absolute path"
                            })),
                        ));
                    }
                }

                resolved_path.unwrap()
            };

            // Read script from resolved file path
            tokio::fs::read_to_string(&resolved_path)
                .await
                .map_err(|e| {
                    McpError::invalid_params(
                        "Failed to read script file",
                        Some(json!({
                            "file": script_file,
                            "resolved_path": resolved_path.to_string_lossy(),
                            "error": e.to_string()
                        })),
                    )
                })?
        } else if let Some(run) = &args.run {
            run.clone()
        } else {
            return Err(McpError::invalid_params(
                "Either 'run' or 'script_file' must be provided",
                None,
            ));
        };

        // Determine which shell to use based on platform and user preference
        let (windows_cmd, unix_cmd) = if cfg!(target_os = "windows") {
            // On Windows, prepare the command for execution
            let shell = args.shell.as_deref().unwrap_or("powershell");
            let command_with_cd = if let Some(ref cwd) = args.working_directory {
                match shell {
                    "cmd" => format!("cd /d \"{cwd}\" && {run_str}"),
                    "powershell" | "pwsh" => format!("cd '{cwd}'; {run_str}"),
                    _ => run_str.clone(), // For other shells, handle cwd differently
                }
            } else {
                run_str.clone()
            };

            let windows_cmd = match shell {
                "bash" => {
                    // Use Git Bash or WSL bash if available
                    format!("bash -c \"{}\"", command_with_cd.replace('\"', "\\\""))
                }
                "sh" => {
                    // Use sh (might be Git Bash)
                    format!("sh -c \"{}\"", command_with_cd.replace('\"', "\\\""))
                }
                "cmd" => {
                    // Use cmd.exe
                    format!("cmd /c \"{command_with_cd}\"")
                }
                "powershell" | "pwsh" => {
                    // Default to PowerShell on Windows
                    command_with_cd
                }
                _ => {
                    // For any other shell
                    command_with_cd
                }
            };
            (Some(windows_cmd), None)
        } else {
            // On Unix-like systems (Linux, macOS)
            let shell = args.shell.as_deref().unwrap_or("bash");
            let command_with_cd = if let Some(ref cwd) = args.working_directory {
                format!("cd '{cwd}' && {run_str}")
            } else {
                run_str.clone()
            };

            let unix_cmd = match shell {
                "python" => format!("python -c \"{}\"", command_with_cd.replace('\"', "\\\"")),
                "node" => format!("node -e \"{}\"", command_with_cd.replace('\"', "\\\"")),
                _ => command_with_cd, // For bash, sh, zsh, etc.
            };
            (None, Some(unix_cmd))
        };

        // Default timeout: 2 minutes (120000ms), 0 means no timeout
        let timeout_ms = args.timeout_ms.unwrap_or(120_000);
        let command_future = self
            .desktop
            .run_command(windows_cmd.as_deref(), unix_cmd.as_deref());

        let output = if timeout_ms == 0 {
            // No timeout
            command_future.await
        } else {
            // Apply timeout
            match tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), command_future)
                .await
            {
                Ok(result) => result,
                Err(_) => {
                    return Err(McpError::internal_error(
                        "Shell command timed out",
                        Some(json!({
                            "reason": format!("Command exceeded timeout of {}ms", timeout_ms),
                            "command": run_str,
                            "shell": args.shell,
                            "working_directory": args.working_directory,
                            "timeout_ms": timeout_ms
                        })),
                    ));
                }
            }
        }
        .map_err(|e| {
            McpError::internal_error(
                "Failed to run command",
                Some(json!({
                    "reason": e.to_string(),
                    "command": run_str,
                    "shell": args.shell,
                    "working_directory": args.working_directory
                })),
            )
        })?;

        span.set_status(true, None);
        span.end();

        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(json!({
                    "exit_status": output.exit_status,
                    "stdout": output.stdout,
                    "stderr": output.stderr,
                    "command": run_str,
                    "shell": args.shell.unwrap_or_else(|| {
                        if cfg!(target_os = "windows") { "powershell" } else { "bash" }.to_string()
                    }),
                    "working_directory": args.working_directory
                }))?],
                None,
            )
            .await,
        ))
    }

    #[tool(
        description = "Activates the window containing the specified element, bringing it to the foreground."
    )]
    pub async fn activate_element(
        &self,
        Parameters(args): Parameters<ActivateElementArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("activate_element", None);

        // Add comprehensive telemetry attributes
        span.set_attribute("selector", args.selector.selector.clone());
        if let Some(retries) = args.action.retries {
            span.set_attribute("retry.max_attempts", retries.to_string());
        }

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            let flag_value = *in_sequence;
            let should_restore_value = !flag_value;
            tracing::info!(
                "[activate_element] Flag check: in_sequence={}, should_restore={}",
                flag_value,
                should_restore_value
            );
            should_restore_value
        };

        if should_restore {
            tracing::info!(
                "[activate_element] Direct MCP call detected - performing window management"
            );
            let _ = self
                .prepare_window_management(
                    &args.selector.process,
                    None,
                    None,
                    None,
                    &args.window_mgmt,
                )
                .await;
        } else {
            tracing::debug!("[activate_element] In sequence - skipping window management (dispatch_tool handles it)");
        }

        let ((_result, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector.build_full_selector(),
                None, // ActivateElement doesn't have alternative selectors
                args.selector.build_fallback_selectors().as_deref(),
                args.action.timeout_ms,
                args.action.retries,
                |element| async move { element.activate_window() },
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => {
                    // Restore windows before returning error
                    self.restore_window_management(should_restore).await;
                    Err(build_element_not_found_error(
                        &args.selector.build_full_selector(),
                        None,
                        args.selector.build_fallback_selectors().as_deref(),
                        e,
                    ))
                }
            }?;

        let element_info = build_element_info(&element);
        let target_pid = element.process_id().unwrap_or(0);

        // Add verification to check if activation actually worked
        tokio::time::sleep(std::time::Duration::from_millis(500)).await; // Give window system time to respond

        let mut verification;

        // Method 1: Check if target application is now the focused app (most reliable)
        if let Ok(focused_element) = self.desktop.focused_element() {
            if let Ok(focused_pid) = focused_element.process_id() {
                let pid_match = focused_pid == target_pid;
                verification = json!({
                    "activation_verified": pid_match,
                    "verification_method": "process_id_comparison",
                    "target_pid": target_pid,
                    "focused_pid": focused_pid,
                    "pid_match": pid_match
                });

                // Method 2: Also check if the specific element is focused (additional confirmation)
                if pid_match {
                    let element_focused = element.is_focused().unwrap_or(false);
                    if let Some(obj) = verification.as_object_mut() {
                        obj.insert("target_element_focused".to_string(), json!(element_focused));
                    }
                }
            } else {
                verification = json!({
                    "activation_verified": false,
                    "verification_method": "process_id_comparison",
                    "target_pid": target_pid,
                    "error": "Could not get focused element PID"
                });
            }
        } else {
            verification = json!({
                "activation_verified": false,
                "verification_method": "process_id_comparison",
                "target_pid": target_pid,
                "error": "Could not get focused element"
            });
        }

        // Determine final status based on verification
        let verified_success = verification
            .get("activation_verified")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let final_status = if verified_success {
            "success"
        } else {
            "success_unverified"
        };

        let recommendation = if verified_success {
            "Window activated and verified successfully. The target application is now in the foreground."
        } else {
            "Window activation was called but could not be verified. The target application may not be in the foreground."
        };

        let mut result_json = json!({
            "action": "activate_element",
            "status": final_status,
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector.build_full_selector(), None, args.selector.build_fallback_selectors().as_deref()),
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "verification": verification,
            "recommendation": recommendation
        });

        // Always attach UI tree for activated elements to help with next actions
        maybe_attach_tree(
            &self.desktop,
            args.tree.include_tree_after_action,
            args.tree.tree_max_depth,
            args.tree.tree_from_selector.as_deref(),
            args.tree.include_detailed_attributes,
            None,
            Some(element.process_id().unwrap_or(0)),
            &mut result_json,
            Some(&element),
            false,
        )
        .await;

        // Restore windows after activation
        self.restore_window_management(should_restore).await;

        span.set_status(true, None);
        span.end();

        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(result_json)?],
                None,
            )
            .await,
        ))
    }

    #[tool(
        description = "Delays execution for a specified number of milliseconds. Useful for waiting between actions to ensure UI stability."
    )]
    async fn delay(
        &self,
        Parameters(args): Parameters<DelayArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("delay", None);

        // Add comprehensive telemetry attributes
        span.set_attribute("delay_ms", args.delay_ms.to_string());
        let start_time = chrono::Utc::now();

        // Use tokio's sleep for async delay
        tokio::time::sleep(std::time::Duration::from_millis(args.delay_ms)).await;

        let end_time = chrono::Utc::now();
        let actual_delay_ms = (end_time - start_time).num_milliseconds();

        span.set_status(true, None);
        span.end();

        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(json!({
                    "action": "delay",
                    "status": "success",
                    "requested_delay_ms": args.delay_ms,
                    "actual_delay_ms": actual_delay_ms,
                    "timestamp": end_time.to_rfc3339()
                }))?],
                None,
            )
            .await,
        ))
    }

    #[tool(
        description = "Performs a mouse drag operation from start to end coordinates. This action requires the application to be focused and may change the UI."
    )]
    async fn mouse_drag(
        &self,
        Parameters(args): Parameters<MouseDragArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("mouse_drag", None);

        // Add comprehensive telemetry attributes
        span.set_attribute("selector", args.selector.selector.clone());
        // Mouse drag uses x,y coordinates, not selectors
        if let Some(retries) = args.action.retries {
            span.set_attribute("retry.max_attempts", retries.to_string());
        }

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            !*in_sequence
        };

        if should_restore {
            let _ = self
                .prepare_window_management(
                    &args.selector.process,
                    None,
                    None,
                    None,
                    &args.window_mgmt,
                )
                .await;
        }

        let action = |element: UIElement| async move {
            element.mouse_drag(args.start_x, args.start_y, args.end_x, args.end_y)
        };

        let ((_result, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector.build_full_selector(),
                args.selector.build_alternative_selectors().as_deref(),
                args.selector.build_fallback_selectors().as_deref(),
                args.action.timeout_ms,
                args.action.retries,
                action,
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector.build_full_selector(),
                    args.selector.build_alternative_selectors().as_deref(),
                    args.selector.build_fallback_selectors().as_deref(),
                    e,
                )),
            }?;

        let element_info = build_element_info(&element);

        let mut result_json = json!({
            "action": "mouse_drag",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector.build_full_selector(), args.selector.build_alternative_selectors().as_deref(), args.selector.build_fallback_selectors().as_deref()),
            "start": (args.start_x, args.start_y),
            "end": (args.end_x, args.end_y),
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        // POST-ACTION VERIFICATION
        if !args.action.verify_element_exists.is_empty()
            || !args.action.verify_element_not_exists.is_empty()
        {
            let verify_timeout_ms = args.action.verify_timeout_ms.unwrap_or(2000);

            let verify_exists_opt = if args.action.verify_element_exists.is_empty() {
                None
            } else {
                Some(args.action.verify_element_exists.as_str())
            };
            let verify_not_exists_opt = if args.action.verify_element_not_exists.is_empty() {
                None
            } else {
                Some(args.action.verify_element_not_exists.as_str())
            };

            match crate::helpers::verify_post_action(
                &self.desktop,
                &element,
                verify_exists_opt,
                verify_not_exists_opt,
                verify_timeout_ms,
                &successful_selector,
            )
            .await
            {
                Ok(verification_result) => {
                    tracing::info!(
                        "[mouse_drag] Verification passed: method={}, details={}",
                        verification_result.method,
                        verification_result.details
                    );
                    span.set_attribute("verification.passed", "true".to_string());
                    span.set_attribute("verification.method", verification_result.method.clone());
                    span.set_attribute(
                        "verification.elapsed_ms",
                        verification_result.elapsed_ms.to_string(),
                    );

                    let verification_json = json!({
                        "passed": verification_result.passed,
                        "method": verification_result.method,
                        "details": verification_result.details,
                        "elapsed_ms": verification_result.elapsed_ms,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    });

                    if let Some(obj) = result_json.as_object_mut() {
                        obj.insert("verification".to_string(), verification_json);
                    }
                }
                Err(e) => {
                    tracing::error!("[mouse_drag] Verification failed: {}", e);
                    span.set_attribute("verification.passed", "false".to_string());
                    span.set_status(false, Some("Verification failed"));
                    span.end();
                    return Err(McpError::internal_error(
                        format!("Post-action verification failed: {e}"),
                        Some(json!({
                            "selector_used": successful_selector,
                            "verify_exists": args.action.verify_element_exists,
                            "verify_not_exists": args.action.verify_element_not_exists,
                            "timeout_ms": verify_timeout_ms,
                        })),
                    ));
                }
            }
        }

        // Action tools only support UI diff, not standalone tree attachment

        self.restore_window_management(should_restore).await;

        span.set_status(true, None);
        span.end();

        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(result_json)?],
                None,
            )
            .await,
        ))
    }

    #[tool(
        name = "click_element_by_index",
        description = "Clicks on an indexed item by its index number. First call get_window_tree to get indexed UI elements (shown as #1, #2, etc. in the tree output). By default clicks UI tree elements (vision_type='ui_tree'). Also supports 'ocr' (include_ocr=true), 'omniparser' (include_omniparser=true), and 'dom' (browser_dom field in browser windows) indices. Supports click types: 'left' (default), 'double', or 'right'."
    )]
    async fn click_element_by_index(
        &self,
        Parameters(args): Parameters<crate::utils::ClickIndexArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("click_index", None);
        span.set_attribute("index", args.index.to_string());
        span.set_attribute("vision_type", format!("{:?}", args.vision_type));
        span.set_attribute("click_type", format!("{:?}", args.click_type));

        // Get bounds based on vision type
        let (item_label, bounds) = match args.vision_type {
            crate::utils::VisionType::UiTree => {
                // Look up the UIA bounds
                let bounds_result = {
                    let bounds = self.uia_bounds.lock().map_err(|e| {
                        McpError::internal_error(format!("Failed to lock UIA bounds: {e}"), None)
                    })?;
                    bounds.get(&args.index).cloned()
                };

                let Some((role, name, (x, y, width, height))) = bounds_result else {
                    span.set_status(false, Some("UIA index not found"));
                    span.end();
                    return Err(McpError::internal_error(
                        format!("UI tree index {} not found. Call get_window_tree first to get indexed UI elements.", args.index),
                        Some(json!({ "index": args.index, "vision_type": "ui_tree" })),
                    ));
                };

                let label = if name.is_empty() {
                    role
                } else {
                    format!("{role}: {name}")
                };
                (label, (x, y, width, height))
            }
            crate::utils::VisionType::Ocr => {
                // Look up the OCR bounds
                let bounds_result = {
                    let bounds = self.ocr_bounds.lock().map_err(|e| {
                        McpError::internal_error(format!("Failed to lock OCR bounds: {e}"), None)
                    })?;
                    bounds.get(&args.index).cloned()
                };

                let Some((text, (x, y, width, height))) = bounds_result else {
                    span.set_status(false, Some("OCR index not found"));
                    span.end();
                    return Err(McpError::internal_error(
                        format!("OCR index {} not found. Call get_window_tree with include_ocr=true first to get indexed OCR words.", args.index),
                        Some(json!({ "index": args.index, "vision_type": "ocr" })),
                    ));
                };

                (text, (x, y, width, height))
            }
            crate::utils::VisionType::Omniparser => {
                // Look up the Omniparser item
                let item_result = {
                    let items = self.omniparser_items.lock().map_err(|e| {
                        McpError::internal_error(
                            format!("Failed to lock Omniparser items: {e}"),
                            None,
                        )
                    })?;
                    items.get(&args.index).cloned()
                };

                let Some(item) = item_result else {
                    span.set_status(false, Some("Omniparser index not found"));
                    span.end();
                    return Err(McpError::internal_error(
                        format!(
                            "Omniparser index {} not found. Call get_window_tree with include_omniparser=true first to get indexed items.",
                            args.index
                        ),
                        Some(json!({ "index": args.index, "vision_type": "omniparser" })),
                    ));
                };

                let bounds = item
                    .box_2d
                    .ok_or_else(|| McpError::internal_error("Item has no bounds", None))?;

                let x = bounds[0];
                let y = bounds[1];
                let width = bounds[2] - bounds[0];
                let height = bounds[3] - bounds[1];

                (item.label, (x, y, width, height))
            }
            crate::utils::VisionType::Dom => {
                // Look up the DOM bounds
                let bounds_result = {
                    let bounds = self.dom_bounds.lock().map_err(|e| {
                        McpError::internal_error(format!("Failed to lock DOM bounds: {e}"), None)
                    })?;
                    bounds.get(&args.index).cloned()
                };

                let Some((tag, identifier, (x, y, width, height))) = bounds_result else {
                    span.set_status(false, Some("DOM index not found"));
                    span.end();
                    return Err(McpError::internal_error(
                        format!("DOM index {} not found. Call get_window_tree on a browser first to get indexed DOM elements.", args.index),
                        Some(json!({ "index": args.index, "vision_type": "dom" })),
                    ));
                };

                let label = if identifier.is_empty() {
                    tag
                } else {
                    format!("{tag}: {identifier}")
                };
                (label, (x, y, width, height))
            }
        };

        // Calculate center of the bounds
        let click_x = bounds.0 + bounds.2 / 2.0;
        let click_y = bounds.1 + bounds.3 / 2.0;

        span.set_attribute("label", item_label.clone());
        span.set_attribute("click_x", click_x.to_string());
        span.set_attribute("click_y", click_y.to_string());

        // Convert ClickType to terminator's ClickType
        let terminator_click_type = match args.click_type {
            crate::utils::ClickType::Left => terminator::ClickType::Left,
            crate::utils::ClickType::Double => terminator::ClickType::Double,
            crate::utils::ClickType::Right => terminator::ClickType::Right,
        };

        // Perform the click
        match self
            .desktop
            .click_at_coordinates_with_type(click_x, click_y, terminator_click_type)
        {
            Ok(()) => {
                let vision_type_str = match args.vision_type {
                    crate::utils::VisionType::UiTree => "ui_tree",
                    crate::utils::VisionType::Ocr => "ocr",
                    crate::utils::VisionType::Omniparser => "omniparser",
                    crate::utils::VisionType::Dom => "dom",
                };
                let click_type_str = match args.click_type {
                    crate::utils::ClickType::Left => "left",
                    crate::utils::ClickType::Double => "double",
                    crate::utils::ClickType::Right => "right",
                };
                let result_json = json!({
                    "action": "click_index",
                    "status": "success",
                    "index": args.index,
                    "vision_type": vision_type_str,
                    "click_type": click_type_str,
                    "label": item_label,
                    "clicked_at": { "x": click_x, "y": click_y },
                    "bounds": { "x": bounds.0, "y": bounds.1, "width": bounds.2, "height": bounds.3 },
                });

                span.set_status(true, None);
                span.end();

                Ok(CallToolResult::success(
                    append_monitor_screenshots_if_enabled(
                        &self.desktop,
                        vec![Content::json(result_json)?],
                        args.monitor.include_monitor_screenshots,
                    )
                    .await,
                ))
            }
            Err(e) => {
                span.set_status(false, Some(&e.to_string()));
                span.end();

                Err(McpError::internal_error(
                    format!(
                        "Failed to click index {} (\"{}\"): {e}",
                        args.index, item_label
                    ),
                    Some(json!({
                        "index": args.index,
                        "vision_type": format!("{:?}", args.vision_type).to_lowercase(),
                        "label": item_label,
                    })),
                ))
            }
        }
    }

    #[tool(
        description = "Validates that an element exists and provides detailed information about it. This is a read-only operation that NEVER throws errors. Returns status='success' with exists=true when found, or status='failed' with exists=false when not found. Use {step_id}_status or {step_id}_result.exists for conditional logic. This is the preferred tool for checking optional/conditional UI elements."
    )]
    pub async fn validate_element(
        &self,
        Parameters(args): Parameters<ValidateElementArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("validate_element", None);

        // Add comprehensive telemetry attributes
        span.set_attribute("selector", args.selector.selector.clone());
        if let Some(timeout) = args.action.timeout_ms {
            span.set_attribute("timeout_ms", timeout.to_string());
        }
        if let Some(retries) = args.action.retries {
            span.set_attribute("retry.max_attempts", retries.to_string());
        }

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            !*in_sequence
        };

        if should_restore {
            tracing::info!(
                "[validate_element] Direct MCP call detected - performing window management"
            );
            let _ = self
                .prepare_window_management(
                    &args.selector.process,
                    None,
                    None,
                    None,
                    &args.window_mgmt,
                )
                .await;
        } else {
            tracing::debug!("[validate_element] In sequence - skipping window management (dispatch_tool handles it)");
        }

        // For validation, the "action" is just succeeding.
        let action = |element: UIElement| async move { Ok(element) };

        let operation_start = std::time::Instant::now();
        match find_and_execute_with_retry_with_fallback(
            &self.desktop,
            &args.selector.build_full_selector(),
            args.selector.build_alternative_selectors().as_deref(),
            args.selector.build_fallback_selectors().as_deref(),
            args.action.timeout_ms,
            args.action.retries,
            action,
        )
        .await
        {
            Ok(((element, _), successful_selector)) => {
                let operation_time_ms = operation_start.elapsed().as_millis() as i64;
                span.set_attribute("operation.duration_ms", operation_time_ms.to_string());
                span.set_attribute("element.found", "true".to_string());
                span.set_attribute("selector.successful", successful_selector.clone());

                // Add element metadata
                if let Some(name) = element.name() {
                    span.set_attribute("element.name", name);
                }
                let mut element_info = build_element_info(&element);
                if let Some(obj) = element_info.as_object_mut() {
                    obj.insert("exists".to_string(), json!(true));
                }

                let mut result_json = json!({
                    "action": "validate_element",
                    "status": "success",
                    "element": element_info,
                    "selector_used": successful_selector,
                    "selectors_tried": get_selectors_tried_all(&args.selector.build_full_selector(), args.selector.build_alternative_selectors().as_deref(), args.selector.build_fallback_selectors().as_deref()),
                    "timestamp": chrono::Utc::now().to_rfc3339()
                });
                maybe_attach_tree(
                    &self.desktop,
                    args.tree.include_tree_after_action,
                    args.tree.tree_max_depth,
                    args.tree.tree_from_selector.as_deref(),
                    args.tree.include_detailed_attributes,
                    None,
                    element.process_id().ok(),
                    &mut result_json,
                    Some(&element),
                    false,
                )
                .await;

                self.restore_window_management(should_restore).await;

                span.set_status(true, None);
                span.end();

                Ok(CallToolResult::success(
                    append_monitor_screenshots_if_enabled(
                        &self.desktop,
                        vec![Content::json(result_json)?],
                        None,
                    )
                    .await,
                ))
            }
            Err(e) => {
                let selectors_tried = get_selectors_tried_all(
                    &args.selector.build_full_selector(),
                    args.selector.build_alternative_selectors().as_deref(),
                    args.selector.build_fallback_selectors().as_deref(),
                );
                let reason_payload = json!({
                    "error_type": "ElementNotFound",
                    "message": format!("The specified element could not be found after trying all selectors. Original error: {}", e),
                    "selectors_tried": selectors_tried,
                    "suggestions": [
                        "This is normal if the element is optional/conditional. Use the 'exists: false' result in conditional logic (if expressions, jumps, or run_command scripts).",
                        "Call `get_window_tree` again to get a fresh view of the UI; it might have changed.",
                        "Verify the element's 'name' and 'role' in the new UI tree. The 'name' attribute might be empty or different from the visible text.",
                        "If the element has no 'name', use its numeric ID selector (e.g., '#12345').",
                        "Consider using alternative_selectors or fallback_selectors for elements with multiple possible states."
                    ]
                });

                // This is not a tool error, but a validation failure, so we return success with the failure info.

                self.restore_window_management(should_restore).await;

                span.set_attribute("element.found", "false".to_string());
                span.set_status(true, None);
                span.end();

                Ok(CallToolResult::success(
                    append_monitor_screenshots_if_enabled(
                        &self.desktop,
                        vec![Content::json(json!({
                            "action": "validate_element",
                            "status": "failed",
                            "exists": false,
                            "reason": reason_payload,
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }))?],
                        None,
                    )
                    .await,
                ))
            }
        }
    }

    #[tool(description = "Highlights an element with a colored border for visual confirmation.")]
    async fn highlight_element(
        &self,
        Parameters(args): Parameters<HighlightElementArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("highlight_element", None);

        // Add comprehensive telemetry attributes
        span.set_attribute("selector", args.selector.selector.clone());
        if let Some(ref color) = args.color {
            span.set_attribute("color", format!("#{color:08X}"));
        }
        if let Some(retries) = args.action.retries {
            span.set_attribute("retry.max_attempts", retries.to_string());
        }

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            !*in_sequence
        };

        if should_restore {
            tracing::info!(
                "[highlight_element] Direct MCP call detected - performing window management"
            );
            let _ = self
                .prepare_window_management(
                    &args.selector.process,
                    None,
                    None,
                    None,
                    &args.window_mgmt,
                )
                .await;
        } else {
            tracing::debug!("[highlight_element] In sequence - skipping window management (dispatch_tool handles it)");
        }

        let duration = args.duration_ms.map(std::time::Duration::from_millis);
        let color = args.color;

        let text = args.text.as_deref();

        #[cfg(target_os = "windows")]
        let text_position = args.text_position.clone().map(|pos| pos.into());
        #[cfg(not(target_os = "windows"))]
        let text_position = None;

        #[cfg(target_os = "windows")]
        let font_style =
            if args.font_size.is_some() || args.font_bold.is_some() || args.font_color.is_some() {
                Some(terminator::platforms::windows::FontStyle {
                    size: args.font_size.unwrap_or(14),
                    bold: args.font_bold.unwrap_or(false),
                    color: args.font_color.unwrap_or(0),
                })
            } else {
                None
            };
        #[cfg(not(target_os = "windows"))]
        let font_style = None;

        let action = {
            move |element: UIElement| {
                let color = color;
                let local_duration = duration;
                let local_text_position = text_position;
                let local_font_style = font_style.clone();
                async move {
                    let handle = element.highlight(
                        color,
                        local_duration,
                        text,
                        local_text_position,
                        local_font_style,
                    )?;
                    Ok(handle)
                }
            }
        };

        // Use a shorter default timeout for highlight to avoid long waits
        let effective_timeout_ms = args.action.timeout_ms.or(Some(1000));

        let ((handle, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector.build_full_selector(),
                args.selector.build_alternative_selectors().as_deref(),
                args.selector.build_fallback_selectors().as_deref(),
                effective_timeout_ms,
                args.action.retries,
                action,
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => {
                    // Restore windows before returning error
                    self.restore_window_management(should_restore).await;
                    Err(build_element_not_found_error(
                        &args.selector.build_full_selector(),
                        args.selector.build_alternative_selectors().as_deref(),
                        args.selector.build_fallback_selectors().as_deref(),
                        e,
                    ))
                }
            }?;

        // Register handle and schedule cleanup
        {
            let mut list = self.active_highlights.lock().await;
            list.push(handle);
        }
        let active_highlights_clone = self.active_highlights.clone();
        let expire_after = args.duration_ms.unwrap_or(1000);
        tokio::spawn(
            async move {
                tokio::time::sleep(Duration::from_millis(expire_after)).await;
                let mut list = active_highlights_clone.lock().await;
                let _ = list.pop();
            }
            .in_current_span(),
        );

        // Build minimal response by default; gate heavy element info behind flag
        let mut result_json = json!({
            "action": "highlight_element",
            "status": "success",
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector.build_full_selector(), args.selector.build_alternative_selectors().as_deref(), args.selector.build_fallback_selectors().as_deref()),
            "color": args.color.unwrap_or(0x0000FF),
            "duration_ms": args.duration_ms.unwrap_or(1000),
            "visibility": { "requested_ms": args.duration_ms.unwrap_or(1000) },
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        if args.include_element_info.unwrap_or(false) {
            let element_info = build_element_info(&element);
            result_json["element"] = element_info;
        }
        // Action tools only support UI diff, not standalone tree attachment

        self.restore_window_management(should_restore).await;

        span.set_status(true, None);
        span.end();

        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(result_json)?],
                None,
            )
            .await,
        ))
    }

    #[tool(
        description = "Hide any active inspect overlay that was shown via get_window_tree with show_overlay parameter."
    )]
    async fn hide_inspect_overlay(&self) -> Result<CallToolResult, McpError> {
        #[cfg(target_os = "windows")]
        {
            // Use the stored handle to close the overlay (thread-safe via atomic flag)
            if let Ok(mut handle) = self.inspect_overlay_handle.lock() {
                if let Some(h) = handle.take() {
                    h.close();
                    info!("Closed inspect overlay via handle");
                }
            }
        }

        Ok(CallToolResult::success(vec![Content::json(json!({
            "action": "hide_inspect_overlay",
            "status": "success",
            "message": "Inspect overlay hidden"
        }))?]))
    }

    #[tool(
        description = "Waits for an element to meet a specific condition (visible, enabled, focused, exists)."
    )]
    async fn wait_for_element(
        &self,
        Parameters(args): Parameters<WaitForElementArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("wait_for_element", None);

        // Add comprehensive telemetry attributes
        span.set_attribute("selector", args.selector.selector.clone());
        if let Some(retries) = args.action.retries {
            span.set_attribute("retry.max_attempts", retries.to_string());
        }
        info!(
            "[wait_for_element] Called with selector: '{}', condition: '{}', timeout_ms: {:?}, include_tree: {:?}",
            args.selector.selector, args.condition, args.action.timeout_ms, args.tree.include_tree_after_action
        );

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            !*in_sequence
        };

        if should_restore {
            tracing::info!(
                "[wait_for_element] Direct MCP call detected - performing window management"
            );
            let _ = self
                .prepare_window_management(
                    &args.selector.process,
                    None,
                    None,
                    None,
                    &args.window_mgmt,
                )
                .await;
        } else {
            tracing::debug!("[wait_for_element] In sequence - skipping window management (dispatch_tool handles it)");
        }

        let locator = self
            .desktop
            .locator(Selector::from(args.selector.selector.as_str()));
        let timeout = get_timeout(args.action.timeout_ms);
        let condition_lower = args.condition.to_lowercase();

        // For the "exists" condition, we can use the standard wait
        if condition_lower == "exists" {
            info!(
                "[wait_for_element] Waiting for element to exist: selector='{}', timeout={:?}",
                args.selector.selector, timeout
            );
            match locator.wait(timeout).await {
                Ok(element) => {
                    info!(
                        "[wait_for_element] Element found for selector='{}' within timeout.",
                        args.selector.selector
                    );
                    let mut result_json = json!({
                        "action": "wait_for_element",
                        "status": "success",
                        "condition": args.condition,
                        "condition_met": true,
                        "selector": args.selector.selector,
                        "timeout_ms": timeout.unwrap_or(std::time::Duration::from_millis(5000)).as_millis(),
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    });

                    maybe_attach_tree(
                        &self.desktop,
                        args.tree.include_tree_after_action,
                        args.tree.tree_max_depth,
                        args.tree.tree_from_selector.as_deref(),
                        None, // include_detailed_attributes - use default
                        None, // tree_output_format - use default
                        element.process_id().ok(),
                        &mut result_json,
                        Some(&element),
                        false,
                    )
                    .await;

                    self.restore_window_management(should_restore).await;

                    span.set_status(true, None);
                    span.end();

                    return Ok(CallToolResult::success(
                        append_monitor_screenshots_if_enabled(
                            &self.desktop,
                            vec![Content::json(result_json)?],
                            None,
                        )
                        .await,
                    ));
                }
                Err(e) => {
                    let error_msg = format!("Element not found within timeout: {e}");
                    info!(
                        "[wait_for_element] Element NOT found for selector='{}' within timeout. Error: {}",
                        args.selector.selector, e
                    );

                    self.restore_window_management(should_restore).await;

                    return Err(McpError::internal_error(
                        error_msg,
                        Some(json!({
                            "selector": args.selector.selector,
                            "condition": args.condition,
                            "timeout_ms": timeout.unwrap_or(std::time::Duration::from_millis(5000)).as_millis(),
                            "error": e.to_string()
                        })),
                    ));
                }
            }
        }

        // For other conditions (visible, enabled, focused), we need to poll
        let start_time = std::time::Instant::now();
        let timeout_duration = timeout.unwrap_or(std::time::Duration::from_millis(5000));
        info!(
            "[wait_for_element] Polling for condition '{}' on selector='{}' with timeout {:?}",
            args.condition, args.selector.selector, timeout_duration
        );

        loop {
            // Check if we've exceeded the timeout
            if start_time.elapsed() > timeout_duration {
                let timeout_msg = format!(
                    "Timeout waiting for element to be {} within {}ms",
                    args.condition,
                    timeout_duration.as_millis()
                );
                info!(
                    "[wait_for_element] Timeout exceeded for selector='{}', condition='{}', waited {}ms",
                    args.selector.selector, args.condition, start_time.elapsed().as_millis()
                );

                self.restore_window_management(should_restore).await;

                return Err(McpError::internal_error(
                    timeout_msg,
                    Some(json!({
                        "selector": args.selector.selector,
                        "condition": args.condition,
                        "timeout_ms": timeout_duration.as_millis(),
                        "elapsed_ms": start_time.elapsed().as_millis()
                    })),
                ));
            }

            // Try to find the element with a short timeout
            match locator
                .wait(Some(std::time::Duration::from_millis(100)))
                .await
            {
                Ok(element) => {
                    info!(
                        "[wait_for_element] Element found for selector='{}', checking condition '{}'",
                        args.selector.selector, args.condition
                    );
                    // Element exists, now check the specific condition
                    let condition_met = match condition_lower.as_str() {
                        "visible" => {
                            let v = element.is_visible().unwrap_or(false);
                            info!(
                                "[wait_for_element] is_visible() for selector='{}': {}",
                                args.selector.selector, v
                            );
                            v
                        }
                        "enabled" => {
                            let v = element.is_enabled().unwrap_or(false);
                            info!(
                                "[wait_for_element] is_enabled() for selector='{}': {}",
                                args.selector.selector, v
                            );
                            v
                        }
                        "focused" => {
                            let v = element.is_focused().unwrap_or(false);
                            info!(
                                "[wait_for_element] is_focused() for selector='{}': {}",
                                args.selector.selector, v
                            );
                            v
                        }
                        _ => {
                            info!(
                                "[wait_for_element] Invalid condition provided: '{}'",
                                args.condition
                            );

                            self.restore_window_management(should_restore).await;

                            return Err(McpError::invalid_params(
                                "Invalid condition. Valid: exists, visible, enabled, focused",
                                Some(json!({"provided_condition": args.condition})),
                            ));
                        }
                    };

                    if condition_met {
                        info!(
                            "[wait_for_element] Condition '{}' met for selector='{}' after {}ms",
                            args.condition,
                            args.selector.selector,
                            start_time.elapsed().as_millis()
                        );
                        // Condition is met, return success
                        let mut result_json = json!({
                            "action": "wait_for_element",
                            "status": "success",
                            "condition": args.condition,
                            "condition_met": true,
                            "selector": args.selector.selector,
                            "timeout_ms": timeout_duration.as_millis(),
                            "elapsed_ms": start_time.elapsed().as_millis(),
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        });

                        maybe_attach_tree(
                            &self.desktop,
                            args.tree.include_tree_after_action,
                            args.tree.tree_max_depth,
                            args.tree.tree_from_selector.as_deref(),
                            None, // include_detailed_attributes - use default
                            None, // tree_output_format - use default
                            element.process_id().ok(),
                            &mut result_json,
                            Some(&element),
                            false,
                        )
                        .await;

                        self.restore_window_management(should_restore).await;

                        span.set_status(true, None);
                        span.end();

                        return Ok(CallToolResult::success(
                            append_monitor_screenshots_if_enabled(
                                &self.desktop,
                                vec![Content::json(result_json)?],
                                None,
                            )
                            .await,
                        ));
                    } else {
                        info!(
                            "[wait_for_element] Condition '{}' NOT met for selector='{}', continuing to poll...",
                            args.condition, args.selector.selector
                        );
                    }
                    // Condition not met yet, continue polling
                }
                Err(_) => {
                    info!(
                        "[wait_for_element] Element not found for selector='{}', will retry...",
                        args.selector.selector
                    );
                    // Element doesn't exist yet, continue polling
                }
            }

            // Wait a bit before the next poll
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }

    #[tool(
        description = "Opens a URL in the specified browser (uses SDK's built-in browser automation). This is the RECOMMENDED method for browser navigation - more reliable than manually manipulating the address bar with keyboard/mouse actions. Handles page loading, waiting, and error recovery automatically. Requires verify_element_exists and verify_element_not_exists parameters (use empty string \"\" to skip verification). Always use include_tree_after_action: true to get the UI tree of elements after navigation."
    )]
    pub async fn navigate_browser(
        &self,
        Parameters(args): Parameters<NavigateBrowserArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("navigate_browser", None);

        // Add comprehensive telemetry attributes
        span.set_attribute("url", args.url.clone());
        span.set_attribute("process", args.process.clone());

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            !*in_sequence
        };

        if should_restore {
            tracing::info!(
                "[navigate_browser] Direct MCP call detected - performing window management"
            );
            let _ = self
                .prepare_window_management(&args.process, None, None, None, &args.window_mgmt)
                .await;
        } else {
            tracing::debug!("[navigate_browser] In sequence - skipping window management (dispatch_tool handles it)");
        }

        let browser = Some(Browser::Custom(args.process.clone()));
        let ui_element = self.desktop.open_url(&args.url, browser).map_err(|e| {
            McpError::internal_error(
                "Failed to open URL",
                Some(json!({"reason": e.to_string(), "url": args.url, "process": args.process})),
            )
        })?;

        let element_info = build_element_info(&ui_element);

        let mut result_json = json!({
            "action": "navigate_browser",
            "status": "success",
            "url": args.url,
            "process": args.process,
            "element": element_info,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        // POST-ACTION VERIFICATION (must happen BEFORE tree capture so page content is loaded)
        if !args.verify_element_exists.is_empty() || !args.verify_element_not_exists.is_empty() {
            let verify_exists_opt = if args.verify_element_exists.is_empty() {
                None
            } else {
                Some(args.verify_element_exists.as_str())
            };
            let verify_not_exists_opt = if args.verify_element_not_exists.is_empty() {
                None
            } else {
                Some(args.verify_element_not_exists.as_str())
            };

            match crate::helpers::verify_post_action(
                &self.desktop,
                &ui_element,
                verify_exists_opt,
                verify_not_exists_opt,
                args.verify_timeout_ms.unwrap_or(2000),
                &args.url,
            )
            .await
            {
                Ok(verification_result) => {
                    tracing::info!(
                        "[navigate_browser] Verification passed: method={}, details={}",
                        verification_result.method,
                        verification_result.details
                    );
                    span.set_attribute("verification.passed", "true".to_string());
                    span.set_attribute("verification.method", verification_result.method.clone());
                    span.set_attribute(
                        "verification.elapsed_ms",
                        verification_result.elapsed_ms.to_string(),
                    );

                    let verification_json = json!({
                        "passed": verification_result.passed,
                        "method": verification_result.method,
                        "details": verification_result.details,
                        "elapsed_ms": verification_result.elapsed_ms,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    });

                    if let Some(obj) = result_json.as_object_mut() {
                        obj.insert("verification".to_string(), verification_json);
                    }
                }
                Err(e) => {
                    tracing::error!("[navigate_browser] Verification failed: {}", e);
                    span.set_attribute("verification.passed", "false".to_string());
                    span.set_status(false, Some("Verification failed"));
                    span.end();
                    return Err(McpError::internal_error(
                        format!("Post-action verification failed: {e}"),
                        None,
                    ));
                }
            }
        }

        // Capture tree AFTER verification passes (page content should be loaded now)
        maybe_attach_tree(
            &self.desktop,
            args.tree.include_tree_after_action,
            args.tree.tree_max_depth,
            args.tree.tree_from_selector.as_deref(),
            args.tree.include_detailed_attributes,
            None,
            ui_element.process_id().ok(),
            &mut result_json,
            Some(&ui_element),
            false,
        )
        .await;

        self.restore_window_management(should_restore).await;

        span.set_status(true, None);
        span.end();

        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(result_json)?],
                None,
            )
            .await,
        ))
    }

    #[tool(
        description = "Opens an application by name (uses SDK's built-in app launcher). Requires verify_element_exists and verify_element_not_exists parameters (use empty string \"\" to skip verification)."
    )]
    pub async fn open_application(
        &self,
        Parameters(args): Parameters<OpenApplicationArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("open_application", None);

        // Add comprehensive telemetry attributes
        span.set_attribute("app_name", args.app_name.clone());

        // Open the application
        let ui_element = self.desktop.open_application(&args.app_name).map_err(|e| {
            McpError::internal_error(
                "Failed to open application",
                Some(json!({"reason": e.to_string(), "app_name": args.app_name})),
            )
        })?;

        let process_id = ui_element.process_id().unwrap_or(0);
        let _window_title = ui_element.window_title();

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            let flag_value = *in_sequence;
            let should_restore_value = !flag_value;
            tracing::info!(
                "[open_application] Flag check: in_sequence={}, should_restore={}",
                flag_value,
                should_restore_value
            );
            should_restore_value
        };

        if should_restore {
            tracing::info!(
                "[open_application] Direct MCP call detected - performing window management"
            );
            // Note: UIElement is a trait object, can't extract platform-specific type, so pass None
            let _ = self
                .prepare_window_management(
                    &args.app_name,
                    None,
                    Some(process_id),
                    None,
                    &args.window_mgmt,
                )
                .await;
        } else {
            tracing::debug!("[open_application] In sequence - skipping window management (dispatch_tool handles it)");
        }

        let element_info = build_element_info(&ui_element);

        let mut result_json = json!({
            "action": "open_application",
            "status": "success",
            "app_name": args.app_name,
            "application": element_info,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        // Attach UI tree if requested
        maybe_attach_tree(
            &self.desktop,
            args.tree.include_tree_after_action,
            args.tree.tree_max_depth,
            args.tree.tree_from_selector.as_deref(),
            args.tree.include_detailed_attributes,
            args.tree.tree_output_format,
            Some(process_id),
            &mut result_json,
            Some(&ui_element),
            false,
        )
        .await;

        // POST-ACTION VERIFICATION
        if !args.verify_element_exists.is_empty() || !args.verify_element_not_exists.is_empty() {
            let verify_exists_opt = if args.verify_element_exists.is_empty() {
                None
            } else {
                Some(args.verify_element_exists.as_str())
            };
            let verify_not_exists_opt = if args.verify_element_not_exists.is_empty() {
                None
            } else {
                Some(args.verify_element_not_exists.as_str())
            };

            match crate::helpers::verify_post_action(
                &self.desktop,
                &ui_element,
                verify_exists_opt,
                verify_not_exists_opt,
                args.verify_timeout_ms.unwrap_or(2000),
                &args.app_name,
            )
            .await
            {
                Ok(verification_result) => {
                    tracing::info!(
                        "[open_application] Verification passed: method={}, details={}",
                        verification_result.method,
                        verification_result.details
                    );
                    span.set_attribute("verification.passed", "true".to_string());
                    span.set_attribute("verification.method", verification_result.method.clone());
                    span.set_attribute(
                        "verification.elapsed_ms",
                        verification_result.elapsed_ms.to_string(),
                    );

                    let verification_json = json!({
                        "passed": verification_result.passed,
                        "method": verification_result.method,
                        "details": verification_result.details,
                        "elapsed_ms": verification_result.elapsed_ms,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    });

                    if let Some(obj) = result_json.as_object_mut() {
                        obj.insert("verification".to_string(), verification_json);
                    }
                }
                Err(e) => {
                    tracing::error!("[open_application] Verification failed: {}", e);
                    span.set_attribute("verification.passed", "false".to_string());
                    span.set_status(false, Some("Verification failed"));
                    span.end();
                    return Err(McpError::internal_error(
                        format!("Post-action verification failed: {e}"),
                        None,
                    ));
                }
            }
        }

        // Restore windows if we did window management
        self.restore_window_management(should_restore).await;

        span.set_status(true, None);
        span.end();

        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(result_json)?],
                None,
            )
            .await,
        ))
    }

    #[tool(description = "Scrolls a UI element in the specified direction by the given amount.")]
    async fn scroll_element(
        &self,
        Parameters(args): Parameters<ScrollElementArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("scroll_element", None);

        // Add comprehensive telemetry attributes
        span.set_attribute("selector", args.selector.selector.clone());
        span.set_attribute("direction", format!("{:?}", args.direction));
        span.set_attribute("amount", args.amount.to_string());
        if let Some(retries) = args.action.retries {
            span.set_attribute("retry.max_attempts", retries.to_string());
        }
        tracing::info!(
            "[scroll_element] Called with selector: '{}', direction: '{}', amount: {}",
            args.selector.selector,
            args.direction,
            args.amount
        );

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            !*in_sequence
        };

        if should_restore {
            tracing::info!(
                "[scroll_element] Direct MCP call detected - performing window management"
            );
            let _ = self
                .prepare_window_management(
                    &args.selector.process,
                    None,
                    None,
                    None,
                    &args.window_mgmt,
                )
                .await;
        } else {
            tracing::debug!("[scroll_element] In sequence - skipping window management (dispatch_tool handles it)");
        }

        let direction = args.direction.clone();
        let amount = args.amount;
        let highlight_before = args.highlight.highlight_before_action;
        let action = {
            move |element: UIElement| {
                let direction = direction.clone();
                async move {
                    // Ensure element is visible and apply highlighting if enabled
                    if highlight_before {
                        Self::ensure_visible_and_apply_highlight(&element, "scroll");
                    }

                    // Execute the scroll action with state tracking
                    element.scroll_with_state(&direction, amount)
                }
            }
        };

        // Store tree config to avoid move issues
        let tree_output_format = args
            .tree
            .tree_output_format
            .unwrap_or(crate::mcp_types::TreeOutputFormat::CompactYaml);

        let ((result, element), successful_selector, ui_diff) =
            match crate::helpers::find_and_execute_with_ui_diff(
                &self.desktop,
                &args.selector.build_full_selector(),
                args.selector.build_alternative_selectors().as_deref(),
                args.selector.build_fallback_selectors().as_deref(),
                args.action.timeout_ms,
                args.action.retries,
                action,
                args.tree.ui_diff_before_after,
                args.tree.tree_max_depth,
                args.tree.include_detailed_attributes,
                tree_output_format,
            )
            .await
            {
                Ok(((result, element), selector, diff)) => {
                    if diff.is_some() {
                        span.set_attribute("ui_diff.captured", "true".to_string());
                    }
                    Ok(((result, element), selector, diff))
                }
                Err(e) => {
                    // Restore windows before returning error
                    self.restore_window_management(should_restore).await;
                    Err(build_element_not_found_error(
                        &args.selector.build_full_selector(),
                        args.selector.build_alternative_selectors().as_deref(),
                        args.selector.build_fallback_selectors().as_deref(),
                        e,
                    ))
                }
            }?;

        let element_info = build_element_info(&element);

        let mut result_json = json!({
            "action": "scroll_element",
            "status": "success",
            "action_result": {
                "action": result.action,
                "details": result.details,
                "data": result.data,
            },
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector.build_full_selector(), args.selector.build_alternative_selectors().as_deref(), args.selector.build_fallback_selectors().as_deref()),
            "direction": args.direction,
            "amount": args.amount,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        // POST-ACTION VERIFICATION
        if !args.action.verify_element_exists.is_empty()
            || !args.action.verify_element_not_exists.is_empty()
        {
            let verify_timeout_ms = args.action.verify_timeout_ms.unwrap_or(2000);

            let verify_exists_opt = if args.action.verify_element_exists.is_empty() {
                None
            } else {
                Some(args.action.verify_element_exists.as_str())
            };
            let verify_not_exists_opt = if args.action.verify_element_not_exists.is_empty() {
                None
            } else {
                Some(args.action.verify_element_not_exists.as_str())
            };

            match crate::helpers::verify_post_action(
                &self.desktop,
                &element,
                verify_exists_opt,
                verify_not_exists_opt,
                verify_timeout_ms,
                &successful_selector,
            )
            .await
            {
                Ok(verification_result) => {
                    tracing::info!(
                        "[scroll_element] Verification passed: method={}, details={}",
                        verification_result.method,
                        verification_result.details
                    );
                    span.set_attribute("verification.passed", "true".to_string());
                    span.set_attribute("verification.method", verification_result.method.clone());
                    span.set_attribute(
                        "verification.elapsed_ms",
                        verification_result.elapsed_ms.to_string(),
                    );

                    let verification_json = json!({
                        "passed": verification_result.passed,
                        "method": verification_result.method,
                        "details": verification_result.details,
                        "elapsed_ms": verification_result.elapsed_ms,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    });

                    if let Some(obj) = result_json.as_object_mut() {
                        obj.insert("verification".to_string(), verification_json);
                    }
                }
                Err(e) => {
                    tracing::error!("[scroll_element] Verification failed: {}", e);
                    span.set_attribute("verification.passed", "false".to_string());
                    span.set_status(false, Some("Verification failed"));
                    span.end();
                    return Err(McpError::internal_error(
                        format!("Post-action verification failed: {e}"),
                        Some(json!({
                            "selector_used": successful_selector,
                            "verify_exists": args.action.verify_element_exists,
                            "verify_not_exists": args.action.verify_element_not_exists,
                            "timeout_ms": verify_timeout_ms,
                        })),
                    ));
                }
            }
        }

        // Attach UI diff if captured
        if let Some(diff_result) = ui_diff {
            tracing::debug!(
                "[scroll_element] Attaching UI diff to result (has_changes: {})",
                diff_result.has_changes
            );
            span.set_attribute("ui_diff.has_changes", diff_result.has_changes.to_string());

            result_json["ui_diff"] = json!(diff_result.diff);
            result_json["has_ui_changes"] = json!(diff_result.has_changes);
            if args.tree.ui_diff_include_full_trees_in_response.unwrap_or(false) {
                result_json["tree_before"] = json!(diff_result.tree_before);
                result_json["tree_after"] = json!(diff_result.tree_after);
            }
        }

        self.restore_window_management(should_restore).await;

        span.set_status(true, None);
        span.end();

        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(result_json)?],
                None,
            )
            .await,
        ))
    }

    #[tool(description = "Selects an option in a dropdown or combobox by its visible text.")]
    async fn select_option(
        &self,
        Parameters(args): Parameters<crate::utils::SelectOptionArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("select_option", None);

        // Add comprehensive telemetry attributes
        span.set_attribute("option_name", args.option_name.clone());
        if let Some(retries) = args.action.retries {
            span.set_attribute("retry.max_attempts", retries.to_string());
        }

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            !*in_sequence
        };

        if should_restore {
            tracing::info!(
                "[select_option] Direct MCP call detected - performing window management"
            );
            let _ = self
                .prepare_window_management(
                    &args.selector.process,
                    None,
                    None,
                    None,
                    &args.window_mgmt,
                )
                .await;
        } else {
            tracing::debug!("[select_option] In sequence - skipping window management (dispatch_tool handles it)");
        }

        let option_name = args.option_name.clone();
        let action = move |element: UIElement| {
            let option_name = option_name.clone();
            async move {
                // Ensure element is visible before interaction
                if let Err(e) = Self::ensure_element_in_view(&element) {
                    tracing::warn!("Failed to ensure element is in view for select_option: {e}");
                }
                element.select_option_with_state(&option_name)
            }
        };

        // Store tree config to avoid move issues
        let tree_output_format = args
            .tree
            .tree_output_format
            .unwrap_or(crate::mcp_types::TreeOutputFormat::CompactYaml);

        let ((result, element), successful_selector, ui_diff) =
            match crate::helpers::find_and_execute_with_ui_diff(
                &self.desktop,
                &args.selector.build_full_selector(),
                args.selector.build_alternative_selectors().as_deref(),
                args.selector.build_fallback_selectors().as_deref(),
                args.action.timeout_ms,
                args.action.retries,
                action,
                args.tree.ui_diff_before_after,
                args.tree.tree_max_depth,
                args.tree.include_detailed_attributes,
                tree_output_format,
            )
            .await
            {
                Ok(((result, element), selector, diff)) => {
                    if diff.is_some() {
                        span.set_attribute("ui_diff.captured", "true".to_string());
                    }
                    Ok(((result, element), selector, diff))
                }
                Err(e) => {
                    // Restore windows before returning error
                    self.restore_window_management(should_restore).await;
                    Err(build_element_not_found_error(
                        &args.selector.build_full_selector(),
                        args.selector.build_alternative_selectors().as_deref(),
                        args.selector.build_fallback_selectors().as_deref(),
                        e,
                    ))
                }
            }?;

        let element_info = build_element_info(&element);

        let mut result_json = json!({
            "action": "select_option",
            "status": "success",
            "action_result": {
                "action": result.action,
                "details": result.details,
                "data": result.data,
            },
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector.build_full_selector(), args.selector.build_alternative_selectors().as_deref(), args.selector.build_fallback_selectors().as_deref()),
            "option_selected": args.option_name,
        });

        // Attach UI diff if captured
        if let Some(diff_result) = ui_diff {
            tracing::debug!(
                "[select_option] Attaching UI diff to result (has_changes: {})",
                diff_result.has_changes
            );
            span.set_attribute("ui_diff.has_changes", diff_result.has_changes.to_string());

            result_json["ui_diff"] = json!(diff_result.diff);
            result_json["has_ui_changes"] = json!(diff_result.has_changes);
            if args.tree.ui_diff_include_full_trees_in_response.unwrap_or(false) {
                result_json["tree_before"] = json!(diff_result.tree_before);
                result_json["tree_after"] = json!(diff_result.tree_after);
            }
        }

        self.restore_window_management(should_restore).await;

        span.set_status(true, None);
        span.end();

        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(result_json)?],
                None,
            )
            .await,
        ))
    }

    #[tool(
        description = "Lists all available option strings from a dropdown, list box, or similar control. This is a read-only operation."
    )]
    async fn list_options(
        &self,
        Parameters(args): Parameters<LocatorArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("list_options", None);

        // Add comprehensive telemetry attributes
        span.set_attribute("selector", args.selector.selector.clone());
        if let Some(retries) = args.action.retries {
            span.set_attribute("retry.max_attempts", retries.to_string());
        }

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            let flag_value = *in_sequence;
            let should_restore_value = !flag_value;
            tracing::info!(
                "[list_options] Flag check: in_sequence={}, should_restore={}",
                flag_value,
                should_restore_value
            );
            should_restore_value
        };

        if should_restore {
            tracing::info!(
                "[list_options] Direct MCP call detected - performing window management"
            );
            let _ = self
                .prepare_window_management(
                    &args.selector.process,
                    None,
                    None,
                    None,
                    &args.window_mgmt,
                )
                .await;
        } else {
            tracing::debug!("[list_options] In sequence - skipping window management (dispatch_tool handles it)");
        }

        let ((options, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector.build_full_selector(),
                args.selector.build_alternative_selectors().as_deref(),
                args.selector.build_fallback_selectors().as_deref(),
                args.action.timeout_ms,
                args.action.retries,
                |element| async move { element.list_options() },
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => {
                    // Restore windows before returning error
                    self.restore_window_management(should_restore).await;
                    Err(build_element_not_found_error(
                        &args.selector.build_full_selector(),
                        args.selector.build_alternative_selectors().as_deref(),
                        args.selector.build_fallback_selectors().as_deref(),
                        e,
                    ))
                }
            }?;

        let element_info = build_element_info(&element);

        let mut result_json = json!({
            "action": "list_options",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector.build_full_selector(), args.selector.build_alternative_selectors().as_deref(), args.selector.build_fallback_selectors().as_deref()),
            "options": options,
            "count": options.len(),
        });
        maybe_attach_tree(
            &self.desktop,
            args.tree.include_tree_after_action,
            args.tree.tree_max_depth,
            args.tree.tree_from_selector.as_deref(),
            args.tree.include_detailed_attributes,
            None,
            element.process_id().ok(),
            &mut result_json,
            Some(&element),
            false,
        )
        .await;

        // Restore windows after listing options
        self.restore_window_management(should_restore).await;

        span.set_status(true, None);
        span.end();

        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(result_json)?],
                None,
            )
            .await,
        ))
    }

    #[tool(
        description = "Sets the state of a toggleable control (e.g., checkbox, switch). This action requires the application to be focused and may change the UI."
    )]
    async fn set_toggled(
        &self,
        Parameters(args): Parameters<SetToggledArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("set_toggled", None);

        // Add comprehensive telemetry attributes
        span.set_attribute("selector", args.selector.selector.clone());
        span.set_attribute("state", args.state.to_string());
        if let Some(retries) = args.action.retries {
            span.set_attribute("retry.max_attempts", retries.to_string());
        }

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            !*in_sequence
        };

        if should_restore {
            tracing::info!("[set_toggled] Direct MCP call detected - performing window management");
            let _ = self
                .prepare_window_management(
                    &args.selector.process,
                    None,
                    None,
                    None,
                    &args.window_mgmt,
                )
                .await;
        } else {
            tracing::debug!(
                "[set_toggled] In sequence - skipping window management (dispatch_tool handles it)"
            );
        }

        let state = args.state;
        let action = move |element: UIElement| async move {
            // Ensure element is visible before interaction
            if let Err(e) = Self::ensure_element_in_view(&element) {
                tracing::warn!("Failed to ensure element is in view for set_toggled: {e}");
            }
            element.set_toggled_with_state(state)
        };

        // Store tree config to avoid move issues
        let tree_output_format = args
            .tree
            .tree_output_format
            .unwrap_or(crate::mcp_types::TreeOutputFormat::CompactYaml);

        let ((result, element), successful_selector, ui_diff) =
            match crate::helpers::find_and_execute_with_ui_diff(
                &self.desktop,
                &args.selector.build_full_selector(),
                None, // SetToggled doesn't have alternative selectors
                args.selector.build_fallback_selectors().as_deref(),
                args.action.timeout_ms,
                args.action.retries,
                action,
                args.tree.ui_diff_before_after,
                args.tree.tree_max_depth,
                args.tree.include_detailed_attributes,
                tree_output_format,
            )
            .await
            {
                Ok(((result, element), selector, diff)) => {
                    if diff.is_some() {
                        span.set_attribute("ui_diff.captured", "true".to_string());
                    }
                    Ok(((result, element), selector, diff))
                }
                Err(e) => Err(build_element_not_found_error(
                    &args.selector.build_full_selector(),
                    None,
                    args.selector.build_fallback_selectors().as_deref(),
                    e,
                )),
            }?;

        let element_info = build_element_info(&element);

        let mut result_json = json!({
            "action": "set_toggled",
            "status": "success",
            "action_result": {
                "action": result.action,
                "details": result.details,
                "data": result.data,
            },
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector.build_full_selector(), None, args.selector.build_fallback_selectors().as_deref()),
            "state_set_to": args.state,
        });

        // POST-ACTION VERIFICATION: Magic auto-verification or explicit verification
        let should_auto_verify = args.action.verify_element_exists.is_empty()
            && args.action.verify_element_not_exists.is_empty();

        if should_auto_verify {
            // MAGIC AUTO-VERIFICATION: Verify toggle state was actually set
            // For toggle state, we do a direct property check (can't use selector)
            tracing::debug!(
                "[set_toggled] Auto-verification: checking is_toggled = {}",
                args.state
            );
            span.set_attribute("verification.auto_inferred", "true".to_string());

            // Try direct read first (fast path)
            let actual_state = element.is_toggled().unwrap_or(!args.state); // Default to opposite if can't read

            if actual_state != args.state {
                // State mismatch - verification failed
                tracing::error!(
                    "[set_toggled] Auto-verification failed: expected {}, got {}",
                    args.state,
                    actual_state
                );
                span.set_attribute("verification.passed", "false".to_string());
                span.set_status(false, Some("Toggle state verification failed"));
                span.end();
                return Err(McpError::internal_error(
                    format!(
                        "Toggle state verification failed: expected {}, got {}",
                        args.state, actual_state
                    ),
                    Some(json!({
                        "expected_state": args.state,
                        "actual_state": actual_state,
                        "selector_used": successful_selector,
                    })),
                ));
            }

            tracing::info!(
                "[set_toggled] Auto-verification passed: is_toggled = {}",
                actual_state
            );
            span.set_attribute("verification.passed", "true".to_string());
            span.set_attribute("verification.method", "direct_property_read".to_string());

            if let Some(obj) = result_json.as_object_mut() {
                obj.insert(
                    "verification".to_string(),
                    json!({
                        "passed": true,
                        "method": "direct_property_read",
                        "expected_state": args.state,
                        "actual_state": actual_state,
                    }),
                );
            }
        } else if !args.action.verify_element_exists.is_empty()
            || !args.action.verify_element_not_exists.is_empty()
        {
            // Explicit verification using selectors
            let verify_timeout_ms = args.action.verify_timeout_ms.unwrap_or(2000);

            let verify_exists_opt = if args.action.verify_element_exists.is_empty() {
                None
            } else {
                Some(args.action.verify_element_exists.as_str())
            };
            let verify_not_exists_opt = if args.action.verify_element_not_exists.is_empty() {
                None
            } else {
                Some(args.action.verify_element_not_exists.as_str())
            };

            match crate::helpers::verify_post_action(
                &self.desktop,
                &element,
                verify_exists_opt,
                verify_not_exists_opt,
                verify_timeout_ms,
                &successful_selector,
            )
            .await
            {
                Ok(verification_result) => {
                    span.set_attribute("verification.passed", "true".to_string());
                    span.set_attribute("verification.method", verification_result.method.clone());

                    if let Some(obj) = result_json.as_object_mut() {
                        obj.insert(
                            "verification".to_string(),
                            json!({
                                "passed": verification_result.passed,
                                "method": verification_result.method,
                                "details": verification_result.details,
                                "elapsed_ms": verification_result.elapsed_ms,
                            }),
                        );
                    }
                }
                Err(e) => {
                    span.set_status(false, Some("Verification failed"));
                    span.end();
                    return Err(McpError::internal_error(
                        format!("Post-action verification failed: {e}"),
                        Some(json!({
                            "selector_used": successful_selector,
                            "timeout_ms": verify_timeout_ms,
                        })),
                    ));
                }
            }
        }

        // Attach UI diff if captured
        if let Some(diff_result) = ui_diff {
            tracing::debug!(
                "[set_toggled] Attaching UI diff to result (has_changes: {})",
                diff_result.has_changes
            );
            span.set_attribute("ui_diff.has_changes", diff_result.has_changes.to_string());

            result_json["ui_diff"] = json!(diff_result.diff);
            result_json["has_ui_changes"] = json!(diff_result.has_changes);
            if args.tree.ui_diff_include_full_trees_in_response.unwrap_or(false) {
                result_json["tree_before"] = json!(diff_result.tree_before);
                result_json["tree_after"] = json!(diff_result.tree_after);
            }
        }

        self.restore_window_management(should_restore).await;

        span.set_status(true, None);
        span.end();

        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(result_json)?],
                None,
            )
            .await,
        ))
    }

    #[tool(
        description = "Sets the value of a range-based control like a slider. This action requires the application to be focused and may change the UI."
    )]
    async fn set_range_value(
        &self,
        Parameters(args): Parameters<SetRangeValueArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("set_range_value", None);

        // Add comprehensive telemetry attributes
        span.set_attribute("selector", args.selector.selector.clone());
        span.set_attribute("value", args.value.to_string());
        if let Some(retries) = args.action.retries {
            span.set_attribute("retry.max_attempts", retries.to_string());
        }

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            !*in_sequence
        };

        if should_restore {
            tracing::info!(
                "[set_range_value] Direct MCP call detected - performing window management"
            );
            let _ = self
                .prepare_window_management(
                    &args.selector.process,
                    None,
                    None,
                    None,
                    &args.window_mgmt,
                )
                .await;
        } else {
            tracing::debug!("[set_range_value] In sequence - skipping window management (dispatch_tool handles it)");
        }

        let value = args.value;
        let action = move |element: UIElement| async move {
            // Ensure element is visible before interaction
            if let Err(e) = Self::ensure_element_in_view(&element) {
                tracing::warn!("Failed to ensure element is in view for set_range_value: {e}");
            }
            element.set_range_value(value)
        };

        // Store tree config to avoid move issues
        let tree_output_format = args
            .tree
            .tree_output_format
            .unwrap_or(crate::mcp_types::TreeOutputFormat::CompactYaml);

        let ((_result, element), successful_selector, ui_diff) =
            match crate::helpers::find_and_execute_with_ui_diff(
                &self.desktop,
                &args.selector.build_full_selector(),
                None, // SetRangeValue doesn't have alternative selectors
                args.selector.build_fallback_selectors().as_deref(),
                args.action.timeout_ms,
                args.action.retries,
                action,
                args.tree.ui_diff_before_after,
                args.tree.tree_max_depth,
                args.tree.include_detailed_attributes,
                tree_output_format,
            )
            .await
            {
                Ok(((result, element), selector, diff)) => {
                    if diff.is_some() {
                        span.set_attribute("ui_diff.captured", "true".to_string());
                    }
                    Ok(((result, element), selector, diff))
                }
                Err(e) => Err(build_element_not_found_error(
                    &args.selector.build_full_selector(),
                    None,
                    args.selector.build_fallback_selectors().as_deref(),
                    e,
                )),
            }?;

        let element_info = build_element_info(&element);

        let mut result_json = json!({
            "action": "set_range_value",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector.build_full_selector(), None, args.selector.build_fallback_selectors().as_deref()),
            "value_set_to": args.value,
        });

        // POST-ACTION VERIFICATION: Magic auto-verification or explicit verification
        let should_auto_verify = args.action.verify_element_exists.is_empty()
            && args.action.verify_element_not_exists.is_empty();

        if should_auto_verify {
            // MAGIC AUTO-VERIFICATION: Verify range value was actually set
            tracing::debug!(
                "[set_range_value] Auto-verification: checking range_value = {}",
                args.value
            );
            span.set_attribute("verification.auto_inferred", "true".to_string());

            let actual_value = element.get_range_value().unwrap_or(f64::NAN);
            let tolerance = 0.01; // Allow small floating point differences

            if (actual_value - args.value).abs() > tolerance {
                tracing::error!(
                    "[set_range_value] Auto-verification failed: expected {}, got {}",
                    args.value,
                    actual_value
                );
                span.set_attribute("verification.passed", "false".to_string());
                span.set_status(false, Some("Range value verification failed"));
                span.end();
                return Err(McpError::internal_error(
                    format!(
                        "Range value verification failed: expected {}, got {}",
                        args.value, actual_value
                    ),
                    Some(json!({
                        "expected_value": args.value,
                        "actual_value": actual_value,
                        "selector_used": successful_selector,
                    })),
                ));
            }

            tracing::info!(
                "[set_range_value] Auto-verification passed: range_value = {}",
                actual_value
            );
            span.set_attribute("verification.passed", "true".to_string());
            span.set_attribute("verification.method", "direct_property_read".to_string());

            if let Some(obj) = result_json.as_object_mut() {
                obj.insert(
                    "verification".to_string(),
                    json!({
                        "passed": true,
                        "method": "direct_property_read",
                        "expected_value": args.value,
                        "actual_value": actual_value,
                    }),
                );
            }
        } else if !args.action.verify_element_exists.is_empty()
            || !args.action.verify_element_not_exists.is_empty()
        {
            // Explicit verification using selectors
            let verify_timeout_ms = args.action.verify_timeout_ms.unwrap_or(2000);

            let verify_exists_opt = if args.action.verify_element_exists.is_empty() {
                None
            } else {
                Some(args.action.verify_element_exists.as_str())
            };
            let verify_not_exists_opt = if args.action.verify_element_not_exists.is_empty() {
                None
            } else {
                Some(args.action.verify_element_not_exists.as_str())
            };

            match crate::helpers::verify_post_action(
                &self.desktop,
                &element,
                verify_exists_opt,
                verify_not_exists_opt,
                verify_timeout_ms,
                &successful_selector,
            )
            .await
            {
                Ok(verification_result) => {
                    span.set_attribute("verification.passed", "true".to_string());
                    span.set_attribute("verification.method", verification_result.method.clone());

                    if let Some(obj) = result_json.as_object_mut() {
                        obj.insert(
                            "verification".to_string(),
                            json!({
                                "passed": verification_result.passed,
                                "method": verification_result.method,
                                "details": verification_result.details,
                                "elapsed_ms": verification_result.elapsed_ms,
                            }),
                        );
                    }
                }
                Err(e) => {
                    span.set_status(false, Some("Verification failed"));
                    span.end();
                    return Err(McpError::internal_error(
                        format!("Post-action verification failed: {e}"),
                        Some(json!({
                            "selector_used": successful_selector,
                            "timeout_ms": verify_timeout_ms,
                        })),
                    ));
                }
            }
        }

        // Attach UI diff if captured
        if let Some(diff_result) = ui_diff {
            tracing::debug!(
                "[set_range_value] Attaching UI diff to result (has_changes: {})",
                diff_result.has_changes
            );
            span.set_attribute("ui_diff.has_changes", diff_result.has_changes.to_string());

            result_json["ui_diff"] = json!(diff_result.diff);
            result_json["has_ui_changes"] = json!(diff_result.has_changes);
            if args.tree.ui_diff_include_full_trees_in_response.unwrap_or(false) {
                result_json["tree_before"] = json!(diff_result.tree_before);
                result_json["tree_after"] = json!(diff_result.tree_after);
            }
        }

        self.restore_window_management(should_restore).await;

        span.set_status(true, None);
        span.end();

        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(result_json)?],
                None,
            )
            .await,
        ))
    }

    #[tool(
        description = "Sets the selection state of a selectable item (e.g., in a list or calendar). This action requires the application to be focused and may change the UI."
    )]
    async fn set_selected(
        &self,
        Parameters(args): Parameters<SetSelectedArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("set_selected", None);

        // Add comprehensive telemetry attributes
        span.set_attribute("selector", args.selector.selector.clone());
        span.set_attribute("state", args.state.to_string());
        if let Some(retries) = args.action.retries {
            span.set_attribute("retry.max_attempts", retries.to_string());
        }

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            !*in_sequence
        };

        if should_restore {
            tracing::info!(
                "[set_selected] Direct MCP call detected - performing window management"
            );
            let _ = self
                .prepare_window_management(
                    &args.selector.process,
                    None,
                    None,
                    None,
                    &args.window_mgmt,
                )
                .await;
        } else {
            tracing::debug!("[set_selected] In sequence - skipping window management (dispatch_tool handles it)");
        }

        let state = args.state;
        let action =
            move |element: UIElement| async move { element.set_selected_with_state(state) };

        // Store tree config to avoid move issues
        let tree_output_format = args
            .tree
            .tree_output_format
            .unwrap_or(crate::mcp_types::TreeOutputFormat::CompactYaml);

        let ((result, element), successful_selector, ui_diff) =
            match crate::helpers::find_and_execute_with_ui_diff(
                &self.desktop,
                &args.selector.build_full_selector(),
                None, // SetSelected doesn't have alternative selectors
                args.selector.build_fallback_selectors().as_deref(),
                args.action.timeout_ms,
                args.action.retries,
                action,
                args.tree.ui_diff_before_after,
                args.tree.tree_max_depth,
                args.tree.include_detailed_attributes,
                tree_output_format,
            )
            .await
            {
                Ok(((result, element), selector, diff)) => {
                    if diff.is_some() {
                        span.set_attribute("ui_diff.captured", "true".to_string());
                    }
                    Ok(((result, element), selector, diff))
                }
                Err(e) => Err(build_element_not_found_error(
                    &args.selector.build_full_selector(),
                    None,
                    args.selector.build_fallback_selectors().as_deref(),
                    e,
                )),
            }?;

        let element_info = build_element_info(&element);

        let mut result_json = json!({
            "action": "set_selected",
            "status": "success",
            "action_result": {
                "action": result.action,
                "details": result.details,
                "data": result.data,
            },
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector.build_full_selector(), None, args.selector.build_fallback_selectors().as_deref()),
            "state_set_to": args.state,
        });

        // POST-ACTION VERIFICATION: Magic auto-verification or explicit verification
        let should_auto_verify = args.action.verify_element_exists.is_empty()
            && args.action.verify_element_not_exists.is_empty();

        if should_auto_verify {
            // MAGIC AUTO-VERIFICATION: Verify selected state was actually set
            tracing::debug!(
                "[set_selected] Auto-verification: checking is_selected = {}",
                args.state
            );
            span.set_attribute("verification.auto_inferred", "true".to_string());

            let actual_state = element.is_selected().unwrap_or(!args.state);

            if actual_state != args.state {
                tracing::error!(
                    "[set_selected] Auto-verification failed: expected {}, got {}",
                    args.state,
                    actual_state
                );
                span.set_attribute("verification.passed", "false".to_string());
                span.set_status(false, Some("Selected state verification failed"));
                span.end();
                return Err(McpError::internal_error(
                    format!(
                        "Selected state verification failed: expected {}, got {}",
                        args.state, actual_state
                    ),
                    Some(json!({
                        "expected_state": args.state,
                        "actual_state": actual_state,
                        "selector_used": successful_selector,
                    })),
                ));
            }

            tracing::info!(
                "[set_selected] Auto-verification passed: is_selected = {}",
                actual_state
            );
            span.set_attribute("verification.passed", "true".to_string());
            span.set_attribute("verification.method", "direct_property_read".to_string());

            if let Some(obj) = result_json.as_object_mut() {
                obj.insert(
                    "verification".to_string(),
                    json!({
                        "passed": true,
                        "method": "direct_property_read",
                        "expected_state": args.state,
                        "actual_state": actual_state,
                    }),
                );
            }
        } else if !args.action.verify_element_exists.is_empty()
            || !args.action.verify_element_not_exists.is_empty()
        {
            // Explicit verification using selectors
            let verify_timeout_ms = args.action.verify_timeout_ms.unwrap_or(2000);

            let verify_exists_opt = if args.action.verify_element_exists.is_empty() {
                None
            } else {
                Some(args.action.verify_element_exists.as_str())
            };
            let verify_not_exists_opt = if args.action.verify_element_not_exists.is_empty() {
                None
            } else {
                Some(args.action.verify_element_not_exists.as_str())
            };

            match crate::helpers::verify_post_action(
                &self.desktop,
                &element,
                verify_exists_opt,
                verify_not_exists_opt,
                verify_timeout_ms,
                &successful_selector,
            )
            .await
            {
                Ok(verification_result) => {
                    span.set_attribute("verification.passed", "true".to_string());
                    span.set_attribute("verification.method", verification_result.method.clone());

                    if let Some(obj) = result_json.as_object_mut() {
                        obj.insert(
                            "verification".to_string(),
                            json!({
                                "passed": verification_result.passed,
                                "method": verification_result.method,
                                "details": verification_result.details,
                                "elapsed_ms": verification_result.elapsed_ms,
                            }),
                        );
                    }
                }
                Err(e) => {
                    span.set_status(false, Some("Verification failed"));
                    span.end();
                    return Err(McpError::internal_error(
                        format!("Post-action verification failed: {e}"),
                        Some(json!({
                            "selector_used": successful_selector,
                            "timeout_ms": verify_timeout_ms,
                        })),
                    ));
                }
            }
        }

        // Attach UI diff if captured
        if let Some(diff_result) = ui_diff {
            tracing::debug!(
                "[set_selected] Attaching UI diff to result (has_changes: {})",
                diff_result.has_changes
            );
            span.set_attribute("ui_diff.has_changes", diff_result.has_changes.to_string());

            result_json["ui_diff"] = json!(diff_result.diff);
            result_json["has_ui_changes"] = json!(diff_result.has_changes);
            if args.tree.ui_diff_include_full_trees_in_response.unwrap_or(false) {
                result_json["tree_before"] = json!(diff_result.tree_before);
                result_json["tree_after"] = json!(diff_result.tree_after);
            }
        }

        self.restore_window_management(should_restore).await;

        span.set_status(true, None);
        span.end();

        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(result_json)?],
                None,
            )
            .await,
        ))
    }

    #[tool(
        description = "Checks if a control (like a checkbox or toggle switch) is currently toggled on. This is a read-only operation."
    )]
    async fn is_toggled(
        &self,
        Parameters(args): Parameters<LocatorArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("is_toggled", None);

        // Add comprehensive telemetry attributes
        span.set_attribute("selector", args.selector.selector.clone());
        if let Some(retries) = args.action.retries {
            span.set_attribute("retry.max_attempts", retries.to_string());
        }

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            let flag_value = *in_sequence;
            let should_restore_value = !flag_value;
            tracing::info!(
                "[is_toggled] Flag check: in_sequence={}, should_restore={}",
                flag_value,
                should_restore_value
            );
            should_restore_value
        };

        if should_restore {
            tracing::info!("[is_toggled] Direct MCP call detected - performing window management");
            let _ = self
                .prepare_window_management(
                    &args.selector.process,
                    None,
                    None,
                    None,
                    &args.window_mgmt,
                )
                .await;
        } else {
            tracing::debug!(
                "[is_toggled] In sequence - skipping window management (dispatch_tool handles it)"
            );
        }

        let ((is_toggled, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector.build_full_selector(),
                args.selector.build_alternative_selectors().as_deref(),
                args.selector.build_fallback_selectors().as_deref(),
                args.action.timeout_ms,
                args.action.retries,
                |element| async move { element.is_toggled() },
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector.build_full_selector(),
                    args.selector.build_alternative_selectors().as_deref(),
                    args.selector.build_fallback_selectors().as_deref(),
                    e,
                )),
            }?;

        let element_info = build_element_info(&element);

        let mut result_json = json!({
            "action": "is_toggled",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector.build_full_selector(), args.selector.build_alternative_selectors().as_deref(), args.selector.build_fallback_selectors().as_deref()),
            "is_toggled": is_toggled,
        });
        maybe_attach_tree(
            &self.desktop,
            args.tree.include_tree_after_action,
            args.tree.tree_max_depth,
            args.tree.tree_from_selector.as_deref(),
            args.tree.include_detailed_attributes,
            None,
            element.process_id().ok(),
            &mut result_json,
            Some(&element),
            false,
        )
        .await;

        // Restore windows after checking if element is toggled
        self.restore_window_management(should_restore).await;

        span.set_status(true, None);
        span.end();

        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(result_json)?],
                None,
            )
            .await,
        ))
    }

    #[tool(
        description = "Gets the current value from a range-based control like a slider or progress bar. This is a read-only operation."
    )]
    async fn get_range_value(
        &self,
        Parameters(args): Parameters<LocatorArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("get_range_value", None);

        // Add comprehensive telemetry attributes
        span.set_attribute("selector", args.selector.selector.clone());
        if let Some(retries) = args.action.retries {
            span.set_attribute("retry.max_attempts", retries.to_string());
        }

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            let flag_value = *in_sequence;
            let should_restore_value = !flag_value;
            tracing::info!(
                "[get_range_value] Flag check: in_sequence={}, should_restore={}",
                flag_value,
                should_restore_value
            );
            should_restore_value
        };

        if should_restore {
            tracing::info!(
                "[get_range_value] Direct MCP call detected - performing window management"
            );
            let _ = self
                .prepare_window_management(
                    &args.selector.process,
                    None,
                    None,
                    None,
                    &args.window_mgmt,
                )
                .await;
        } else {
            tracing::debug!("[get_range_value] In sequence - skipping window management (dispatch_tool handles it)");
        }

        let ((value, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector.build_full_selector(),
                args.selector.build_alternative_selectors().as_deref(),
                args.selector.build_fallback_selectors().as_deref(),
                args.action.timeout_ms,
                args.action.retries,
                |element| async move { element.get_range_value() },
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector.build_full_selector(),
                    args.selector.build_alternative_selectors().as_deref(),
                    args.selector.build_fallback_selectors().as_deref(),
                    e,
                )),
            }?;

        let element_info = build_element_info(&element);

        let mut result_json = json!({
            "action": "get_range_value",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector.build_full_selector(), args.selector.build_alternative_selectors().as_deref(), args.selector.build_fallback_selectors().as_deref()),
            "value": value,
        });
        maybe_attach_tree(
            &self.desktop,
            args.tree.include_tree_after_action,
            args.tree.tree_max_depth,
            args.tree.tree_from_selector.as_deref(),
            args.tree.include_detailed_attributes,
            None,
            element.process_id().ok(),
            &mut result_json,
            Some(&element),
            false,
        )
        .await;

        // Restore windows after getting range value
        self.restore_window_management(should_restore).await;

        span.set_status(true, None);
        span.end();

        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(result_json)?],
                None,
            )
            .await,
        ))
    }

    #[tool(
        description = "Checks if a selectable item (e.g., in a calendar, list, or tab) is currently selected. This is a read-only operation."
    )]
    async fn is_selected(
        &self,
        Parameters(args): Parameters<LocatorArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("is_selected", None);

        // Add comprehensive telemetry attributes
        span.set_attribute("selector", args.selector.selector.clone());
        if let Some(retries) = args.action.retries {
            span.set_attribute("retry.max_attempts", retries.to_string());
        }

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            let flag_value = *in_sequence;
            let should_restore_value = !flag_value;
            tracing::info!(
                "[is_selected] Flag check: in_sequence={}, should_restore={}",
                flag_value,
                should_restore_value
            );
            should_restore_value
        };

        if should_restore {
            tracing::info!("[is_selected] Direct MCP call detected - performing window management");
            let _ = self
                .prepare_window_management(
                    &args.selector.process,
                    None,
                    None,
                    None,
                    &args.window_mgmt,
                )
                .await;
        } else {
            tracing::debug!(
                "[is_selected] In sequence - skipping window management (dispatch_tool handles it)"
            );
        }

        let ((is_selected, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector.build_full_selector(),
                args.selector.build_alternative_selectors().as_deref(),
                args.selector.build_fallback_selectors().as_deref(),
                args.action.timeout_ms,
                args.action.retries,
                |element| async move { element.is_selected() },
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector.build_full_selector(),
                    args.selector.build_alternative_selectors().as_deref(),
                    args.selector.build_fallback_selectors().as_deref(),
                    e,
                )),
            }?;

        let element_info = build_element_info(&element);

        let mut result_json = json!({
            "action": "is_selected",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector.build_full_selector(), args.selector.build_alternative_selectors().as_deref(), args.selector.build_fallback_selectors().as_deref()),
            "is_selected": is_selected,
        });
        maybe_attach_tree(
            &self.desktop,
            args.tree.include_tree_after_action,
            args.tree.tree_max_depth,
            args.tree.tree_from_selector.as_deref(),
            args.tree.include_detailed_attributes,
            None,
            element.process_id().ok(),
            &mut result_json,
            Some(&element),
            false,
        )
        .await;

        // Restore windows after checking if element is selected
        self.restore_window_management(should_restore).await;

        span.set_status(true, None);
        span.end();

        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(result_json)?],
                None,
            )
            .await,
        ))
    }

    #[tool(
        description = "Captures a screenshot of a specific UI element. Automatically resizes to max 1920px (customizable via max_dimension parameter) while maintaining aspect ratio. Uses process-scoped selector for reliable element capture."
    )]
    async fn capture_element_screenshot(
        &self,
        Parameters(args): Parameters<CaptureElementScreenshotArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("capture_element_screenshot", None);

        // Add telemetry attributes
        span.set_attribute("selector", args.selector.selector.clone());
        if let Some(retries) = args.action.retries {
            span.set_attribute("retry.max_attempts", retries.to_string());
        }

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            !*in_sequence
        };

        if should_restore {
            tracing::info!("[capture_element_screenshot] Direct MCP call detected - performing window management");
            let _ = self
                .prepare_window_management(
                    &args.selector.process,
                    None,
                    None,
                    None,
                    &args.window_mgmt,
                )
                .await;
        } else {
            tracing::debug!("[capture_element_screenshot] In sequence - skipping window management (dispatch_tool handles it)");
        }

        // Capture screenshot using process-scoped selector
        let ((screenshot_result, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector.build_full_selector(),
                args.selector.build_alternative_selectors().as_deref(),
                args.selector.build_fallback_selectors().as_deref(),
                args.action.timeout_ms,
                args.action.retries,
                |element| async move { element.capture() },
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector.build_full_selector(),
                    args.selector.build_alternative_selectors().as_deref(),
                    args.selector.build_fallback_selectors().as_deref(),
                    e,
                )),
            }?;

        // Store original dimensions for metadata
        let original_width = screenshot_result.width;
        let original_height = screenshot_result.height;
        let original_size_bytes = screenshot_result.image_data.len();

        // Convert BGRA to RGBA (xcap returns BGRA format, we need RGBA)
        // Swap red and blue channels: BGRA -> RGBA
        let rgba_data: Vec<u8> = screenshot_result
            .image_data
            .chunks_exact(4)
            .flat_map(|bgra| [bgra[2], bgra[1], bgra[0], bgra[3]]) // B,G,R,A -> R,G,B,A
            .collect();

        // Apply resize if needed (default max dimension is 1920px)
        let max_dim = args.max_dimension.unwrap_or(1920);
        let (final_width, final_height, final_rgba_data, was_resized) = if original_width > max_dim
            || original_height > max_dim
        {
            // Calculate new dimensions maintaining aspect ratio
            let scale = (max_dim as f32 / original_width.max(original_height) as f32).min(1.0);
            let new_width = (original_width as f32 * scale).round() as u32;
            let new_height = (original_height as f32 * scale).round() as u32;

            // Create ImageBuffer from RGBA data
            let img =
                ImageBuffer::<Rgba<u8>, _>::from_raw(original_width, original_height, rgba_data)
                    .ok_or_else(|| {
                        McpError::internal_error(
                            "Failed to create image buffer from screenshot data",
                            None,
                        )
                    })?;

            // Resize using Lanczos3 filter for high quality
            let resized =
                image::imageops::resize(&img, new_width, new_height, FilterType::Lanczos3);

            (new_width, new_height, resized.into_raw(), true)
        } else {
            (original_width, original_height, rgba_data, false)
        };

        // Encode to PNG with maximum compression
        let mut png_data = Vec::new();
        let encoder = PngEncoder::new(Cursor::new(&mut png_data));
        encoder
            .write_image(
                &final_rgba_data,
                final_width,
                final_height,
                ExtendedColorType::Rgba8,
            )
            .map_err(|e| {
                McpError::internal_error(
                    "Failed to encode screenshot to PNG",
                    Some(json!({ "reason": e.to_string() })),
                )
            })?;

        let base64_image = general_purpose::STANDARD.encode(&png_data);

        let element_info = build_element_info(&element);

        span.set_status(true, None);
        span.end();

        // Build metadata with resize information
        let metadata = json!({
            "action": "capture_element_screenshot",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector.build_full_selector(), args.selector.build_alternative_selectors().as_deref(), args.selector.build_fallback_selectors().as_deref()),
            "image_format": "png",
            "original_size": {
                "width": original_width,
                "height": original_height,
                "bytes": original_size_bytes,
                "mb": (original_size_bytes as f64 / 1024.0 / 1024.0)
            },
            "final_size": {
                "width": final_width,
                "height": final_height,
                "bytes": png_data.len(),
                "mb": (png_data.len() as f64 / 1024.0 / 1024.0)
            },
            "resized": was_resized,
            "max_dimension_applied": max_dim,
        });

        self.restore_window_management(should_restore).await;

        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![
                    Content::json(metadata)?,
                    Content::image(base64_image, "image/png".to_string()),
                ],
                None,
            )
            .await,
        ))
    }

    #[tool(
        description = "Invokes a UI element. This is often more reliable than clicking for controls like radio buttons or menu items. This action requires the application to be focused and may change the UI."
    )]
    async fn invoke_element(
        &self,
        Parameters(args): Parameters<InvokeElementArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("invoke_element", None);

        // Add comprehensive telemetry attributes
        span.set_attribute("selector", args.selector.selector.clone());
        if let Some(retries) = args.action.retries {
            span.set_attribute("retry.max_attempts", retries.to_string());
        }

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            let flag_value = *in_sequence;
            let should_restore_value = !flag_value;
            tracing::info!(
                "[invoke_element] Flag check: in_sequence={}, should_restore={}",
                flag_value,
                should_restore_value
            );
            should_restore_value
        };

        if should_restore {
            tracing::info!(
                "[invoke_element] Direct MCP call detected - performing window management"
            );
            let _ = self
                .prepare_window_management(
                    &args.selector.process,
                    None,
                    None,
                    None,
                    &args.window_mgmt,
                )
                .await;
        } else {
            tracing::debug!("[invoke_element] In sequence - skipping window management (dispatch_tool handles it)");
        }

        // Store tree config to avoid move issues
        let tree_output_format = args
            .tree
            .tree_output_format
            .unwrap_or(crate::mcp_types::TreeOutputFormat::CompactYaml);

        let ((result, element), successful_selector, ui_diff) =
            match crate::helpers::find_and_execute_with_ui_diff(
                &self.desktop,
                &args.selector.build_full_selector(),
                args.selector.build_alternative_selectors().as_deref(),
                args.selector.build_fallback_selectors().as_deref(),
                args.action.timeout_ms,
                args.action.retries,
                |element| async move {
                    // Ensure element is visible before interaction
                    if let Err(e) = Self::ensure_element_in_view(&element) {
                        tracing::warn!("Failed to ensure element is in view for invoke: {e}");
                    }
                    element.invoke_with_state()
                },
                args.tree.ui_diff_before_after,
                args.tree.tree_max_depth,
                args.tree.include_detailed_attributes,
                tree_output_format,
            )
            .await
            {
                Ok(((result, element), selector, diff)) => {
                    if diff.is_some() {
                        span.set_attribute("ui_diff.captured", "true".to_string());
                    }
                    Ok(((result, element), selector, diff))
                }
                Err(e) => {
                    // Restore windows before returning error
                    self.restore_window_management(should_restore).await;
                    Err(build_element_not_found_error(
                        &args.selector.build_full_selector(),
                        args.selector.build_alternative_selectors().as_deref(),
                        args.selector.build_fallback_selectors().as_deref(),
                        e,
                    ))
                }
            }?;

        let element_info = build_element_info(&element);

        let mut result_json = json!({
            "action": "invoke",
            "status": "success",
            "action_result": {
                "action": result.action,
                "details": result.details,
                "data": result.data,
            },
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector.build_full_selector(), args.selector.build_alternative_selectors().as_deref(), args.selector.build_fallback_selectors().as_deref()),
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        // POST-ACTION VERIFICATION
        if !args.action.verify_element_exists.is_empty()
            || !args.action.verify_element_not_exists.is_empty()
        {
            let verify_timeout_ms = args.action.verify_timeout_ms.unwrap_or(2000);

            let verify_exists_opt = if args.action.verify_element_exists.is_empty() {
                None
            } else {
                Some(args.action.verify_element_exists.as_str())
            };
            let verify_not_exists_opt = if args.action.verify_element_not_exists.is_empty() {
                None
            } else {
                Some(args.action.verify_element_not_exists.as_str())
            };

            match crate::helpers::verify_post_action(
                &self.desktop,
                &element,
                verify_exists_opt,
                verify_not_exists_opt,
                verify_timeout_ms,
                &successful_selector,
            )
            .await
            {
                Ok(verification_result) => {
                    tracing::info!(
                        "[invoke_element] Verification passed: method={}, details={}",
                        verification_result.method,
                        verification_result.details
                    );
                    span.set_attribute("verification.passed", "true".to_string());
                    span.set_attribute("verification.method", verification_result.method.clone());
                    span.set_attribute(
                        "verification.elapsed_ms",
                        verification_result.elapsed_ms.to_string(),
                    );

                    let verification_json = json!({
                        "passed": verification_result.passed,
                        "method": verification_result.method,
                        "details": verification_result.details,
                        "elapsed_ms": verification_result.elapsed_ms,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    });

                    if let Some(obj) = result_json.as_object_mut() {
                        obj.insert("verification".to_string(), verification_json);
                    }
                }
                Err(e) => {
                    tracing::error!("[invoke_element] Verification failed: {}", e);
                    span.set_attribute("verification.passed", "false".to_string());
                    span.set_status(false, Some("Verification failed"));
                    span.end();
                    return Err(McpError::internal_error(
                        format!("Post-action verification failed: {e}"),
                        Some(json!({
                            "selector_used": successful_selector,
                            "verify_exists": args.action.verify_element_exists,
                            "verify_not_exists": args.action.verify_element_not_exists,
                            "timeout_ms": verify_timeout_ms,
                        })),
                    ));
                }
            }
        }

        // Attach UI diff if captured (action tools only support diff, not standalone tree)
        if let Some(diff_result) = ui_diff {
            tracing::debug!(
                "[invoke_element] Attaching UI diff to result (has_changes: {})",
                diff_result.has_changes
            );
            span.set_attribute("ui_diff.has_changes", diff_result.has_changes.to_string());

            result_json["ui_diff"] = json!(diff_result.diff);
            result_json["has_ui_changes"] = json!(diff_result.has_changes);
            if args.tree.ui_diff_include_full_trees_in_response.unwrap_or(false) {
                result_json["tree_before"] = json!(diff_result.tree_before);
                result_json["tree_after"] = json!(diff_result.tree_after);
            }
        }

        // Restore windows after invoking element
        self.restore_window_management(should_restore).await;

        span.set_status(true, None);
        span.end();

        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(result_json)?],
                None,
            )
            .await,
        ))
    }

    #[tool(
        description = "Stops active element highlights immediately. If an ID is provided, stops that specific highlight; otherwise stops all."
    )]
    async fn stop_highlighting(
        &self,
        Parameters(_args): Parameters<crate::utils::StopHighlightingArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("stop_highlighting", None);

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            !*in_sequence
        };

        // Note: stop_highlighting doesn't interact with specific windows, so no prepare needed

        // Current minimal implementation ignores highlight_id and stops all tracked highlights
        let mut list = self.active_highlights.lock().await;
        let mut stopped = 0usize;
        while let Some(handle) = list.pop() {
            handle.close();
            stopped += 1;
        }
        let response = json!({
            "action": "stop_highlighting",
            "status": "success",
            "highlights_stopped": stopped,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        self.restore_window_management(should_restore).await;

        span.set_status(true, None);
        span.end();

        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(response)?],
                None,
            )
            .await,
        ))
    }
    // Tool functions continue below - part of impl block with #[tool_router]
    #[tool(
        description = "Executes multiple tools in sequence. Useful for automating complex workflows that require multiple steps. Each tool in the sequence can have its own error handling and delay configuration. Tool names can be provided either in short form (e.g., 'click_element') or full form (e.g., 'mcp_terminator-mcp-agent_click_element'). When using run_command with engine mode, data can be passed between steps using set_env - return { set_env: { key: value } } from one step. Access variables using direct syntax (e.g., 'key == \"value\"' in conditions or {{key}} in substitutions). IMPORTANT: Locator methods (.first, .all) require mandatory timeout parameters in milliseconds - use .first(0) for immediate search (no polling/retry), .first(1000) to retry for 1 second, or .first(5000) for slow-loading UI. Default timeout changed from 30s to 0ms (no polling) for performance. Supports conditional jumps with 'jumps' array - each jump has 'if' (expression evaluated on success), 'to_id' (target step), and optional 'reason' (logged explanation). Multiple jump conditions are evaluated in order with first-match-wins. Step results are accessible as {step_id}_status and {step_id}_result in jump expressions. Expressions support equality (==, !=), numeric comparison (>, <, >=, <=), logical operators (&&, ||, !), and functions (contains, startsWith, endsWith, always). Undefined variables are handled gracefully (undefined != 'value' returns true). Type coercion automatically converts strings to numbers for numeric comparisons. Supports partial execution with 'start_from_step' and 'end_at_step' parameters to run specific step ranges. By default, jumps are skipped at the 'end_at_step' boundary for predictable execution; use 'execute_jumps_at_end: true' to allow jumps at the boundary (e.g., for loops). State is automatically persisted to .mediar/workflows/ folder in workflow's directory when using file:// URLs, allowing workflows to be resumed from any step."
    )]
    pub async fn execute_sequence(
        &self,
        peer: Peer<RoleServer>,
        request_context: RequestContext<RoleServer>,
        Parameters(args): Parameters<ExecuteSequenceArgs>,
    ) -> Result<CallToolResult, McpError> {
        return self
            .execute_sequence_impl(peer, request_context, args)
            .await;
    }

    #[tool(description = "Maximizes a window.")]
    async fn maximize_window(
        &self,
        Parameters(args): Parameters<MaximizeWindowArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("maximize_window", None);

        // Add comprehensive telemetry attributes
        span.set_attribute("selector", args.selector.selector.clone());
        if let Some(retries) = args.action.retries {
            span.set_attribute("retry.max_attempts", retries.to_string());
        }

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            !*in_sequence
        };

        if should_restore {
            tracing::info!(
                "[maximize_window] Direct MCP call detected - performing window management"
            );
            let _ = self
                .prepare_window_management(
                    &args.selector.process,
                    None,
                    None,
                    None,
                    &args.window_mgmt,
                )
                .await;
        } else {
            tracing::debug!("[maximize_window] In sequence - skipping window management (dispatch_tool handles it)");
        }

        let ((_result, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector.build_full_selector(),
                args.selector.build_alternative_selectors().as_deref(),
                args.selector.build_fallback_selectors().as_deref(),
                args.action.timeout_ms,
                args.action.retries,
                |element| async move { element.maximize_window() },
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector.build_full_selector(),
                    args.selector.build_alternative_selectors().as_deref(),
                    args.selector.build_fallback_selectors().as_deref(),
                    e,
                )),
            }?;

        let element_info = build_element_info(&element);

        let result_json = json!({
            "action": "maximize_window",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector.build_full_selector(), args.selector.build_alternative_selectors().as_deref(), args.selector.build_fallback_selectors().as_deref()),
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        // Action tools only support UI diff, not standalone tree attachment

        self.restore_window_management(should_restore).await;

        span.set_status(true, None);
        span.end();

        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(result_json)?],
                None,
            )
            .await,
        ))
    }

    #[tool(description = "Minimizes a window.")]
    async fn minimize_window(
        &self,
        Parameters(args): Parameters<MinimizeWindowArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("minimize_window", None);

        // Add comprehensive telemetry attributes
        span.set_attribute("selector", args.selector.selector.clone());
        if let Some(retries) = args.action.retries {
            span.set_attribute("retry.max_attempts", retries.to_string());
        }

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            !*in_sequence
        };

        if should_restore {
            tracing::info!(
                "[minimize_window] Direct MCP call detected - performing window management"
            );
            let _ = self
                .prepare_window_management(
                    &args.selector.process,
                    None,
                    None,
                    None,
                    &args.window_mgmt,
                )
                .await;
        } else {
            tracing::debug!("[minimize_window] In sequence - skipping window management (dispatch_tool handles it)");
        }

        let ((_result, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector.build_full_selector(),
                args.selector.build_alternative_selectors().as_deref(),
                args.selector.build_fallback_selectors().as_deref(),
                args.action.timeout_ms,
                args.action.retries,
                |element| async move { element.minimize_window() },
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector.build_full_selector(),
                    args.selector.build_alternative_selectors().as_deref(),
                    args.selector.build_fallback_selectors().as_deref(),
                    e,
                )),
            }?;

        let element_info = build_element_info(&element);

        let result_json = json!({
            "action": "minimize_window",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector.build_full_selector(), args.selector.build_alternative_selectors().as_deref(), args.selector.build_fallback_selectors().as_deref()),
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        // Action tools only support UI diff, not standalone tree attachment

        self.restore_window_management(should_restore).await;

        span.set_status(true, None);
        span.end();

        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(result_json)?],
                None,
            )
            .await,
        ))
    }

    #[tool(
        description = "Sets the zoom level to a specific percentage (e.g., 100 for 100%, 150 for 150%, 50 for 50%)."
    )]
    async fn set_zoom(
        &self,
        Parameters(args): Parameters<SetZoomArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("set_zoom", None);

        // Add comprehensive telemetry attributes
        span.set_attribute("percentage", args.percentage.to_string());

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            !*in_sequence
        };

        if should_restore {
            tracing::info!("[set_zoom] Direct MCP call detected - performing window management");
            // Note: set_zoom operates on browser context, using generic browser process
            let _ = self
                .prepare_window_management("chrome", None, None, None, &args.window_mgmt)
                .await;
        } else {
            tracing::debug!(
                "[set_zoom] In sequence - skipping window management (dispatch_tool handles it)"
            );
        }

        self.desktop.set_zoom(args.percentage).await.map_err(|e| {
            McpError::internal_error("Failed to set zoom", Some(json!({"reason": e.to_string()})))
        })?;
        let mut result_json = json!({
            "action": "set_zoom",
            "status": "success",
            "percentage": args.percentage,
            "note": "Zoom level set to the specified percentage"
        });
        maybe_attach_tree(
            &self.desktop,
            args.tree.include_tree_after_action,
            args.tree.tree_max_depth,
            args.tree.tree_from_selector.as_deref(),
            args.tree.include_detailed_attributes,
            None,
            None, // No specific element for zoom operation
            &mut result_json,
            None, // No element available for zoom
            false,
        )
        .await;

        self.restore_window_management(should_restore).await;

        span.set_status(true, None);
        span.end();

        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(result_json)?],
                None,
            )
            .await,
        ))
    }

    #[tool(
        description = "Sets the text value of an editable control (e.g., an input field) directly using the underlying accessibility API. This action requires the application to be focused and may change the UI."
    )]
    async fn set_value(
        &self,
        Parameters(args): Parameters<SetValueArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("set_value", None);

        // Add comprehensive telemetry attributes
        span.set_attribute("selector", args.selector.selector.clone());
        span.set_attribute("value", args.value.to_string());
        if let Some(retries) = args.action.retries {
            span.set_attribute("retry.max_attempts", retries.to_string());
        }

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            !*in_sequence
        };

        if should_restore {
            tracing::info!("[set_value] Direct MCP call detected - performing window management");
            let _ = self
                .prepare_window_management(
                    &args.selector.process,
                    None,
                    None,
                    None,
                    &args.window_mgmt,
                )
                .await;
        } else {
            tracing::debug!(
                "[set_value] In sequence - skipping window management (dispatch_tool handles it)"
            );
        }

        let value_to_set = args.value.clone();
        let action = move |element: UIElement| {
            let value_to_set = value_to_set.clone();
            async move {
                // Activate window to ensure it has keyboard focus before setting value
                if let Err(e) = element.activate_window() {
                    tracing::warn!("Failed to activate window before setting value: {}", e);
                }
                element.set_value(&value_to_set)
            }
        };

        // Store tree config to avoid move issues
        let tree_output_format = args
            .tree
            .tree_output_format
            .unwrap_or(crate::mcp_types::TreeOutputFormat::CompactYaml);

        let ((_result, element), successful_selector, ui_diff) =
            match crate::helpers::find_and_execute_with_ui_diff(
                &self.desktop,
                &args.selector.build_full_selector(),
                args.selector.build_alternative_selectors().as_deref(),
                args.selector.build_fallback_selectors().as_deref(),
                args.action.timeout_ms,
                args.action.retries,
                action,
                args.tree.ui_diff_before_after,
                args.tree.tree_max_depth,
                args.tree.include_detailed_attributes,
                tree_output_format,
            )
            .await
            {
                Ok(((result, element), selector, diff)) => {
                    if diff.is_some() {
                        span.set_attribute("ui_diff.captured", "true".to_string());
                    }
                    Ok(((result, element), selector, diff))
                }
                Err(e) => {
                    // Restore windows before returning error
                    self.restore_window_management(should_restore).await;
                    Err(build_element_not_found_error(
                        &args.selector.build_full_selector(),
                        args.selector.build_alternative_selectors().as_deref(),
                        args.selector.build_fallback_selectors().as_deref(),
                        e,
                    ))
                }
            }?;

        let element_info = build_element_info(&element);

        let mut result_json = json!({
            "action": "set_value",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector.build_full_selector(), args.selector.build_alternative_selectors().as_deref(), args.selector.build_fallback_selectors().as_deref()),
            "value_set_to": args.value,
        });

        // POST-ACTION VERIFICATION: Magic auto-verification or explicit verification
        let should_auto_verify = args.action.verify_element_exists.is_empty()
            && args.action.verify_element_not_exists.is_empty();

        let verify_exists = if should_auto_verify {
            // MAGIC AUTO-VERIFICATION: Verify the value was actually set
            tracing::debug!(
                "[set_value] Auto-verification enabled for value: {}",
                args.value
            );
            span.set_attribute("verification.auto_inferred", "true".to_string());
            format!("text:{}", args.value)
        } else {
            args.action.verify_element_exists.clone()
        };

        let verify_not_exists = args.action.verify_element_not_exists.clone();

        let skip_verification = verify_exists.is_empty() && verify_not_exists.is_empty();

        if !skip_verification {
            let verify_timeout_ms = args.action.verify_timeout_ms.unwrap_or(2000);

            let verify_exists_opt = if verify_exists.is_empty() {
                None
            } else {
                Some(verify_exists.as_str())
            };
            let verify_not_exists_opt = if verify_not_exists.is_empty() {
                None
            } else {
                Some(verify_not_exists.as_str())
            };

            match crate::helpers::verify_post_action(
                &self.desktop,
                &element,
                verify_exists_opt,
                verify_not_exists_opt,
                verify_timeout_ms,
                &successful_selector,
            )
            .await
            {
                Ok(verification_result) => {
                    tracing::info!(
                        "[set_value] Verification passed: {}",
                        verification_result.details
                    );
                    span.set_attribute("verification.passed", "true".to_string());
                    span.set_attribute("verification.method", verification_result.method.clone());

                    if let Some(obj) = result_json.as_object_mut() {
                        obj.insert(
                            "verification".to_string(),
                            json!({
                                "passed": verification_result.passed,
                                "method": verification_result.method,
                                "details": verification_result.details,
                                "elapsed_ms": verification_result.elapsed_ms,
                            }),
                        );
                    }
                }
                Err(e) => {
                    tracing::error!("[set_value] Verification failed: {}", e);
                    span.set_status(false, Some("Verification failed"));
                    span.end();
                    return Err(McpError::internal_error(
                        format!("Post-action verification failed: {e}"),
                        Some(json!({
                            "selector_used": successful_selector,
                            "verify_exists": verify_exists,
                            "timeout_ms": verify_timeout_ms,
                        })),
                    ));
                }
            }
        }

        // Attach UI diff if captured
        if let Some(diff_result) = ui_diff {
            tracing::debug!(
                "[set_value] Attaching UI diff to result (has_changes: {})",
                diff_result.has_changes
            );
            span.set_attribute("ui_diff.has_changes", diff_result.has_changes.to_string());

            result_json["ui_diff"] = json!(diff_result.diff);
            result_json["has_ui_changes"] = json!(diff_result.has_changes);
            if args.tree.ui_diff_include_full_trees_in_response.unwrap_or(false) {
                result_json["tree_before"] = json!(diff_result.tree_before);
                result_json["tree_after"] = json!(diff_result.tree_after);
            }
        }

        self.restore_window_management(should_restore).await;

        span.set_status(true, None);
        span.end();

        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(result_json)?],
                None,
            )
            .await,
        ))
    }

    // Removed: run_javascript tool (merged into run_command with engine)

    #[tool(
        description = "Execute JavaScript in a browser using the Chrome extension bridge. Full access to HTML DOM for data extraction, page analysis, and manipulation.

Alternative: In run_command with engine: javascript, use desktop.executeBrowserScript(script)
to execute browser scripts directly without needing a selector. Automatically targets active browser tab.

Parameters:
- script: JavaScript code to execute (optional if script_file is provided)
- script_file: Path to JavaScript file to load and execute (optional)
- env: Environment variables to inject as 'var env = {...}' (optional)
- outputs: Outputs from previous steps to inject as 'var outputs = {...}' (optional)

═══════════════════════════════════════════════════════════════════
COMMON BROWSER AUTOMATION PATTERNS
═══════════════════════════════════════════════════════════════════

Finding Elements:
  document.querySelector('.class')              // First match
  document.querySelectorAll('.class')           // All matches
  document.getElementById('id')                 // By ID
  document.querySelector('input[name=\"x\"]')     // By attribute
  document.querySelector('#form > button')      // CSS selectors
  document.forms[0]                             // First form
  document.links                                // All links
  document.images                               // All images

Extracting Data:
  element.innerText                             // Visible text only
  element.textContent                           // All text (including hidden)
  element.value                                 // Input/textarea/select value
  element.checked                               // Checkbox/radio state
  element.getAttribute('href')                  // Any attribute
  element.className                             // CSS classes
  element.id                                    // Element ID
  element.tagName                               // Tag name (e.g., 'DIV')
  
  // Extract from multiple elements
  Array.from(document.querySelectorAll('.item')).map(el => ({
    text: el.innerText,
    value: el.getAttribute('data-id')
  }))

Performing Actions:
  element.click()                               // Click element
  input.value = 'text to enter'                 // Fill input
  textarea.value = 'long text'                  // Fill textarea
  select.value = 'option2'                      // Select dropdown option
  checkbox.checked = true                       // Check checkbox
  element.focus()                               // Focus element
  element.blur()                                // Remove focus
  element.scrollIntoView()                      // Scroll to element
  element.scrollIntoView({ behavior: 'smooth' }) // Smooth scroll
  window.scrollTo(0, document.body.scrollHeight) // Scroll to bottom

Checking Element State:
  // Existence
  const exists = !!document.querySelector('.el')
  const exists = document.getElementById('id') !== null
  
  // Visibility
  const isVisible = element.offsetParent !== null
  const style = window.getComputedStyle(element)
  const isVisible = style.display !== 'none' && style.visibility !== 'hidden'
  
  // Form state
  const isDisabled = input.disabled
  const isRequired = input.required
  const isEmpty = input.value.trim() === ''
  
  // Position
  const rect = element.getBoundingClientRect()
  const isInViewport = rect.top >= 0 && rect.bottom <= window.innerHeight

Extracting Forms:
  // Get all forms and their inputs
  Array.from(document.forms).map(form => ({
    id: form.id,
    action: form.action,
    method: form.method,
    inputs: Array.from(form.elements).map(el => ({
      name: el.name,
      type: el.type,
      value: el.value,
      required: el.required
    }))
  }))

Extracting Tables:
  // Convert table to array of rows
  const table = document.querySelector('table')
  const rows = Array.from(table.querySelectorAll('tbody tr')).map(row => {
    const cells = Array.from(row.querySelectorAll('td'))
    return cells.map(cell => cell.innerText.trim())
  })

Extracting Links & Images:
  // All links with metadata
  Array.from(document.links).map(link => ({
    text: link.innerText,
    href: link.href,
    isExternal: link.hostname !== window.location.hostname
  }))
  
  // Images with alt text check
  Array.from(document.images).map(img => ({
    src: img.src,
    alt: img.alt || '[missing]',
    width: img.naturalWidth,
    height: img.naturalHeight
  }))

Detecting Page State:
  // Login detection
  const hasLoginForm = !!document.querySelector('form[action*=\"login\"], #loginForm')
  const hasUserMenu = !!document.querySelector('.user-menu, [class*=\"account\"]')
  const isLoggedIn = !hasLoginForm && hasUserMenu
  
  // Loading state
  const isLoading = !!document.querySelector('.spinner, .loading, [class*=\"loading\"]')
  
  // Framework detection
  const hasReact = !!document.querySelector('[data-reactroot], #root')
  const hasJQuery = typeof jQuery !== 'undefined' || typeof $ !== 'undefined'
  const hasAngular = !!document.querySelector('[ng-app], [data-ng-app]')

Waiting for Dynamic Content:
  // Wait for element to appear
  await new Promise((resolve) => {
    const checkInterval = setInterval(() => {
      const element = document.querySelector('.dynamic-content')
      if (element) {
        clearInterval(checkInterval)
        resolve(element)
      }
    }, 100) // Check every 100ms
  })
  
  // Wait for loading to finish
  await new Promise((resolve) => {
    const checkInterval = setInterval(() => {
      const loading = document.querySelector('.loading')
      if (!loading || loading.offsetParent === null) {
        clearInterval(checkInterval)
        resolve()
      }
    }, 100)
  })

Extracting Metadata:
  {
    url: window.location.href,
    title: document.title,
    description: document.querySelector('meta[name=\"description\"]')?.content,
    canonical: document.querySelector('link[rel=\"canonical\"]')?.href,
    language: document.documentElement.lang,
    charset: document.characterSet
  }

Working with iframes:
  // Execute inside iframe context
  const IFRAMESELCTOR = querySelector(\"#payment-frame\");
  // Your code now runs in iframe's document context
  const input = document.querySelector('input[name=\"cardnumber\"]')

═══════════════════════════════════════════════════════════════════
CRITICAL RULES
═══════════════════════════════════════════════════════════════════

Return Values:
  ❌ Don't return null/undefined - causes step failure
  ❌ Returning { success: false } causes step to FAIL (use intentionally to bail out)
  ✅ Always return JSON.stringify() for objects/arrays
  ✅ Return descriptive data for workflow branching
  
  Example:
  return JSON.stringify({
    login_required: 'true',      // For workflow 'if' conditions
    form_count: 3,
    page_loaded: 'true'
  })

Injected Variables (from previous steps):
  Always use typeof checks - variables injected with 'var':
  const config = (typeof user_config !== 'undefined') ? user_config : {}
  const items = (typeof item_list !== 'undefined') ? item_list : []

Console Logging:
  console.log('Debug:', data)     // Visible in extension logs
  console.error('Error:', err)    // Streamed to MCP agent
  console.warn('Warning:', msg)   // For debugging workflows

Async Operations:
  Both patterns work (auto-detected and awaited):
  
  // Async IIFE (auto-detected)
  (async function() {
    const text = await navigator.clipboard.readText()
    return JSON.stringify({ clipboard_text: text })
  })()
  
  // Promise chain
  navigator.clipboard.readText()
    .then(text => JSON.stringify({ clipboard_text: text }))
    .catch(err => JSON.stringify({ error: err.message }))
  
  CRITICAL: Both .then() and .catch() MUST return values!

Delays:
  await new Promise(resolve => setTimeout(resolve, 500))  // ✅ Works
  sleep(500)  // ❌ NOT available in browser context

Type Conversion:
  // ❌ Can't use string methods on objects
  if (data.toLowerCase().includes('error'))  // TypeError!
  
  // ✅ Stringify first
  if (JSON.stringify(data).toLowerCase().includes('error'))

Size Limits:
  Max 30KB response. Truncate large data:
  const html = document.documentElement.outerHTML
  return html.length > 30000 ? html.substring(0, 30000) + '...' : html

Navigation Timing:
  Separate navigation actions from return statements.
  Scripts triggering navigation (clicking links, form submit) can be killed
  before return executes, causing NULL_RESULT.
  
  ❌ Don't do this:
  button.click() // triggers navigation
  return JSON.stringify({ clicked: true }) // Never executes
  
  ✅ Do this:
  return JSON.stringify({ ready_to_navigate: true })
  // Let workflow handle navigation in next step

Examples: See browser_dom_extraction.yml and comprehensive_ui_test.yml
Requires Chrome extension to be installed."
    )]
    async fn execute_browser_script(
        &self,
        Parameters(args): Parameters<ExecuteBrowserScriptArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Start telemetry span
        let mut span = StepSpan::new("execute_browser_script", None);

        // Add comprehensive telemetry attributes
        if let Some(ref script) = args.script {
            span.set_attribute("script.length", script.len().to_string());
        }
        if let Some(ref script_file) = args.script_file {
            span.set_attribute("script_file", script_file.clone());
        }
        use serde_json::json;
        let start_instant = std::time::Instant::now();

        // Check if we need to perform window management (only for direct MCP calls, not sequences)
        let should_restore = {
            let in_sequence = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            let flag_value = *in_sequence;
            let should_restore_value = !flag_value;
            tracing::info!(
                "[execute_browser_script] Flag check: in_sequence={}, should_restore={}",
                flag_value,
                should_restore_value
            );
            should_restore_value
        };

        if should_restore {
            tracing::info!(
                "[execute_browser_script] Direct MCP call detected - performing window management"
            );
            let _ = self
                .prepare_window_management(
                    &args.selector.process,
                    None,
                    None,
                    None,
                    &args.window_mgmt,
                )
                .await;
        } else {
            tracing::debug!("[execute_browser_script] In sequence - skipping window management (dispatch_tool handles it)");
        }

        // Resolve the script content
        let script_content = if let Some(script_file) = &args.script_file {
            // Resolve script file with priority order (same logic as run_command)
            let resolved_path = {
                let script_path = std::path::Path::new(script_file);
                let mut resolved_path = None;
                let mut resolution_attempts = Vec::new();

                // Only resolve if path is relative
                if script_path.is_relative() {
                    tracing::info!(
                        "[SCRIPTS_BASE_PATH] Resolving relative browser script: '{}'",
                        script_file
                    );

                    // Priority 1: Try scripts_base_path if provided
                    let scripts_base_guard = self.current_scripts_base_path.lock().await;
                    if let Some(ref base_path) = *scripts_base_guard {
                        tracing::info!(
                            "[SCRIPTS_BASE_PATH] Checking scripts_base_path for browser script: {}",
                            base_path
                        );
                        let base = std::path::Path::new(base_path);
                        if base.exists() && base.is_dir() {
                            let candidate = base.join(script_file);
                            resolution_attempts
                                .push(format!("scripts_base_path: {}", candidate.display()));
                            tracing::info!(
                                "[SCRIPTS_BASE_PATH] Looking for browser script at: {}",
                                candidate.display()
                            );
                            if candidate.exists() {
                                tracing::info!(
                                    "[SCRIPTS_BASE_PATH] ✓ Found browser script in scripts_base_path: {} -> {}",
                                    script_file,
                                    candidate.display()
                                );
                                resolved_path = Some(candidate);
                            } else {
                                tracing::info!(
                                    "[SCRIPTS_BASE_PATH] ✗ Browser script not found in scripts_base_path: {}",
                                    candidate.display()
                                );
                            }
                        } else {
                            tracing::warn!(
                                "[SCRIPTS_BASE_PATH] Base path does not exist or is not a directory: {}",
                                base_path
                            );
                        }
                    } else {
                        tracing::debug!(
                            "[SCRIPTS_BASE_PATH] No scripts_base_path configured for browser script"
                        );
                    }
                    drop(scripts_base_guard);

                    // Priority 2: Try workflow directory if not found yet
                    if resolved_path.is_none() {
                        let workflow_dir_guard = self.current_workflow_dir.lock().await;
                        if let Some(ref workflow_dir) = *workflow_dir_guard {
                            let candidate = workflow_dir.join(script_file);
                            resolution_attempts
                                .push(format!("workflow_dir: {}", candidate.display()));
                            if candidate.exists() {
                                tracing::info!(
                                    "[execute_browser_script] Resolved via workflow directory: {} -> {}",
                                    script_file,
                                    candidate.display()
                                );
                                resolved_path = Some(candidate);
                            }
                        }
                    }

                    // Priority 3: Check current directory or use as-is
                    if resolved_path.is_none() {
                        let candidate = script_path.to_path_buf();
                        resolution_attempts.push(format!("as-is: {}", candidate.display()));

                        // Check if file exists before using it
                        if candidate.exists() {
                            tracing::info!(
                                "[execute_browser_script] Found script file at: {}",
                                candidate.display()
                            );
                            resolved_path = Some(candidate);
                        } else {
                            tracing::warn!(
                                "[execute_browser_script] Script file not found: {} (tried: {:?})",
                                script_file,
                                resolution_attempts
                            );
                            // Return error immediately for missing file
                            return Err(McpError::invalid_params(
                                format!("Script file '{script_file}' not found"),
                                Some(json!({
                                    "file": script_file,
                                    "resolution_attempts": resolution_attempts,
                                    "error": "File does not exist"
                                })),
                            ));
                        }
                    }
                } else {
                    // Absolute path - check if exists
                    let candidate = script_path.to_path_buf();
                    if candidate.exists() {
                        tracing::info!(
                            "[execute_browser_script] Using absolute path: {}",
                            script_file
                        );
                        resolved_path = Some(candidate);
                    } else {
                        tracing::warn!(
                            "[execute_browser_script] Absolute script file not found: {}",
                            script_file
                        );
                        return Err(McpError::invalid_params(
                            format!("Script file '{script_file}' not found"),
                            Some(json!({
                                "file": script_file,
                                "error": "File does not exist at absolute path"
                            })),
                        ));
                    }
                }

                resolved_path.unwrap()
            };

            // Read script from resolved file path
            tokio::fs::read_to_string(&resolved_path)
                .await
                .map_err(|e| {
                    McpError::invalid_params(
                        "Failed to read script file",
                        Some(json!({
                            "file": script_file,
                            "resolved_path": resolved_path.to_string_lossy(),
                            "error": e.to_string()
                        })),
                    )
                })?
        } else if let Some(script) = &args.script {
            if script.is_empty() {
                // Restore windows before returning error
                self.restore_window_management(should_restore).await;
                return Err(McpError::invalid_params("Script cannot be empty", None));
            }
            script.clone()
        } else {
            // Restore windows before returning error
            self.restore_window_management(should_restore).await;
            return Err(McpError::invalid_params(
                "Either 'script' or 'script_file' must be provided",
                None,
            ));
        };

        // Build the final script with env prepended if provided
        let mut final_script = String::new();

        // Extract workflow variables and accumulated env from special env keys
        let mut variables_json = "{}".to_string();
        let mut accumulated_env_json = "{}".to_string();
        let mut env_data = args.env.clone();

        if let Some(env) = &env_data {
            if let Some(env_obj) = env.as_object() {
                // Extract workflow variables
                if let Some(vars) = env_obj.get("_workflow_variables") {
                    variables_json =
                        serde_json::to_string(vars).unwrap_or_else(|_| "{}".to_string());
                }
                // Extract accumulated env
                if let Some(acc_env) = env_obj.get("_accumulated_env") {
                    accumulated_env_json =
                        serde_json::to_string(acc_env).unwrap_or_else(|_| "{}".to_string());
                }
            }
        }

        // Remove special keys from env before normal processing
        if let Some(env) = &mut env_data {
            if let Some(env_obj) = env.as_object_mut() {
                env_obj.remove("_workflow_variables");
                env_obj.remove("_accumulated_env");
            }
        }

        // Prepare explicit env if provided
        let explicit_env_json = if let Some(env) = &env_data {
            if env.as_object().is_some_and(|o| !o.is_empty()) {
                serde_json::to_string(&env).map_err(|e| {
                    McpError::internal_error(
                        "Failed to serialize env data",
                        Some(json!({"error": e.to_string()})),
                    )
                })?
            } else {
                "{}".to_string()
            }
        } else {
            "{}".to_string()
        };

        // Inject accumulated env first
        final_script.push_str(&format!("var env = {accumulated_env_json};\n"));

        // Merge explicit env if provided
        if explicit_env_json != "{}" {
            final_script.push_str(&format!("env = Object.assign(env, {explicit_env_json});\n"));
        }

        // Inject individual variables from env (browser scripts are always JavaScript)
        let merged_env = if explicit_env_json != "{}" {
            // Merge accumulated and explicit env for individual vars
            let mut base: serde_json::Map<String, serde_json::Value> =
                serde_json::from_str(&accumulated_env_json).unwrap_or_default();
            if let Ok(explicit) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(
                &explicit_env_json,
            ) {
                base.extend(explicit);
            }
            serde_json::to_string(&base).unwrap_or_else(|_| "{}".to_string())
        } else {
            accumulated_env_json.clone()
        };

        // Track which variables will be injected
        let mut injected_vars = std::collections::HashSet::new();

        if let Ok(env_obj) =
            serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&merged_env)
        {
            for (key, value) in env_obj {
                // Inject all valid JavaScript identifiers from env
                // The IIFE wrapper prevents conflicts with previous script executions
                if Self::is_valid_js_identifier(&key) {
                    // Smart handling of potentially double-stringified JSON
                    let injectable_value = if let Some(str_val) = value.as_str() {
                        let trimmed = str_val.trim();
                        // Check if it looks like JSON (object or array)
                        if (trimmed.starts_with('{') && trimmed.ends_with('}'))
                            || (trimmed.starts_with('[') && trimmed.ends_with(']'))
                        {
                            // Try to parse as JSON to avoid double stringification
                            match serde_json::from_str::<serde_json::Value>(str_val) {
                                Ok(parsed) => {
                                    tracing::debug!(
                                        "[execute_browser_script] Detected JSON string for env.{}, parsing to avoid double stringification",
                                        key
                                    );
                                    parsed
                                }
                                Err(_) => {
                                    // Not valid JSON despite looking like it, keep as string
                                    value.clone()
                                }
                            }
                        } else {
                            // Regular string value, keep as is
                            value.clone()
                        }
                    } else {
                        // Not a string (number, bool, object, etc.), keep as is
                        value.clone()
                    };

                    // Now stringify for injection (single level of stringification)
                    if let Ok(value_json) = serde_json::to_string(&injectable_value) {
                        final_script.push_str(&format!("var {key} = {value_json};\n"));
                        injected_vars.insert(key.clone()); // Track this variable
                    }
                }
            }
        }

        // Inject variables
        final_script.push_str(&format!("var variables = {variables_json};\n"));

        // Parse and inject individual workflow variables
        if let Ok(variables_obj) =
            serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&variables_json)
        {
            for (key, value) in variables_obj {
                // Inject all valid JavaScript identifiers from variables
                if Self::is_valid_js_identifier(&key) {
                    // Smart handling of potentially double-stringified JSON
                    let injectable_value = if let Some(str_val) = value.as_str() {
                        let trimmed = str_val.trim();
                        // Check if it looks like JSON (object or array)
                        if (trimmed.starts_with('{') && trimmed.ends_with('}'))
                            || (trimmed.starts_with('[') && trimmed.ends_with(']'))
                        {
                            // Try to parse as JSON to avoid double stringification
                            match serde_json::from_str::<serde_json::Value>(str_val) {
                                Ok(parsed) => {
                                    tracing::debug!(
                                        "[execute_browser_script] Detected JSON string for variables.{}, parsing to avoid double stringification",
                                        key
                                    );
                                    parsed
                                }
                                Err(_) => {
                                    // Not valid JSON despite looking like it, keep as string
                                    value.clone()
                                }
                            }
                        } else {
                            // Regular string value, keep as is
                            value.clone()
                        }
                    } else {
                        // Not a string (number, bool, object, etc.), keep as is
                        value.clone()
                    };

                    // Now stringify for injection (single level of stringification)
                    if let Ok(value_json) = serde_json::to_string(&injectable_value) {
                        final_script.push_str(&format!("var {key} = {value_json};\n"));
                        injected_vars.insert(key.clone()); // Track this variable for smart replacement
                    }
                }
            }
        }

        tracing::debug!("[execute_browser_script] Injected accumulated env, explicit env, individual vars, and workflow variables");

        // Smart replacement of declarations with assignments for already-injected variables
        let mut modified_script = script_content.clone();
        if !injected_vars.is_empty() {
            tracing::info!(
                "[execute_browser_script] Checking for variable declarations to replace. Injected vars count: {}",
                injected_vars.len()
            );

            for var_name in &injected_vars {
                // Create regex to match declarations of this variable
                // Matches: const varName =, let varName =, var varName =
                // With optional whitespace, handling line start
                let pattern = format!(
                    r"(?m)^(\s*)(const|let|var)\s+{}\s*=",
                    regex::escape(var_name)
                );

                if let Ok(re) = Regex::new(&pattern) {
                    let before = modified_script.clone();
                    modified_script = re
                        .replace_all(&modified_script, format!("${{1}}{var_name} ="))
                        .to_string();

                    if before != modified_script {
                        tracing::info!(
                            "[execute_browser_script] Replaced declaration of '{}' with assignment to avoid redeclaration error",
                            var_name
                        );
                    }
                }
            }

            // Log first 500 chars of modified script for debugging
            let preview: String = modified_script.chars().take(500).collect();
            tracing::debug!(
                "[execute_browser_script] Modified script preview after replacements: {}...",
                preview
            );
        }

        // Validate that browser scripts don't use top-level return statements
        if modified_script.trim_start().starts_with("return ") {
            // Restore windows before returning error
            self.restore_window_management(should_restore).await;
            return Err(McpError::invalid_params(
                "Browser scripts cannot use top-level 'return' statements. \
                 Remove 'return' from the beginning of your script. \
                 Example: Use '(async function() {...})()' instead of 'return (async function() {...})()'",
                None
            ));
        }

        let cleaned_script = modified_script;

        // Check if console log capture is enabled
        let include_logs = args.include_logs.unwrap_or(false);

        if include_logs {
            // Inject console capture wrapper
            final_script.push_str(
                r#"
// Console capture wrapper (auto-injected when include_logs=true)
var __terminator_logs__ = [];
var __terminator_console__ = {
  log: console.log,
  warn: console.warn,
  error: console.error,
  info: console.info,
  debug: console.debug
};

console.log = function(...args) {
  __terminator_logs__.push(['log', ...args.map(a => {
    try { return typeof a === 'object' ? JSON.stringify(a) : String(a); }
    catch(e) { return String(a); }
  })]);
  __terminator_console__.log.apply(console, args);
};
console.error = function(...args) {
  __terminator_logs__.push(['error', ...args.map(a => {
    try { return typeof a === 'object' ? JSON.stringify(a) : String(a); }
    catch(e) { return String(a); }
  })]);
  __terminator_console__.error.apply(console, args);
};
console.warn = function(...args) {
  __terminator_logs__.push(['warn', ...args.map(a => {
    try { return typeof a === 'object' ? JSON.stringify(a) : String(a); }
    catch(e) { return String(a); }
  })]);
  __terminator_console__.warn.apply(console, args);
};
console.info = function(...args) {
  __terminator_logs__.push(['info', ...args.map(a => {
    try { return typeof a === 'object' ? JSON.stringify(a) : String(a); }
    catch(e) { return String(a); }
  })]);
  __terminator_console__.info.apply(console, args);
};

"#,
            );

            // Wrap user script to capture result + logs
            // The user script becomes the last expression which eval() will return
            final_script.push_str("(function() {\n");
            final_script.push_str("  var __user_result__ = (");
            final_script.push_str(&cleaned_script);
            final_script.push_str(");\n");
            final_script.push_str("  return JSON.stringify({\n");
            final_script.push_str("    result: __user_result__,\n");
            final_script.push_str("    logs: __terminator_logs__\n");
            final_script.push_str("  });\n");
            final_script.push_str("})()");
        } else {
            // Append the cleaned script without wrapper
            final_script.push_str(&cleaned_script);
        }
        let script_len = final_script.len();
        let script_preview: String = final_script.chars().take(200).collect();
        tracing::info!(
            "[execute_browser_script] start selector='{}' timeout_ms={:?} retries={:?} script_bytes={}",
            args.selector.selector,
            args.action.timeout_ms,
            args.action.retries,
            script_len
        );
        tracing::debug!(
            "[execute_browser_script] script_preview: {}",
            script_preview
        );

        let script_clone = final_script.clone();
        let ((script_result, element), successful_selector) =
            match crate::utils::find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector.build_full_selector(),
                args.selector.build_alternative_selectors().as_deref(),
                args.selector.build_fallback_selectors().as_deref(),
                args.action.timeout_ms,
                args.action.retries,
                |el| {
                    let script = script_clone.clone();
                    async move { el.execute_browser_script(&script).await }
                },
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => {
                    // Use warn! since browser script failures are often expected (extension not installed, user script errors)
                    tracing::warn!(
                        "[execute_browser_script] failed selector='{}' alt='{:?}' fallback='{:?}' error={}",
                        args.selector.selector,
                        args.selector.alternative_selectors,
                        args.selector.fallback_selectors,
                        e
                    );

                    // Check if this is a JavaScript execution error or extension bridge error
                    if let Some(AutomationError::PlatformError(msg)) =
                        e.downcast_ref::<AutomationError>()
                    {
                        if msg.contains("JavaScript") || msg.contains("script") {
                            // Return JavaScript-specific error, not "Element not found"
                            // Restore windows before returning error
                            self.restore_window_management(should_restore).await;
                            return Err(McpError::invalid_params(
                                "Browser script execution failed",
                                Some(json!({
                                    "error_type": "script_execution_failure",
                                    "message": msg.clone(),
                                    "selector": args.selector.selector,
                                    "selectors_tried": get_selectors_tried_all(
                                        &args.selector.build_full_selector(),
                                        args.selector.build_alternative_selectors().as_deref(),
                                        args.selector.build_fallback_selectors().as_deref(),
                                    ),
                                    "suggestion": "Check the browser console for JavaScript errors. The script may have timed out or encountered an error."
                                })),
                            ));
                        }
                        // Check for extension bridge connection errors
                        if msg.contains("extension")
                            || msg.contains("bridge")
                            || msg.contains("port")
                            || msg.contains("bind")
                            || msg.contains("client")
                        {
                            self.restore_window_management(should_restore).await;
                            return Err(McpError::invalid_params(
                                "Browser extension connection failed",
                                Some(json!({
                                    "error_type": "extension_connection_failure",
                                    "message": msg.clone(),
                                    "selector": args.selector.selector,
                                    "suggestion": "The Chrome extension bridge failed to connect. Try: 1) Kill all terminator-mcp-agent processes, 2) Refresh the browser tab, 3) Check if Terminator Bridge extension is installed and active."
                                })),
                            ));
                        }
                    }

                    // For other errors, treat as element not found
                    // Restore windows before returning error
                    self.restore_window_management(should_restore).await;
                    Err(build_element_not_found_error(
                        &args.selector.build_full_selector(),
                        args.selector.build_alternative_selectors().as_deref(),
                        args.selector.build_fallback_selectors().as_deref(),
                        e,
                    ))
                }
            }?;
        let elapsed_ms = start_instant.elapsed().as_millis() as u64;
        tracing::info!(
            "[execute_browser_script] target resolved selector='{}' role='{}' name='{}' pid={} in {}ms",
            successful_selector,
            element.role(),
            element.name().unwrap_or_default(),
            element.process_id().unwrap_or(0),
            elapsed_ms
        );

        let selectors_tried = get_selectors_tried_all(
            &args.selector.build_full_selector(),
            args.selector.build_alternative_selectors().as_deref(),
            args.selector.build_fallback_selectors().as_deref(),
        );

        // Parse script_result to extract result and logs if console capture was enabled
        let (actual_result, captured_logs) = if include_logs {
            // Try to parse the wrapped result
            match serde_json::from_str::<serde_json::Value>(&script_result) {
                Ok(parsed) => {
                    let result = parsed.get("result").cloned().unwrap_or_else(|| {
                        // If no result field, use the whole parsed value
                        parsed.clone()
                    });
                    let logs = parsed.get("logs").cloned();
                    (result, logs)
                }
                Err(e) => {
                    // Failed to parse - script might have returned non-JSON
                    // Fall back to treating the whole result as the actual result
                    tracing::warn!(
                        "[execute_browser_script] Failed to parse wrapped result, falling back to raw result. Error: {}",
                        e
                    );
                    (json!(script_result), None)
                }
            }
        } else {
            // No wrapping, use script_result as-is
            (json!(script_result), None)
        };

        let mut result_json = json!({
            "action": "execute_browser_script",
            "status": "success",
            "selector": successful_selector,
            "selector_used": successful_selector,
            "selectors_tried": selectors_tried,
            "element": build_element_info(&element),
            "script": "[script content omitted to reduce verbosity]",
            "script_file": args.script_file,
            "env_provided": args.env.is_some(),
            "result": actual_result,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "duration_ms": elapsed_ms,
            "script_bytes": script_len,
        });

        // Include logs if they were captured
        if let Some(logs) = captured_logs {
            result_json["logs"] = logs;
        }

        // Note: Tree attachment removed - use ui_diff_before_after for tree context

        // Restore windows after executing browser script
        self.restore_window_management(should_restore).await;

        span.set_status(true, None);
        span.end();

        Ok(CallToolResult::success(
            append_monitor_screenshots_if_enabled(
                &self.desktop,
                vec![Content::json(result_json)?],
                None,
            )
            .await,
        ))
    }

    #[tool(
        description = "Stops all currently executing workflows/tools by cancelling active requests. Use this when the user clicks a stop button or wants to abort execution."
    )]
    async fn stop_execution(&self) -> Result<CallToolResult, McpError> {
        info!("🛑 Stop execution requested - cancelling all active requests");

        // Cancel all active requests using the request manager
        self.request_manager.cancel_all().await;

        let active_count = self.request_manager.active_count().await;
        info!(
            "✅ Cancelled all active requests. Active count: {}",
            active_count
        );

        let result_json = json!({
            "action": "stop_execution",
            "status": "success",
            "message": "All active requests have been cancelled",
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }
}

impl DesktopWrapper {
    pub(crate) async fn dispatch_tool(
        &self,
        peer: Peer<RoleServer>,
        request_context: RequestContext<RoleServer>,
        tool_name: &str,
        arguments: &serde_json::Value,
        execution_context: Option<crate::utils::ToolExecutionContext>,
    ) -> Result<CallToolResult, McpError> {
        use rmcp::handler::server::wrapper::Parameters;

        // Check if request is already cancelled before dispatching
        if request_context.ct.is_cancelled() {
            return Err(McpError::internal_error(
                format!("Tool {tool_name} cancelled before execution"),
                Some(json!({"code": -32001, "tool": tool_name})),
            ));
        }

        // Window management for UI interaction tools
        // Check if tool has a 'process' argument - if so, it needs window management
        // No whitelist - any tool with a process argument gets window management
        let process_name = arguments
            .get("process")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Extract window management options from arguments (defaults to enabled)
        let window_mgmt_opts: crate::utils::WindowManagementOptions =
            serde_json::from_value(arguments.clone()).unwrap_or_default();

        // Perform window management if needed
        if let Some(ref process) = process_name {
            let _ = self
                .prepare_window_management(
                    process,
                    execution_context.as_ref(),
                    None,
                    None,
                    &window_mgmt_opts,
                )
                .await;

            // Small delay to let window operations settle
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }

        // Set in_sequence flag to prevent individual tools from doing their own window management
        // dispatch_tool handles window management centrally
        {
            let mut in_seq = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            *in_seq = true;
        }

        // Wrap each tool call with cancellation support
        let result = match tool_name {
            "get_window_tree" => {
                match serde_json::from_value::<GetWindowTreeArgs>(arguments.clone()) {
                    Ok(args) => {
                        // Use tokio::select with the cancellation token from request_context
                        tokio::select! {
                            result = self.get_window_tree(Parameters(args)) => result,
                            _ = request_context.ct.cancelled() => {
                                Err(McpError::internal_error(
                                    format!("{tool_name} cancelled"),
                                    Some(json!({"code": -32001, "tool": tool_name}))
                                ))
                            }
                        }
                    }
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for get_window_tree",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "get_applications_and_windows_list" => {
                match serde_json::from_value::<GetApplicationsArgs>(arguments.clone()) {
                    Ok(args) => {
                        tokio::select! {
                            result = self.get_applications_and_windows_list(Parameters(args)) => result,
                            _ = request_context.ct.cancelled() => {
                                Err(McpError::internal_error(
                                    format!("{tool_name} cancelled"),
                                    Some(json!({"code": -32001, "tool": tool_name}))
                                ))
                            }
                        }
                    }
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for get_applications_and_windows_list",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "click_element" => {
                match serde_json::from_value::<ClickElementArgs>(arguments.clone()) {
                    Ok(args) => {
                        tokio::select! {
                            result = self.click_element(Parameters(args)) => result,
                            _ = request_context.ct.cancelled() => {
                                Err(McpError::internal_error(
                                    format!("{tool_name} cancelled"),
                                    Some(json!({"code": -32001, "tool": tool_name}))
                                ))
                            }
                        }
                    }
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for click_element",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "type_into_element" => {
                match serde_json::from_value::<TypeIntoElementArgs>(arguments.clone()) {
                    Ok(args) => {
                        tokio::select! {
                            result = self.type_into_element(Parameters(args)) => result,
                            _ = request_context.ct.cancelled() => {
                                Err(McpError::internal_error(
                                    format!("{tool_name} cancelled"),
                                    Some(json!({"code": -32001, "tool": tool_name}))
                                ))
                            }
                        }
                    }
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for type_into_element",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "press_key" => match serde_json::from_value::<PressKeyArgs>(arguments.clone()) {
                Ok(args) => {
                    tokio::select! {
                        result = self.press_key(Parameters(args)) => result,
                        _ = request_context.ct.cancelled() => {
                            Err(McpError::internal_error(
                                format!("{tool_name} cancelled"),
                                Some(json!({"code": -32001, "tool": tool_name}))
                            ))
                        }
                    }
                }
                Err(e) => Err(McpError::invalid_params(
                    "Invalid arguments for press_key",
                    Some(json!({"error": e.to_string()})),
                )),
            },
            "press_key_global" => {
                match serde_json::from_value::<GlobalKeyArgs>(arguments.clone()) {
                    Ok(args) => self.press_key_global(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for press_key_global",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "validate_element" => {
                match serde_json::from_value::<ValidateElementArgs>(arguments.clone()) {
                    Ok(args) => self.validate_element(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for validate_element",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "wait_for_element" => {
                match serde_json::from_value::<WaitForElementArgs>(arguments.clone()) {
                    Ok(args) => {
                        tokio::select! {
                            result = self.wait_for_element(Parameters(args)) => result,
                            _ = request_context.ct.cancelled() => {
                                Err(McpError::internal_error(
                                    format!("{tool_name} cancelled"),
                                    Some(json!({"code": -32001, "tool": tool_name}))
                                ))
                            }
                        }
                    }
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for wait_for_element",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }

            "activate_element" => {
                match serde_json::from_value::<ActivateElementArgs>(arguments.clone()) {
                    Ok(args) => self.activate_element(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for activate_element",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "navigate_browser" => {
                match serde_json::from_value::<NavigateBrowserArgs>(arguments.clone()) {
                    Ok(args) => {
                        tokio::select! {
                            result = self.navigate_browser(Parameters(args)) => result,
                            _ = request_context.ct.cancelled() => {
                                Err(McpError::internal_error(
                                    format!("{tool_name} cancelled"),
                                    Some(json!({"code": -32001, "tool": tool_name}))
                                ))
                            }
                        }
                    }
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for navigate_browser",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "execute_browser_script" => {
                match serde_json::from_value::<ExecuteBrowserScriptArgs>(arguments.clone()) {
                    Ok(args) => self.execute_browser_script(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for execute_browser_script",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "open_application" => {
                match serde_json::from_value::<OpenApplicationArgs>(arguments.clone()) {
                    Ok(args) => self.open_application(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for open_application",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "scroll_element" => {
                match serde_json::from_value::<ScrollElementArgs>(arguments.clone()) {
                    Ok(args) => self.scroll_element(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for scroll_element",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "delay" => match serde_json::from_value::<DelayArgs>(arguments.clone()) {
                Ok(args) => self.delay(Parameters(args)).await,
                Err(e) => Err(McpError::invalid_params(
                    "Invalid arguments for delay",
                    Some(json!({"error": e.to_string()})),
                )),
            },
            "run_command" => match serde_json::from_value::<RunCommandArgs>(arguments.clone()) {
                Ok(args) => {
                    // Create a child cancellation token from the request context
                    let cancellation_token = tokio_util::sync::CancellationToken::new();
                    let child_token = cancellation_token.child_token();

                    // Link it to the request context cancellation
                    let ct_for_task = request_context.ct.clone();
                    tokio::spawn(
                        async move {
                            ct_for_task.cancelled().await;
                            cancellation_token.cancel();
                        }
                        .in_current_span(),
                    );

                    self.run_command_impl(args, Some(child_token)).await
                }
                Err(e) => Err(McpError::invalid_params(
                    "Invalid arguments for run_command",
                    Some(json!({"error": e.to_string()})),
                )),
            },
            "mouse_drag" => match serde_json::from_value::<MouseDragArgs>(arguments.clone()) {
                Ok(args) => self.mouse_drag(Parameters(args)).await,
                Err(e) => Err(McpError::invalid_params(
                    "Invalid arguments for mouse_drag",
                    Some(json!({"error": e.to_string()})),
                )),
            },
            "click_element_by_index" | "click_index" | "click_cv_index" => {
                match serde_json::from_value::<crate::utils::ClickIndexArgs>(arguments.clone()) {
                    Ok(args) => self.click_element_by_index(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for click_element_by_index",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "highlight_element" => {
                match serde_json::from_value::<HighlightElementArgs>(arguments.clone()) {
                    Ok(args) => self.highlight_element(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for highlight_element",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "select_option" => {
                match serde_json::from_value::<crate::utils::SelectOptionArgs>(arguments.clone()) {
                    Ok(args) => self.select_option(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for select_option",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "list_options" => match serde_json::from_value::<LocatorArgs>(arguments.clone()) {
                Ok(args) => self.list_options(Parameters(args)).await,
                Err(e) => Err(McpError::invalid_params(
                    "Invalid arguments for list_options",
                    Some(json!({"error": e.to_string()})),
                )),
            },
            "set_toggled" => match serde_json::from_value::<SetToggledArgs>(arguments.clone()) {
                Ok(args) => self.set_toggled(Parameters(args)).await,
                Err(e) => Err(McpError::invalid_params(
                    "Invalid arguments for set_toggled",
                    Some(json!({"error": e.to_string()})),
                )),
            },
            "set_range_value" => {
                match serde_json::from_value::<SetRangeValueArgs>(arguments.clone()) {
                    Ok(args) => self.set_range_value(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for set_range_value",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "set_selected" => match serde_json::from_value::<SetSelectedArgs>(arguments.clone()) {
                Ok(args) => self.set_selected(Parameters(args)).await,
                Err(e) => Err(McpError::invalid_params(
                    "Invalid arguments for set_selected",
                    Some(json!({"error": e.to_string()})),
                )),
            },
            "is_toggled" => match serde_json::from_value::<LocatorArgs>(arguments.clone()) {
                Ok(args) => self.is_toggled(Parameters(args)).await,
                Err(e) => Err(McpError::invalid_params(
                    "Invalid arguments for is_toggled",
                    Some(json!({"error": e.to_string()})),
                )),
            },
            "get_range_value" => match serde_json::from_value::<LocatorArgs>(arguments.clone()) {
                Ok(args) => self.get_range_value(Parameters(args)).await,
                Err(e) => Err(McpError::invalid_params(
                    "Invalid arguments for get_range_value",
                    Some(json!({"error": e.to_string()})),
                )),
            },
            "is_selected" => match serde_json::from_value::<LocatorArgs>(arguments.clone()) {
                Ok(args) => self.is_selected(Parameters(args)).await,
                Err(e) => Err(McpError::invalid_params(
                    "Invalid arguments for is_selected",
                    Some(json!({"error": e.to_string()})),
                )),
            },
            "capture_element_screenshot" => {
                match serde_json::from_value::<CaptureElementScreenshotArgs>(arguments.clone()) {
                    Ok(args) => self.capture_element_screenshot(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for capture_element_screenshot",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "invoke_element" => {
                match serde_json::from_value::<InvokeElementArgs>(arguments.clone()) {
                    Ok(args) => self.invoke_element(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for invoke_element",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "maximize_window" => {
                match serde_json::from_value::<MaximizeWindowArgs>(arguments.clone()) {
                    Ok(args) => self.maximize_window(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for maximize_window",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "minimize_window" => {
                match serde_json::from_value::<MinimizeWindowArgs>(arguments.clone()) {
                    Ok(args) => self.minimize_window(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for minimize_window",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "set_zoom" => match serde_json::from_value::<SetZoomArgs>(arguments.clone()) {
                Ok(args) => self.set_zoom(Parameters(args)).await,
                Err(e) => Err(McpError::invalid_params(
                    "Invalid arguments for set_zoom",
                    Some(json!({ "error": e.to_string() })),
                )),
            },
            "set_value" => match serde_json::from_value::<SetValueArgs>(arguments.clone()) {
                Ok(args) => self.set_value(Parameters(args)).await,
                Err(e) => Err(McpError::invalid_params(
                    "Invalid arguments for set_value",
                    Some(json!({ "error": e.to_string() })),
                )),
            },
            // run_javascript is deprecated and merged into run_command with engine
            "execute_sequence" => {
                // Handle nested execute_sequence calls by delegating to execute_sequence_impl
                // Use Box::pin to handle async recursion (dispatch_tool -> execute_sequence_impl -> ... -> dispatch_tool)
                match serde_json::from_value::<ExecuteSequenceArgs>(arguments.clone()) {
                    Ok(args) => {
                        Box::pin(self.execute_sequence_impl(peer, request_context, args)).await
                    }
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for execute_sequence",
                        Some(json!({ "error": e.to_string() })),
                    )),
                }
            }
            "stop_highlighting" => {
                match serde_json::from_value::<StopHighlightingArgs>(arguments.clone()) {
                    Ok(args) => self.stop_highlighting(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for stop_highlighting",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "stop_execution" => {
                // No arguments needed for stop_execution
                self.stop_execution().await
            }
            _ => Err(McpError::internal_error(
                "Unknown tool called",
                Some(json!({"tool_name": tool_name})),
            )),
        };

        // Reset in_sequence flag after tool execution
        {
            let mut in_seq = self.in_sequence.lock().unwrap_or_else(|e| e.into_inner());
            *in_seq = false;
        }

        // Restore windows appropriately based on execution context
        match execution_context {
            None => {
                // Single tool execution: always restore if window management was performed
                if process_name.is_some() {
                    if let Err(e) = self.window_manager.restore_all_windows().await {
                        tracing::warn!("Failed to restore windows: {}", e);
                    }
                    // Clear captured state after restoration
                    self.window_manager.clear_captured_state().await;
                    tracing::info!("Restored all windows to original state (single tool)");
                }
            }
            Some(ref ctx) => {
                // Sequence execution: only restore on last step
                if ctx.is_last_step && process_name.is_some() {
                    if let Err(e) = self.window_manager.restore_all_windows().await {
                        tracing::warn!("Failed to restore windows: {}", e);
                    }
                    // Clear captured state after restoration
                    self.window_manager.clear_captured_state().await;
                    tracing::info!("Restored all windows to original state (sequence last step)");
                }
            }
        }

        result
    }
}

#[tool_handler]
impl ServerHandler for DesktopWrapper {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::LATEST,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(crate::prompt::get_server_instructions().to_string()),
        }
    }
}
