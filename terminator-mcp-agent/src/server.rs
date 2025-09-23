use crate::helpers::*;
use crate::scripting_engine;
use crate::utils::find_and_execute_with_retry_with_fallback;
pub use crate::utils::DesktopWrapper;
use crate::utils::{
    get_timeout, ActionHighlightConfig, ActivateElementArgs, ClickElementArgs, CloseElementArgs,
    DelayArgs, ExecuteBrowserScriptArgs, ExecuteSequenceArgs, ExportWorkflowSequenceArgs,
    GetApplicationsArgs, GetFocusedWindowTreeArgs, GetWindowTreeArgs, GlobalKeyArgs,
    HighlightElementArgs, ImportWorkflowSequenceArgs, LocatorArgs, MaximizeWindowArgs,
    MinimizeWindowArgs, MouseDragArgs, NavigateBrowserArgs, OpenApplicationArgs, PressKeyArgs,
    RecordWorkflowArgs, RunCommandArgs, ScrollElementArgs, SelectOptionArgs, SetRangeValueArgs,
    SetSelectedArgs, SetToggledArgs, SetValueArgs, SetZoomArgs, StopHighlightingArgs,
    TypeIntoElementArgs, ValidateElementArgs, WaitForElementArgs, ZoomArgs,
};
use futures::StreamExt;
use image::{ExtendedColorType, ImageEncoder};
use regex::Regex;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
};
use rmcp::{tool, ErrorData as McpError, ServerHandler};
use rmcp::{tool_handler, tool_router};
use serde_json::json;
use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;
use terminator::{AutomationError, Browser, Desktop, Selector, UIElement};
use terminator_workflow_recorder::{PerformanceMode, WorkflowRecorder, WorkflowRecorderConfig};
use tokio::sync::Mutex;
use tracing::{info, warn};

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
    /// Helper function to determine include_tree value
    /// Checks the INCLUDE_TREE environment variable
    /// If not present, defaults to true
    /// Otherwise uses the environment variable value
    fn get_include_tree_default(args_value: Option<bool>) -> bool {
        // First check if args explicitly set a value
        if let Some(value) = args_value {
            return value;
        }

        // Then check environment variable
        match std::env::var("INCLUDE_TREE") {
            Ok(val) => {
                // Parse the string value to bool
                match val.to_lowercase().as_str() {
                    "true" | "1" | "yes" | "on" => true,
                    "false" | "0" | "no" | "off" => false,
                    _ => true, // Default to true if invalid value
                }
            }
            Err(_) => true, // Default to true if env var not present
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
        log_capture: Option<crate::log_capture::LogCapture>,
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
            recorder: Arc::new(Mutex::new(None)),
            active_highlights: Arc::new(Mutex::new(Vec::new())),
            log_capture,
            current_workflow_dir: Arc::new(Mutex::new(None)),
            current_scripts_base_path: Arc::new(Mutex::new(None)),
        })
    }

    /// Create TreeBuildConfig based on include_detailed_attributes parameter
    /// Defaults to comprehensive attributes for LLM usage if include_detailed_attributes is not specified
    fn create_tree_config(
        include_detailed_attributes: Option<bool>,
    ) -> terminator::platforms::TreeBuildConfig {
        let include_detailed = include_detailed_attributes.unwrap_or(true);

        if include_detailed {
            terminator::platforms::TreeBuildConfig {
                property_mode: terminator::platforms::PropertyLoadingMode::Complete,
                timeout_per_operation_ms: Some(100), // Slightly higher timeout for detailed loading
                yield_every_n_elements: Some(25),    // More frequent yielding for responsiveness
                batch_size: Some(25),
            }
        } else {
            terminator::platforms::TreeBuildConfig::default() // Fast mode
        }
    }

    #[tool(
        description = "Get the complete UI tree for an application by PID and optional window title. This is your primary tool for understanding the application's current state. This is a read-only operation."
    )]
    pub async fn get_window_tree(
        &self,
        Parameters(args): Parameters<GetWindowTreeArgs>,
    ) -> Result<CallToolResult, McpError> {
        let tree_config = Self::create_tree_config(args.include_detailed_attributes);

        let tree = self
            .desktop
            .get_window_tree(args.pid, args.title.as_deref(), Some(tree_config))
            .map_err(|e| {
                McpError::resource_not_found(
                    "Failed to get window tree",
                    Some(json!({"reason": e.to_string(), "pid": args.pid, "title": args.title})),
                )
            })?;

        let mut result_json = json!({
            "action": "get_window_tree",
            "status": "success",
            "pid": args.pid,
            "title": args.title,
            "detailed_attributes": args.include_detailed_attributes.unwrap_or(true),
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "recommendation": "Prefer role|name selectors (e.g., 'button|Submit'). Use the element ID (e.g., '#12345') as a fallback if the name is missing or generic."
        });

        // Always include the tree unless explicitly disabled
        if let Ok(tree_val) = serde_json::to_value(tree) {
            if let Some(obj) = result_json.as_object_mut() {
                obj.insert("ui_tree".to_string(), tree_val);
            }
        }

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Get the complete UI tree for the currently focused window. This is a convenient tool that automatically detects which window has focus and returns its UI tree. This is a read-only operation."
    )]
    pub async fn get_focused_window_tree(
        &self,
        Parameters(args): Parameters<crate::utils::GetFocusedWindowTreeArgs>,
    ) -> Result<CallToolResult, McpError> {
        let tree_config = Self::create_tree_config(args.include_detailed_attributes);

        // Get the currently focused element
        let focused_element = self.desktop.focused_element().map_err(|e| {
            McpError::internal_error(
                "Failed to get focused element",
                Some(json!({"reason": e.to_string()})),
            )
        })?;

        // Get the PID and window title from the focused element
        let pid = focused_element.process_id().unwrap_or(0);

        if pid == 0 {
            return Err(McpError::internal_error(
                "Could not get process ID from focused element",
                Some(json!({"element_role": focused_element.role()})),
            ));
        }

        let window_title = focused_element.window_title();
        let app_name = focused_element.application_name();

        // Get the window tree for the focused application
        let tree = self
            .desktop
            .get_window_tree(pid, Some(&window_title), Some(tree_config))
            .map_err(|e| {
                McpError::resource_not_found(
                    "Failed to get window tree for focused window",
                    Some(json!({
                        "reason": e.to_string(),
                        "pid": pid,
                        "window_title": window_title,
                        "app_name": app_name
                    })),
                )
            })?;

        let result_json = json!({
            "action": "get_focused_window_tree",
            "status": "success",
            "focused_window": {
                "pid": pid,
                "window_title": window_title,
                "application_name": app_name,
            },
            "detailed_attributes": args.include_detailed_attributes.unwrap_or(true),
            "ui_tree": tree,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "recommendation": "Prefer role|name selectors (e.g., 'button|Submit'). Use the element ID (e.g., '#12345') as a fallback if the name is missing or generic."
        });

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Get all applications currently running and their state. This is a read-only operation."
    )]
    pub async fn get_applications(
        &self,
        Parameters(args): Parameters<GetApplicationsArgs>,
    ) -> Result<CallToolResult, McpError> {
        let apps = self.desktop.applications().map_err(|e| {
            McpError::resource_not_found(
                "Failed to get applications",
                Some(json!({"reason": e.to_string()})),
            )
        })?;

        let include_tree = Self::get_include_tree_default(args.include_tree);
        let tree_config = if include_tree {
            Some(Self::create_tree_config(args.include_detailed_attributes))
        } else {
            None
        };

        let app_info_futures: Vec<_> = apps
            .iter()
            .map(|app| {
                let desktop = self.desktop.clone();
                let app_name = app.name().unwrap_or_default();
                let app_id = app.id().unwrap_or_default();
                let app_role = app.role();
                let app_pid = app.process_id().unwrap_or(0);
                let is_focused = app.is_focused().unwrap_or(false);
                let config = tree_config.clone();

                let suggested_selector = if !app_name.is_empty() {
                    format!("{}|{}", &app_role, &app_name)
                } else {
                    format!("#{app_id}")
                };

                tokio::spawn(async move {
                    let tree = if include_tree && app_pid > 0 {
                        desktop.get_window_tree(app_pid, None, config).ok()
                    } else {
                        None
                    };

                    json!({
                        "name": app_name,
                        "id": app_id,
                        "role": app_role,
                        "pid": app_pid,
                        "is_focused": is_focused,
                        "suggested_selector": suggested_selector,
                        "ui_tree": tree
                    })
                })
            })
            .collect();

        let app_info_results = futures::future::join_all(app_info_futures).await;
        let mut applications = Vec::new();

        for result in app_info_results {
            match result {
                Ok(app_info) => applications.push(app_info),
                Err(e) => {
                    warn!("Failed to get app info: {}", e);
                }
            }
        }

        let result_json = json!({
            "action": "get_applications",
            "status": "success",
            "include_tree": include_tree,
            "detailed_attributes": args.include_detailed_attributes.unwrap_or(true),
            "applications": applications,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

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

    /// Ensures element is visible and optionally applies highlighting before action
    fn ensure_visible_and_apply_highlight(
        element: &UIElement,
        highlight_config: Option<&ActionHighlightConfig>,
        action_name: &str,
    ) {
        // Always ensure element is in view first (for all actions, not just when highlighting)
        if let Err(e) = Self::ensure_element_in_view(element) {
            tracing::warn!("Failed to ensure element is in view for {action_name} action: {e}");
        }

        // Then apply highlighting if configured
        if let Some(config) = highlight_config {
            if config.enabled {
                let duration = config.duration_ms.map(std::time::Duration::from_millis);
                let color = config.color;
                let text = config.text.as_deref();

                #[cfg(target_os = "windows")]
                let text_position = config.text_position.clone().map(|pos| pos.into());
                #[cfg(not(target_os = "windows"))]
                let text_position = None;

                #[cfg(target_os = "windows")]
                let font_style = config.font_style.clone().map(|style| style.into());
                #[cfg(not(target_os = "windows"))]
                let font_style = None;

                tracing::info!(
                    "HIGHLIGHT_BEFORE_{} duration={:?}",
                    action_name.to_uppercase(),
                    duration
                );
                if let Ok(_highlight_handle) =
                    element.highlight(color, duration, text, text_position, font_style)
                {
                    // Highlight applied successfully - runs concurrently with action
                } else {
                    tracing::warn!("Failed to apply highlighting before {action_name} action");
                }
            }
        }
    }

    #[tool(
        description = "Types text into a UI element with smart clipboard optimization and verification. Much faster than press key. This action requires the application to be focused and may change the UI."
    )]
    async fn type_into_element(
        &self,
        Parameters(args): Parameters<TypeIntoElementArgs>,
    ) -> Result<CallToolResult, McpError> {
        tracing::info!(
            "[type_into_element] Called with selector: '{}'",
            args.selector
        );

        let text_to_type = args.text_to_type.clone();
        let should_clear = args.clear_before_typing.unwrap_or(true);

        let action = {
            let highlight_config = args.highlight_before_action.clone();
            move |element: UIElement| {
                let text_to_type = text_to_type.clone();
                let highlight_config = highlight_config.clone();
                async move {
                    // Apply highlighting before action if configured
                    Self::ensure_visible_and_apply_highlight(
                        &element,
                        highlight_config.as_ref(),
                        "type",
                    );

                    // Execute the typing action with state tracking
                    if should_clear {
                        if let Err(clear_error) = element.set_value("") {
                            warn!(
                                "Warning: Failed to clear element before typing: {}",
                                clear_error
                            );
                        }
                    }
                    element.type_text_with_state(&text_to_type, true)
                }
            }
        };

        let ((result, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector,
                args.alternative_selectors.as_deref(),
                args.fallback_selectors.as_deref(),
                args.timeout_ms,
                args.retries,
                action,
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector,
                    args.alternative_selectors.as_deref(),
                    args.fallback_selectors.as_deref(),
                    e,
                )),
            }?;

        let mut result_json = json!({
            "action": "type_into_element",
            "status": "success",
            "text_typed": args.text_to_type,
            "cleared_before_typing": args.clear_before_typing.unwrap_or(true),
            "action_result": {
                "action": result.action,
                "details": result.details,
                "data": result.data,
            },
            "element": build_element_info(&element),
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector, args.alternative_selectors.as_deref(), args.fallback_selectors.as_deref()),
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        // Verification if requested
        if args.verify_action.unwrap_or(true) {
            // Create a new locator for verification using the successful selector
            let verification_locator = self
                .desktop
                .locator(Selector::from(successful_selector.as_str()));
            if let Ok(updated_element) = verification_locator
                .wait(Some(std::time::Duration::from_millis(500)))
                .await
            {
                let current_text = updated_element.text(0).unwrap_or_default();
                let should_clear = args.clear_before_typing.unwrap_or(true);
                let text_matches = if should_clear {
                    current_text == args.text_to_type
                } else {
                    current_text.contains(&args.text_to_type)
                };

                if !text_matches {
                    return Err(McpError::internal_error(
                        "Text verification failed after typing.",
                        Some(json!({
                            "expected_text": args.text_to_type,
                            "actual_text": current_text,
                            "element": build_element_info(&updated_element),
                            "selector_used": successful_selector,
                        })),
                    ));
                }

                let verification = json!({
                    "text_value_after": current_text,
                    "text_check_passed": text_matches,
                    "element_focused": updated_element.is_focused().unwrap_or(false),
                    "element_enabled": updated_element.is_enabled().unwrap_or(false),
                    "verification_timestamp": chrono::Utc::now().to_rfc3339()
                });
                if let Some(obj) = result_json.as_object_mut() {
                    obj.insert("verification".to_string(), verification);
                }
            } else {
                return Err(McpError::internal_error(
                    "Failed to find element for verification after typing.",
                    Some(json!({
                        "selector_used": successful_selector,
                    })),
                ));
            }
        }

        // Always attach tree for better context, or if an override is provided
        maybe_attach_tree(
            &self.desktop,
            Self::get_include_tree_default(args.include_tree),
            args.include_detailed_attributes,
            Some(element.process_id().unwrap_or(0)),
            &mut result_json,
        );

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Clicks a UI element. This action requires the application to be focused and may change the UI."
    )]
    pub async fn click_element(
        &self,
        Parameters(args): Parameters<ClickElementArgs>,
    ) -> Result<CallToolResult, McpError> {
        tracing::info!("[click_element] Called with selector: '{}'", args.selector);

        if let Some(ref pos) = args.click_position {
            tracing::info!(
                "[click_element] Click position: {}%, {}%",
                pos.x_percentage,
                pos.y_percentage
            );
        }

        let action = {
            let highlight_config = args.highlight_before_action.clone();
            let click_position = args.click_position.clone();
            move |element: UIElement| {
                let highlight_config = highlight_config.clone();
                let click_position = click_position.clone();
                async move {
                    // Ensure element is visible and apply highlighting if configured
                    Self::ensure_visible_and_apply_highlight(
                        &element,
                        highlight_config.as_ref(),
                        "click",
                    );

                    // Click at specific position if provided
                    if let Some(pos) = click_position {
                        // Get element bounds to calculate absolute position
                        match element.bounds() {
                            Ok(bounds) => {
                                // Calculate absolute coordinates from percentages
                                let x = bounds.0 + (bounds.2 * pos.x_percentage as f64 / 100.0);
                                let y = bounds.1 + (bounds.3 * pos.y_percentage as f64 / 100.0);

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
                                        pos.x_percentage, pos.y_percentage
                                    ),
                                })
                            }
                            Err(e) => {
                                tracing::warn!("[click_element] Failed to get bounds for position click: {}. Falling back to center click.", e);
                                element.click()
                            }
                        }
                    } else {
                        // Default center click
                        element.click()
                    }
                }
            }
        };

        let ((click_result, element), successful_selector) =
            match crate::utils::find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector,
                args.alternative_selectors.as_deref(),
                args.fallback_selectors.as_deref(),
                args.timeout_ms,
                args.retries,
                action,
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector,
                    args.alternative_selectors.as_deref(),
                    args.fallback_selectors.as_deref(),
                    e,
                )),
            }?;

        // Optionally include troubleshooting recommendations when evidence suggests the click may have missed the intended target
        let details_str = &click_result.details;
        // Consider the outcome uncertain whenever immediate post-change signals are absent,
        // regardless of the click path. This makes guidance available for subtle misses too.
        let looks_uncertain = details_str.contains("window_title_changed=false")
            && details_str.contains("bounds_changed=false");

        let mut result_json = json!({
            "action": "click",
            "status": "success",
            "selector_used": successful_selector,
            "click_result": {
                "method": click_result.method,
                "coordinates": click_result.coordinates,
                "details": click_result.details,
            },
            "element": {
                "role": element.role(),
                "name": element.name(),
                "bounds": element.bounds().ok(),
                "window_title": element.window_title()
            },
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        if looks_uncertain {
            if let Some(obj) = result_json.as_object_mut() {
                obj.insert(
                    "recommendations".to_string(),
                    json!([
                        "Read the evidence first: if click_result.method is CenterFallback or ClickablePoint and window_title_changed/bounds_changed are false, treat it as uncertain/likely missed target.",
                        "Prefer action semantics: try invoke_element on the same selector, or validate_element → focus target → press_key '{Enter}'.",
                        "Narrow the selector to the true clickable child (the text anchor), not the enclosing group; keep role:hyperlink and tighten name:, or use the element’s numeric #id.",
                        "If the site opens in a new tab, wait for tab/title/address-bar change; otherwise treat as failed and refine selector.",
                        "Always pair the click with a postcondition: address bar/title/tab change or a destination-unique element; if it doesn’t happen, re-run with the steps above.",
                        "Selector tip: prefer role:hyperlink with a unique substring (often the destination domain) or the numeric #id, and add |nth:0 if needed."
                    ]),
                );
            }
        }

        maybe_attach_tree(
            &self.desktop,
            Self::get_include_tree_default(args.include_tree),
            args.include_detailed_attributes,
            Some(element.process_id().unwrap_or(0)),
            &mut result_json,
        );

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Sends a key press to a UI element. Use curly brace format: '{Ctrl}c', '{Alt}{F4}', '{Enter}', '{PageDown}', etc. This action requires the application to be focused and may change the UI."
    )]
    async fn press_key(
        &self,
        Parameters(args): Parameters<PressKeyArgs>,
    ) -> Result<CallToolResult, McpError> {
        tracing::info!(
            "[press_key] Called with selector: '{}', key: '{}'",
            args.selector,
            args.key
        );

        let key_to_press = args.key.clone();
        let action = {
            let highlight_config = args.highlight_before_action.clone();
            move |element: UIElement| {
                let key_to_press = key_to_press.clone();
                let highlight_config = highlight_config.clone();
                async move {
                    // Ensure element is visible and apply highlighting if configured
                    Self::ensure_visible_and_apply_highlight(
                        &element,
                        highlight_config.as_ref(),
                        "key",
                    );

                    // Execute the key press action with state tracking
                    element.press_key_with_state(&key_to_press)
                }
            }
        };

        let ((result, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector,
                None, // PressKey doesn't have alternative selectors yet
                args.fallback_selectors.as_deref(),
                args.timeout_ms,
                args.retries,
                action,
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector,
                    None,
                    args.fallback_selectors.as_deref(),
                    e,
                )),
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
            "selectors_tried": get_selectors_tried_all(&args.selector, None, args.fallback_selectors.as_deref()),
            "timestamp": chrono::Utc::now().to_rfc3339()
        });
        maybe_attach_tree(
            &self.desktop,
            Self::get_include_tree_default(args.include_tree),
            args.include_detailed_attributes,
            element.process_id().ok(),
            &mut result_json,
        );

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Sends a key press to the currently focused element (no selector required). Use curly brace format: '{Ctrl}c', '{Alt}{F4}', '{Enter}', '{PageDown}', etc. This action requires the application to be focused and may change the UI."
    )]
    async fn press_key_global(
        &self,
        Parameters(args): Parameters<GlobalKeyArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Identify focused element
        let element = self.desktop.focused_element().map_err(|e| {
            McpError::internal_error(
                "Failed to get focused element",
                Some(json!({"reason": e.to_string()})),
            )
        })?;

        // Gather metadata for debugging / result payload
        let element_info = build_element_info(&element);

        // Perform the key press
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
            "key_pressed": args.key,
            "element": element_info,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });
        maybe_attach_tree(
            &self.desktop,
            Self::get_include_tree_default(args.include_tree),
            args.include_detailed_attributes,
            element.process_id().ok(),
            &mut result_json,
        );

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Executes a shell command (GitHub Actions-style) OR runs inline code via an engine. Use 'run' for shell commands. Or set 'engine' to 'node'/'bun'/'javascript'/'typescript'/'ts' for JS/TS with terminator.js, or 'python' for Python with terminator.py and provide the code in 'run' or 'script_file'. TypeScript is supported with automatic transpilation. When using engine mode, you can pass data to subsequent workflow steps by returning { set_env: { key: value } } or using console.log('::set-env name=key::value'). Access variables in later steps with {{env.key}} substitution. NEW: Use 'script_file' to load scripts from files, 'env' to inject environment variables as 'var env = {...}' (JS/TS) or 'env = {...}' (Python)."
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
        // Engine-based execution path (provides SDK bindings)
        if let Some(engine_value) = args.engine.as_ref() {
            let engine = engine_value.to_ascii_lowercase();

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

                        // Priority 3: Use as-is if still not found
                        if resolved_path.is_none() {
                            let candidate = script_path.to_path_buf();
                            resolution_attempts.push(format!("as-is: {}", candidate.display()));
                            tracing::info!(
                                "[SCRIPTS_BASE_PATH] Using path as-is (not found in base paths): {}",
                                script_file
                            );
                            resolved_path = Some(candidate);
                        }
                    } else {
                        // Absolute path - use as-is
                        tracing::info!("[run_command] Using absolute path: {}", script_file);
                        resolved_path = Some(script_path.to_path_buf());
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
                            if let Ok(value_json) = serde_json::to_string(&value) {
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
                            if let Ok(value_json) = serde_json::to_string(&value) {
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
                let execution_result = scripting_engine::execute_javascript_with_nodejs(
                    final_script,
                    cancellation_token,
                )
                .await?;

                // Extract logs and actual result
                let logs = execution_result.get("logs").cloned();
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

                // Build response with logs
                let mut response = json!({
                    "action": "run_command",
                    "mode": "engine",
                    "engine": engine,
                    "status": "success",
                    "result": actual_result
                });

                if let Some(logs) = logs {
                    response["logs"] = logs;
                }

                return Ok(CallToolResult::success(vec![Content::json(response)?]));
            } else if is_ts {
                let execution_result = scripting_engine::execute_typescript_with_nodejs(
                    final_script,
                    cancellation_token,
                )
                .await?;

                // Extract logs and actual result (same as JS)
                let logs = execution_result.get("logs").cloned();
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

                // Build response with logs
                let mut response = json!({
                    "action": "run_command",
                    "mode": "engine",
                    "engine": engine,
                    "status": "success",
                    "result": actual_result
                });

                if let Some(logs) = logs {
                    response["logs"] = logs;
                }

                return Ok(CallToolResult::success(vec![Content::json(response)?]));
            } else if is_py {
                let execution_result =
                    scripting_engine::execute_python_with_bindings(final_script).await?;

                // Check if the Python result indicates a failure (same as JavaScript)
                if let Some(obj) = execution_result.as_object() {
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
                                    Some(execution_result),
                                ));
                            }
                        }
                    }
                }

                // For now, Python doesn't capture logs yet, but we can add it later
                // Just pass through the result as before
                return Ok(CallToolResult::success(vec![Content::json(json!({
                    "action": "run_command",
                    "mode": "engine",
                    "engine": engine,
                    "status": "success",
                    "result": execution_result
                }))?]));
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

                    // Priority 3: Use as-is if still not found
                    if resolved_path.is_none() {
                        let candidate = script_path.to_path_buf();
                        resolution_attempts.push(format!("as-is: {}", candidate.display()));
                        tracing::info!(
                            "[run_command shell] Using path as-is (not found in base paths): {}",
                            script_file
                        );
                        resolved_path = Some(candidate);
                    }
                } else {
                    // Absolute path - use as-is
                    tracing::info!("[run_command shell] Using absolute path: {}", script_file);
                    resolved_path = Some(script_path.to_path_buf());
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

        let output = self
            .desktop
            .run_command(windows_cmd.as_deref(), unix_cmd.as_deref())
            .await
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

        Ok(CallToolResult::success(vec![Content::json(json!({
            "exit_status": output.exit_status,
            "stdout": output.stdout,
            "stderr": output.stderr,
            "command": run_str,
            "shell": args.shell.unwrap_or_else(|| {
                if cfg!(target_os = "windows") { "powershell" } else { "bash" }.to_string()
            }),
            "working_directory": args.working_directory
        }))?]))
    }

    #[tool(
        description = "Activates the window containing the specified element, bringing it to the foreground."
    )]
    pub async fn activate_element(
        &self,
        Parameters(args): Parameters<ActivateElementArgs>,
    ) -> Result<CallToolResult, McpError> {
        let ((_result, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector,
                None, // ActivateElement doesn't have alternative selectors
                args.fallback_selectors.as_deref(),
                args.timeout_ms,
                args.retries,
                |element| async move { element.activate_window() },
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector,
                    None,
                    args.fallback_selectors.as_deref(),
                    e,
                )),
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
            "selectors_tried": get_selectors_tried_all(&args.selector, None, args.fallback_selectors.as_deref()),
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "verification": verification,
            "recommendation": recommendation
        });

        // Always attach UI tree for activated elements to help with next actions
        maybe_attach_tree(
            &self.desktop,
            Self::get_include_tree_default(args.include_tree),
            args.include_detailed_attributes,
            Some(element.process_id().unwrap_or(0)),
            &mut result_json,
        );

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Delays execution for a specified number of milliseconds. Useful for waiting between actions to ensure UI stability."
    )]
    async fn delay(
        &self,
        Parameters(args): Parameters<DelayArgs>,
    ) -> Result<CallToolResult, McpError> {
        let start_time = chrono::Utc::now();

        // Use tokio's sleep for async delay
        tokio::time::sleep(std::time::Duration::from_millis(args.delay_ms)).await;

        let end_time = chrono::Utc::now();
        let actual_delay_ms = (end_time - start_time).num_milliseconds();

        Ok(CallToolResult::success(vec![Content::json(json!({
            "action": "delay",
            "status": "success",
            "requested_delay_ms": args.delay_ms,
            "actual_delay_ms": actual_delay_ms,
            "timestamp": end_time.to_rfc3339()
        }))?]))
    }

    #[tool(
        description = "Performs a mouse drag operation from start to end coordinates. This action requires the application to be focused and may change the UI."
    )]
    async fn mouse_drag(
        &self,
        Parameters(args): Parameters<MouseDragArgs>,
    ) -> Result<CallToolResult, McpError> {
        let action = |element: UIElement| async move {
            element.mouse_drag(args.start_x, args.start_y, args.end_x, args.end_y)
        };

        let ((_result, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector,
                args.alternative_selectors.as_deref(),
                args.fallback_selectors.as_deref(),
                args.timeout_ms,
                args.retries,
                action,
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector,
                    args.alternative_selectors.as_deref(),
                    args.fallback_selectors.as_deref(),
                    e,
                )),
            }?;

        let element_info = build_element_info(&element);

        let mut result_json = json!({
            "action": "mouse_drag",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector, args.alternative_selectors.as_deref(), args.fallback_selectors.as_deref()),
            "start": (args.start_x, args.start_y),
            "end": (args.end_x, args.end_y),
            "timestamp": chrono::Utc::now().to_rfc3339()
        });
        maybe_attach_tree(
            &self.desktop,
            Self::get_include_tree_default(args.include_tree),
            args.include_detailed_attributes,
            element.process_id().ok(),
            &mut result_json,
        );

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Validates that an element exists and provides detailed information about it. This is a read-only operation."
    )]
    pub async fn validate_element(
        &self,
        Parameters(args): Parameters<ValidateElementArgs>,
    ) -> Result<CallToolResult, McpError> {
        // For validation, the "action" is just succeeding.
        let action = |element: UIElement| async move { Ok(element) };

        match find_and_execute_with_retry_with_fallback(
            &self.desktop,
            &args.selector,
            args.alternative_selectors.as_deref(),
            args.fallback_selectors.as_deref(),
            args.timeout_ms,
            args.retries,
            action,
        )
        .await
        {
            Ok(((element, _), successful_selector)) => {
                let mut element_info = build_element_info(&element);
                if let Some(obj) = element_info.as_object_mut() {
                    obj.insert("exists".to_string(), json!(true));
                }

                let mut result_json = json!({
                    "action": "validate_element",
                    "status": "success",
                    "element": element_info,
                    "selector_used": successful_selector,
                    "selectors_tried": get_selectors_tried_all(&args.selector, args.alternative_selectors.as_deref(), args.fallback_selectors.as_deref()),
                    "timestamp": chrono::Utc::now().to_rfc3339()
                });
                maybe_attach_tree(
                    &self.desktop,
                    Self::get_include_tree_default(args.include_tree),
                    args.include_detailed_attributes,
                    element.process_id().ok(),
                    &mut result_json,
                );

                Ok(CallToolResult::success(vec![Content::json(result_json)?]))
            }
            Err(e) => {
                let selectors_tried = get_selectors_tried_all(
                    &args.selector,
                    args.alternative_selectors.as_deref(),
                    args.fallback_selectors.as_deref(),
                );
                let reason_payload = json!({
                    "error_type": "ElementNotFound",
                    "message": format!("The specified element could not be found after trying all selectors. Original error: {}", e),
                    "selectors_tried": selectors_tried,
                    "suggestions": [
                        "Call `get_window_tree` again to get a fresh view of the UI; it might have changed.",
                        "Verify the element's 'name' and 'role' in the new UI tree. The 'name' attribute might be empty or different from the visible text.",
                        "If the element has no 'name', use its numeric ID selector (e.g., '#12345')."
                    ]
                });

                // This is not a tool error, but a validation failure, so we return success with the failure info.
                Ok(CallToolResult::success(vec![Content::json(json!({
                    "action": "validate_element",
                    "status": "failed",
                    "exists": false,
                    "reason": reason_payload,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }))?]))
            }
        }
    }

    #[tool(description = "Highlights an element with a colored border for visual confirmation.")]
    async fn highlight_element(
        &self,
        Parameters(args): Parameters<HighlightElementArgs>,
    ) -> Result<CallToolResult, McpError> {
        let duration = args.duration_ms.map(std::time::Duration::from_millis);
        let color = args.color;

        let text = args.text.as_deref();

        #[cfg(target_os = "windows")]
        let text_position = args.text_position.clone().map(|pos| pos.into());
        #[cfg(not(target_os = "windows"))]
        let text_position = None;

        #[cfg(target_os = "windows")]
        let font_style = args.font_style.clone().map(|style| style.into());
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
        let effective_timeout_ms = args.timeout_ms.or(Some(1000));

        let ((handle, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector,
                args.alternative_selectors.as_deref(),
                args.fallback_selectors.as_deref(),
                effective_timeout_ms,
                args.retries,
                action,
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector,
                    args.alternative_selectors.as_deref(),
                    args.fallback_selectors.as_deref(),
                    e,
                )),
            }?;

        // Register handle and schedule cleanup
        {
            let mut list = self.active_highlights.lock().await;
            list.push(handle);
        }
        let active_highlights_clone = self.active_highlights.clone();
        let expire_after = args.duration_ms.unwrap_or(1000);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(expire_after)).await;
            let mut list = active_highlights_clone.lock().await;
            let _ = list.pop();
        });

        // Build minimal response by default; gate heavy element info behind flag
        let mut result_json = json!({
            "action": "highlight_element",
            "status": "success",
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector, args.alternative_selectors.as_deref(), args.fallback_selectors.as_deref()),
            "color": args.color.unwrap_or(0x0000FF),
            "duration_ms": args.duration_ms.unwrap_or(1000),
            "visibility": { "requested_ms": args.duration_ms.unwrap_or(1000) },
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        if args.include_element_info.unwrap_or(false) {
            let element_info = build_element_info(&element);
            result_json["element"] = element_info;
        }
        maybe_attach_tree(
            &self.desktop,
            Self::get_include_tree_default(args.include_tree),
            args.include_detailed_attributes,
            element.process_id().ok(),
            &mut result_json,
        );

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Waits for an element to meet a specific condition (visible, enabled, focused, exists)."
    )]
    async fn wait_for_element(
        &self,
        Parameters(args): Parameters<WaitForElementArgs>,
    ) -> Result<CallToolResult, McpError> {
        info!(
            "[wait_for_element] Called with selector: '{}', condition: '{}', timeout_ms: {:?}, include_tree: {:?}",
            args.selector, args.condition, args.timeout_ms, args.include_tree
        );

        let locator = self.desktop.locator(Selector::from(args.selector.as_str()));
        let timeout = get_timeout(args.timeout_ms);
        let condition_lower = args.condition.to_lowercase();

        // For the "exists" condition, we can use the standard wait
        if condition_lower == "exists" {
            info!(
                "[wait_for_element] Waiting for element to exist: selector='{}', timeout={:?}",
                args.selector, timeout
            );
            match locator.wait(timeout).await {
                Ok(element) => {
                    info!(
                        "[wait_for_element] Element found for selector='{}' within timeout.",
                        args.selector
                    );
                    let mut result_json = json!({
                        "action": "wait_for_element",
                        "status": "success",
                        "condition": args.condition,
                        "condition_met": true,
                        "selector": args.selector,
                        "timeout_ms": args.timeout_ms.unwrap_or(5000),
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    });

                    maybe_attach_tree(
                        &self.desktop,
                        Self::get_include_tree_default(args.include_tree),
                        args.include_detailed_attributes,
                        element.process_id().ok(),
                        &mut result_json,
                    );

                    return Ok(CallToolResult::success(vec![Content::json(result_json)?]));
                }
                Err(e) => {
                    let error_msg = format!("Element not found within timeout: {e}");
                    info!(
                        "[wait_for_element] Element NOT found for selector='{}' within timeout. Error: {}",
                        args.selector, e
                    );
                    return Err(McpError::internal_error(
                        error_msg,
                        Some(json!({
                            "selector": args.selector,
                            "condition": args.condition,
                            "timeout_ms": args.timeout_ms.unwrap_or(5000),
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
            args.condition, args.selector, timeout_duration
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
                    args.selector, args.condition, start_time.elapsed().as_millis()
                );
                return Err(McpError::internal_error(
                    timeout_msg,
                    Some(json!({
                        "selector": args.selector,
                        "condition": args.condition,
                        "timeout_ms": args.timeout_ms.unwrap_or(5000),
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
                        args.selector, args.condition
                    );
                    // Element exists, now check the specific condition
                    let condition_met = match condition_lower.as_str() {
                        "visible" => {
                            let v = element.is_visible().unwrap_or(false);
                            info!(
                                "[wait_for_element] is_visible() for selector='{}': {}",
                                args.selector, v
                            );
                            v
                        }
                        "enabled" => {
                            let v = element.is_enabled().unwrap_or(false);
                            info!(
                                "[wait_for_element] is_enabled() for selector='{}': {}",
                                args.selector, v
                            );
                            v
                        }
                        "focused" => {
                            let v = element.is_focused().unwrap_or(false);
                            info!(
                                "[wait_for_element] is_focused() for selector='{}': {}",
                                args.selector, v
                            );
                            v
                        }
                        _ => {
                            info!(
                                "[wait_for_element] Invalid condition provided: '{}'",
                                args.condition
                            );
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
                            args.selector,
                            start_time.elapsed().as_millis()
                        );
                        // Condition is met, return success
                        let mut result_json = json!({
                            "action": "wait_for_element",
                            "status": "success",
                            "condition": args.condition,
                            "condition_met": true,
                            "selector": args.selector,
                            "timeout_ms": args.timeout_ms.unwrap_or(5000),
                            "elapsed_ms": start_time.elapsed().as_millis(),
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        });

                        maybe_attach_tree(
                            &self.desktop,
                            Self::get_include_tree_default(args.include_tree),
                            args.include_detailed_attributes,
                            element.process_id().ok(),
                            &mut result_json,
                        );

                        return Ok(CallToolResult::success(vec![Content::json(result_json)?]));
                    } else {
                        info!(
                            "[wait_for_element] Condition '{}' NOT met for selector='{}', continuing to poll...",
                            args.condition, args.selector
                        );
                    }
                    // Condition not met yet, continue polling
                }
                Err(_) => {
                    info!(
                        "[wait_for_element] Element not found for selector='{}', will retry...",
                        args.selector
                    );
                    // Element doesn't exist yet, continue polling
                }
            }

            // Wait a bit before the next poll
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }

    #[tool(
        description = "Opens a URL in the specified browser (uses SDK's built-in browser automation)."
    )]
    async fn navigate_browser(
        &self,
        Parameters(args): Parameters<NavigateBrowserArgs>,
    ) -> Result<CallToolResult, McpError> {
        let browser = args.browser.clone().map(Browser::Custom);
        let ui_element = self.desktop.open_url(&args.url, browser).map_err(|e| {
            McpError::internal_error(
                "Failed to open URL",
                Some(json!({"reason": e.to_string(), "url": args.url, "browser": args.browser})),
            )
        })?;

        let element_info = build_element_info(&ui_element);

        let mut result_json = json!({
            "action": "navigate_browser",
            "status": "success",
            "url": args.url,
            "browser": args.browser,
            "element": element_info,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        maybe_attach_tree(
            &self.desktop,
            Self::get_include_tree_default(args.include_tree),
            args.include_detailed_attributes,
            ui_element.process_id().ok(),
            &mut result_json,
        );

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(description = "Opens an application by name (uses SDK's built-in app launcher).")]
    pub async fn open_application(
        &self,
        Parameters(args): Parameters<OpenApplicationArgs>,
    ) -> Result<CallToolResult, McpError> {
        let ui_element = self.desktop.open_application(&args.app_name).map_err(|e| {
            McpError::internal_error(
                "Failed to open application",
                Some(json!({"reason": e.to_string(), "app_name": args.app_name})),
            )
        })?;

        let process_id = ui_element.process_id().unwrap_or(0);
        let window_title = ui_element.window_title();

        let element_info = build_element_info(&ui_element);

        let mut result_json = json!({
            "action": "open_application",
            "status": "success",
            "app_name": args.app_name,
            "application": element_info,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "recommendation": "Application opened successfully. Use get_window_tree with the PID to get the full UI structure for reliable element targeting."
        });

        // Always attach the full UI tree for newly opened applications
        if process_id > 0 {
            if let Ok(tree) =
                self.desktop
                    .get_window_tree(process_id, Some(window_title.as_str()), None)
            {
                if let Ok(tree_val) = serde_json::to_value(tree) {
                    if let Some(obj) = result_json.as_object_mut() {
                        obj.insert("ui_tree".to_string(), tree_val);
                    }
                }
            }
        }

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Closes a UI element (window, application, dialog, etc.) if it's closable."
    )]
    pub async fn close_element(
        &self,
        Parameters(args): Parameters<CloseElementArgs>,
    ) -> Result<CallToolResult, McpError> {
        let ((_result, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector,
                args.alternative_selectors.as_deref(),
                args.fallback_selectors.as_deref(),
                args.timeout_ms,
                args.retries,
                |element| async move { element.close() },
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector,
                    args.alternative_selectors.as_deref(),
                    args.fallback_selectors.as_deref(),
                    e,
                )),
            }?;

        let element_info = build_element_info(&element);

        Ok(CallToolResult::success(vec![Content::json(json!({
            "action": "close_element",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector, args.alternative_selectors.as_deref(), args.fallback_selectors.as_deref()),
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))?]))
    }

    #[tool(description = "Scrolls a UI element in the specified direction by the given amount.")]
    async fn scroll_element(
        &self,
        Parameters(args): Parameters<ScrollElementArgs>,
    ) -> Result<CallToolResult, McpError> {
        tracing::info!(
            "[scroll_element] Called with selector: '{}', direction: '{}', amount: {}",
            args.selector,
            args.direction,
            args.amount
        );

        let direction = args.direction.clone();
        let amount = args.amount;
        let action = {
            let highlight_config = args.highlight_before_action.clone();
            move |element: UIElement| {
                let direction = direction.clone();
                let highlight_config = highlight_config.clone();
                async move {
                    // Ensure element is visible and apply highlighting if configured
                    Self::ensure_visible_and_apply_highlight(
                        &element,
                        highlight_config.as_ref(),
                        "scroll",
                    );

                    // Execute the scroll action with state tracking
                    element.scroll_with_state(&direction, amount)
                }
            }
        };

        let ((result, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector,
                args.alternative_selectors.as_deref(),
                args.fallback_selectors.as_deref(),
                args.timeout_ms,
                args.retries,
                action,
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector,
                    args.alternative_selectors.as_deref(),
                    args.fallback_selectors.as_deref(),
                    e,
                )),
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
            "selectors_tried": get_selectors_tried_all(&args.selector, args.alternative_selectors.as_deref(), args.fallback_selectors.as_deref()),
            "direction": args.direction,
            "amount": args.amount,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });
        maybe_attach_tree(
            &self.desktop,
            Self::get_include_tree_default(args.include_tree),
            args.include_detailed_attributes,
            element.process_id().ok(),
            &mut result_json,
        );

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(description = "Selects an option in a dropdown or combobox by its visible text.")]
    async fn select_option(
        &self,
        Parameters(args): Parameters<SelectOptionArgs>,
    ) -> Result<CallToolResult, McpError> {
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

        let ((result, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector,
                args.alternative_selectors.as_deref(),
                args.fallback_selectors.as_deref(),
                args.timeout_ms,
                args.retries,
                action,
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector,
                    args.alternative_selectors.as_deref(),
                    args.fallback_selectors.as_deref(),
                    e,
                )),
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
            "selectors_tried": get_selectors_tried_all(&args.selector, args.alternative_selectors.as_deref(), args.fallback_selectors.as_deref()),
            "option_selected": args.option_name,
        });
        maybe_attach_tree(
            &self.desktop,
            Self::get_include_tree_default(args.include_tree),
            args.include_detailed_attributes,
            element.process_id().ok(),
            &mut result_json,
        );

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Lists all available option strings from a dropdown, list box, or similar control. This is a read-only operation."
    )]
    async fn list_options(
        &self,
        Parameters(args): Parameters<LocatorArgs>,
    ) -> Result<CallToolResult, McpError> {
        let ((options, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector,
                args.alternative_selectors.as_deref(),
                args.fallback_selectors.as_deref(),
                args.timeout_ms,
                args.retries,
                |element| async move { element.list_options() },
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector,
                    args.alternative_selectors.as_deref(),
                    args.fallback_selectors.as_deref(),
                    e,
                )),
            }?;

        let element_info = build_element_info(&element);

        let mut result_json = json!({
            "action": "list_options",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector, args.alternative_selectors.as_deref(), args.fallback_selectors.as_deref()),
            "options": options,
            "count": options.len(),
        });
        maybe_attach_tree(
            &self.desktop,
            Self::get_include_tree_default(args.include_tree),
            args.include_detailed_attributes,
            element.process_id().ok(),
            &mut result_json,
        );
        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Sets the state of a toggleable control (e.g., checkbox, switch). This action requires the application to be focused and may change the UI."
    )]
    async fn set_toggled(
        &self,
        Parameters(args): Parameters<SetToggledArgs>,
    ) -> Result<CallToolResult, McpError> {
        let state = args.state;
        let action = move |element: UIElement| async move {
            // Ensure element is visible before interaction
            if let Err(e) = Self::ensure_element_in_view(&element) {
                tracing::warn!("Failed to ensure element is in view for set_toggled: {e}");
            }
            element.set_toggled_with_state(state)
        };

        let ((result, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector,
                None, // SetToggled doesn't have alternative selectors
                args.fallback_selectors.as_deref(),
                args.timeout_ms,
                args.retries,
                action,
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector,
                    None,
                    args.fallback_selectors.as_deref(),
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
            "selectors_tried": get_selectors_tried_all(&args.selector, None, args.fallback_selectors.as_deref()),
            "state_set_to": args.state,
        });
        maybe_attach_tree(
            &self.desktop,
            Self::get_include_tree_default(args.include_tree),
            args.include_detailed_attributes,
            Some(element.process_id().unwrap_or(0)),
            &mut result_json,
        );
        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Sets the value of a range-based control like a slider. This action requires the application to be focused and may change the UI."
    )]
    async fn set_range_value(
        &self,
        Parameters(args): Parameters<SetRangeValueArgs>,
    ) -> Result<CallToolResult, McpError> {
        let value = args.value;
        let action = move |element: UIElement| async move {
            // Ensure element is visible before interaction
            if let Err(e) = Self::ensure_element_in_view(&element) {
                tracing::warn!("Failed to ensure element is in view for set_range_value: {e}");
            }
            element.set_range_value(value)
        };

        let ((_result, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector,
                None, // SetRangeValue doesn't have alternative selectors
                args.fallback_selectors.as_deref(),
                args.timeout_ms,
                args.retries,
                action,
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector,
                    None,
                    args.fallback_selectors.as_deref(),
                    e,
                )),
            }?;

        let element_info = build_element_info(&element);

        let mut result_json = json!({
            "action": "set_range_value",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector, None, args.fallback_selectors.as_deref()),
            "value_set_to": args.value,
        });
        maybe_attach_tree(
            &self.desktop,
            Self::get_include_tree_default(args.include_tree),
            args.include_detailed_attributes,
            Some(element.process_id().unwrap_or(0)),
            &mut result_json,
        );
        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Sets the selection state of a selectable item (e.g., in a list or calendar). This action requires the application to be focused and may change the UI."
    )]
    async fn set_selected(
        &self,
        Parameters(args): Parameters<SetSelectedArgs>,
    ) -> Result<CallToolResult, McpError> {
        let state = args.state;
        let action =
            move |element: UIElement| async move { element.set_selected_with_state(state) };

        let ((result, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector,
                None, // SetSelected doesn't have alternative selectors
                args.fallback_selectors.as_deref(),
                args.timeout_ms,
                args.retries,
                action,
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector,
                    None,
                    args.fallback_selectors.as_deref(),
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
            "selectors_tried": get_selectors_tried_all(&args.selector, None, args.fallback_selectors.as_deref()),
            "state_set_to": args.state,
        });
        maybe_attach_tree(
            &self.desktop,
            Self::get_include_tree_default(args.include_tree),
            args.include_detailed_attributes,
            Some(element.process_id().unwrap_or(0)),
            &mut result_json,
        );
        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Checks if a control (like a checkbox or toggle switch) is currently toggled on. This is a read-only operation."
    )]
    async fn is_toggled(
        &self,
        Parameters(args): Parameters<LocatorArgs>,
    ) -> Result<CallToolResult, McpError> {
        let ((is_toggled, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector,
                args.alternative_selectors.as_deref(),
                args.fallback_selectors.as_deref(),
                args.timeout_ms,
                args.retries,
                |element| async move { element.is_toggled() },
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector,
                    args.alternative_selectors.as_deref(),
                    args.fallback_selectors.as_deref(),
                    e,
                )),
            }?;

        let element_info = build_element_info(&element);

        let mut result_json = json!({
            "action": "is_toggled",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector, args.alternative_selectors.as_deref(), args.fallback_selectors.as_deref()),
            "is_toggled": is_toggled,
        });
        maybe_attach_tree(
            &self.desktop,
            Self::get_include_tree_default(args.include_tree),
            args.include_detailed_attributes,
            element.process_id().ok(),
            &mut result_json,
        );
        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Gets the current value from a range-based control like a slider or progress bar. This is a read-only operation."
    )]
    async fn get_range_value(
        &self,
        Parameters(args): Parameters<LocatorArgs>,
    ) -> Result<CallToolResult, McpError> {
        let ((value, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector,
                args.alternative_selectors.as_deref(),
                args.fallback_selectors.as_deref(),
                args.timeout_ms,
                args.retries,
                |element| async move { element.get_range_value() },
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector,
                    args.alternative_selectors.as_deref(),
                    args.fallback_selectors.as_deref(),
                    e,
                )),
            }?;

        let element_info = build_element_info(&element);

        let mut result_json = json!({
            "action": "get_range_value",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector, args.alternative_selectors.as_deref(), args.fallback_selectors.as_deref()),
            "value": value,
        });
        maybe_attach_tree(
            &self.desktop,
            Self::get_include_tree_default(args.include_tree),
            args.include_detailed_attributes,
            element.process_id().ok(),
            &mut result_json,
        );
        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Checks if a selectable item (e.g., in a calendar, list, or tab) is currently selected. This is a read-only operation."
    )]
    async fn is_selected(
        &self,
        Parameters(args): Parameters<LocatorArgs>,
    ) -> Result<CallToolResult, McpError> {
        let ((is_selected, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector,
                args.alternative_selectors.as_deref(),
                args.fallback_selectors.as_deref(),
                args.timeout_ms,
                args.retries,
                |element| async move { element.is_selected() },
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector,
                    args.alternative_selectors.as_deref(),
                    args.fallback_selectors.as_deref(),
                    e,
                )),
            }?;

        let element_info = build_element_info(&element);

        let mut result_json = json!({
            "action": "is_selected",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector, args.alternative_selectors.as_deref(), args.fallback_selectors.as_deref()),
            "is_selected": is_selected,
        });
        maybe_attach_tree(
            &self.desktop,
            Self::get_include_tree_default(args.include_tree),
            args.include_detailed_attributes,
            element.process_id().ok(),
            &mut result_json,
        );
        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(description = "Captures a screenshot of a specific UI element.")]
    async fn capture_element_screenshot(
        &self,
        Parameters(args): Parameters<ValidateElementArgs>,
    ) -> Result<CallToolResult, McpError> {
        let ((screenshot_result, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector,
                args.alternative_selectors.as_deref(),
                args.fallback_selectors.as_deref(),
                args.timeout_ms,
                args.retries,
                |element| async move { element.capture() },
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector,
                    args.alternative_selectors.as_deref(),
                    args.fallback_selectors.as_deref(),
                    e,
                )),
            }?;

        let mut png_data = Vec::new();
        let encoder = PngEncoder::new(Cursor::new(&mut png_data));
        encoder
            .write_image(
                &screenshot_result.image_data,
                screenshot_result.width,
                screenshot_result.height,
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

        Ok(CallToolResult::success(vec![
            Content::json(json!({
                "action": "capture_element_screenshot",
                "status": "success",
                "element": element_info,
                "selector_used": successful_selector,
                "selectors_tried": get_selectors_tried_all(&args.selector, args.alternative_selectors.as_deref(), args.fallback_selectors.as_deref()),
                "image_format": "png",
            }))?,
            Content::image(base64_image, "image/png".to_string()),
        ]))
    }

    #[tool(
        description = "Invokes a UI element. This is often more reliable than clicking for controls like radio buttons or menu items. This action requires the application to be focused and may change the UI."
    )]
    async fn invoke_element(
        &self,
        Parameters(args): Parameters<LocatorArgs>,
    ) -> Result<CallToolResult, McpError> {
        let ((result, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector,
                args.alternative_selectors.as_deref(),
                args.fallback_selectors.as_deref(),
                args.timeout_ms,
                args.retries,
                |element| async move {
                    // Ensure element is visible before interaction
                    if let Err(e) = Self::ensure_element_in_view(&element) {
                        tracing::warn!("Failed to ensure element is in view for invoke: {e}");
                    }
                    element.invoke_with_state()
                },
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector,
                    args.alternative_selectors.as_deref(),
                    args.fallback_selectors.as_deref(),
                    e,
                )),
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
            "selectors_tried": get_selectors_tried_all(&args.selector, args.alternative_selectors.as_deref(), args.fallback_selectors.as_deref()),
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        maybe_attach_tree(
            &self.desktop,
            Self::get_include_tree_default(args.include_tree),
            args.include_detailed_attributes,
            element.process_id().ok(),
            &mut result_json,
        );

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Records a user's UI interactions into a reusable workflow file. Use action: 'start' to begin recording and 'stop' to end and save the workflow. This allows a human to demonstrate a task for the AI to learn."
    )]
    pub async fn record_workflow(
        &self,
        Parameters(args): Parameters<RecordWorkflowArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut recorder_guard = self.recorder.lock().await;

        match args.action.as_str() {
            "start" => {
                if recorder_guard.is_some() {
                    return Err(McpError::invalid_params(
                        "Recording is already in progress. Please stop the current recording first.",
                        None,
                    ));
                }

                let workflow_name = args.workflow_name.ok_or_else(|| {
                    McpError::invalid_params(
                        "`workflow_name` is required to start recording.",
                        None,
                    )
                })?;

                let config = if args.low_energy_mode.unwrap_or(false) {
                    // This uses a config optimized for performance, which importantly disables
                    // text input completion tracking, a feature the user found caused lag.
                    PerformanceMode::low_energy_config()
                } else {
                    WorkflowRecorderConfig {
                        filter_mouse_noise: !args.record_scroll_events.unwrap_or(false), // Filter out mouse movements and wheel events unless scroll recording is enabled
                        ..WorkflowRecorderConfig::default()
                    }
                };

                let mut recorder = WorkflowRecorder::new(workflow_name.clone(), config);

                // Start highlighting task if enabled
                if let Some(ref highlight_config) = args.highlight_mode {
                    if highlight_config.enabled {
                        let mut event_stream = recorder.event_stream();
                        let highlight_cfg = highlight_config.clone();
                        let active_highlights = self.active_highlights.clone();

                        // Spawn a task to highlight elements as events are captured
                        tokio::spawn(async move {
                            while let Some(event) = event_stream.next().await {
                                // Get the UI element from the event metadata
                                let ui_element = match &event {
                                    terminator_workflow_recorder::WorkflowEvent::Click(e) => {
                                        e.metadata.ui_element.as_ref()
                                    }
                                    terminator_workflow_recorder::WorkflowEvent::TextInputCompleted(e) => {
                                        e.metadata.ui_element.as_ref()
                                    }
                                    terminator_workflow_recorder::WorkflowEvent::Keyboard(e) => {
                                        e.metadata.ui_element.as_ref()
                                    }
                                    terminator_workflow_recorder::WorkflowEvent::DragDrop(e) => {
                                        e.metadata.ui_element.as_ref()
                                    }
                                    terminator_workflow_recorder::WorkflowEvent::ApplicationSwitch(e) => {
                                        e.metadata.ui_element.as_ref()
                                    }
                                    terminator_workflow_recorder::WorkflowEvent::BrowserTabNavigation(e) => {
                                        e.metadata.ui_element.as_ref()
                                    }
                                    terminator_workflow_recorder::WorkflowEvent::Mouse(e) => {
                                        e.metadata.ui_element.as_ref()
                                    }
                                    _ => None,
                                };

                                if let Some(ui_element) = ui_element {
                                    // Determine the event type label
                                    let event_label_string;
                                    let event_label = match &event {
                                        terminator_workflow_recorder::WorkflowEvent::Click(_) => "CLICK",
                                        terminator_workflow_recorder::WorkflowEvent::TextInputCompleted(_) => "TYPE",
                                        terminator_workflow_recorder::WorkflowEvent::Keyboard(e) => {
                                            // Show the key code for keyboard events
                                            event_label_string = format!("KEY: {}", e.key_code);
                                            &event_label_string
                                        }
                                        terminator_workflow_recorder::WorkflowEvent::DragDrop(_) => "DRAG",
                                        terminator_workflow_recorder::WorkflowEvent::ApplicationSwitch(_) => "SWITCH",
                                        terminator_workflow_recorder::WorkflowEvent::BrowserTabNavigation(_) => "TAB",
                                        terminator_workflow_recorder::WorkflowEvent::Mouse(e) => {
                                            match e.button {
                                                terminator_workflow_recorder::MouseButton::Right => "RCLICK",
                                                terminator_workflow_recorder::MouseButton::Middle => "MCLICK",
                                                _ => "MOUSE",
                                            }
                                        }
                                        _ => "EVENT",
                                    };

                                    // Highlight the element with the configured settings
                                    if let Ok(handle) = ui_element.highlight(
                                        highlight_cfg.color,
                                        highlight_cfg.duration_ms.map(Duration::from_millis),
                                        if highlight_cfg.show_labels {
                                            Some(event_label)
                                        } else {
                                            None
                                        },
                                        #[cfg(target_os = "windows")]
                                        highlight_cfg.label_position.clone().map(|pos| pos.into()),
                                        #[cfg(not(target_os = "windows"))]
                                        None,
                                        #[cfg(target_os = "windows")]
                                        highlight_cfg.label_style.clone().map(|style| style.into()),
                                        #[cfg(not(target_os = "windows"))]
                                        None,
                                    ) {
                                        // Track handle and schedule cleanup
                                        {
                                            let mut list = active_highlights.lock().await;
                                            list.push(handle);
                                        }
                                        let active_highlights_clone = active_highlights.clone();
                                        let expire_after = highlight_cfg.duration_ms.unwrap_or(500);
                                        tokio::spawn(async move {
                                            tokio::time::sleep(Duration::from_millis(expire_after))
                                                .await;
                                            let mut list = active_highlights_clone.lock().await;
                                            // Natural expiry: drop one handle (LIFO best-effort)
                                            let _ = list.pop();
                                        });
                                    }
                                }
                            }
                        });

                        info!("Recording started with visual highlighting enabled");
                    }
                }

                recorder.start().await.map_err(|e| {
                    McpError::internal_error(
                        "Failed to start recorder",
                        Some(json!({ "error": e.to_string() })),
                    )
                })?;

                *recorder_guard = Some(recorder);

                let mut response = json!({
                    "action": "record_workflow",
                    "status": "started",
                    "workflow_name": workflow_name,
                    "message": "Recording started. Perform the UI actions you want to record. Call this tool again with action: 'stop' to finish."
                });

                // Add highlighting status to response
                if let Some(ref highlight_config) = args.highlight_mode {
                    if highlight_config.enabled {
                        response["highlighting_enabled"] = json!(true);
                        response["highlight_color"] =
                            json!(highlight_config.color.unwrap_or(0x0000FF));
                        response["highlight_duration_ms"] =
                            json!(highlight_config.duration_ms.unwrap_or(500));
                    }
                }

                Ok(CallToolResult::success(vec![Content::json(response)?]))
            }
            "stop" => {
                let mut recorder = recorder_guard.take().ok_or_else(|| {
                    McpError::invalid_params(
                        "No recording is currently in progress. Please start a recording first.",
                        None,
                    )
                })?;

                recorder.stop().await.map_err(|e| {
                    McpError::internal_error(
                        "Failed to stop recorder",
                        Some(json!({ "error": e.to_string() })),
                    )
                })?;

                let workflow_name = {
                    let workflow = recorder.workflow.lock().unwrap();
                    workflow.name.clone()
                };

                let file_name = args.file_path.unwrap_or_else(|| {
                    let sanitized_name = workflow_name.to_lowercase().replace(' ', "_");
                    format!(
                        "{}_workflow_{}.json",
                        sanitized_name,
                        chrono::Utc::now().format("%Y%m%d_%H%M%S")
                    )
                });

                // Save in the system's temporary directory to ensure write permissions
                let save_dir = std::env::temp_dir().join("terminator_workflows");
                std::fs::create_dir_all(&save_dir).map_err(|e| {
                    McpError::internal_error(
                        "Failed to create workflow directory in temp folder",
                        Some(json!({ "error": e.to_string(), "path": save_dir.to_string_lossy() })),
                    )
                })?;

                let file_path = save_dir.join(file_name);

                info!("Saving workflow to {}", file_path.display());

                recorder.save(&file_path).map_err(|e| {
                    McpError::internal_error(
                        "Failed to save workflow",
                        Some(
                            json!({ "error": e.to_string(), "path": file_path.to_string_lossy() }),
                        ),
                    )
                })?;

                // Convert the recorded workflow to MCP sequences
                let mcp_workflow = match crate::workflow_converter::load_and_convert_workflow(
                    file_path.to_str().unwrap_or_default(),
                )
                .await
                {
                    Ok(mcp_workflow) => {
                        info!("Successfully converted workflow to MCP sequences");

                        // Return null if no steps were converted
                        if mcp_workflow.steps.is_empty() {
                            info!("No convertible events found in workflow");
                            None
                        } else {
                            // Build mcp_workflow object with conversion_notes at the root level
                            let mut workflow_obj = json!({
                                "tool_name": "execute_sequence",
                                "arguments": {
                                    "items": mcp_workflow.steps
                                }
                            });

                            // Add conversion_notes at the root level if they exist
                            if let Some(metadata) = &mcp_workflow.metadata {
                                if !metadata.conversion_notes.is_empty() {
                                    workflow_obj["conversion_notes"] =
                                        json!(metadata.conversion_notes);
                                }
                            }

                            Some(workflow_obj)
                        }
                    }
                    Err(e) => {
                        warn!("Failed to convert workflow to MCP: {}", e);
                        None
                    }
                };

                // Build response matching client expectations
                let mut response = json!({
                    "status": "success",
                    "file_path": file_path.to_string_lossy()
                });

                // Add MCP workflow if conversion was successful, otherwise null
                response["mcp_workflow"] = mcp_workflow.unwrap_or(serde_json::Value::Null);

                Ok(CallToolResult::success(vec![Content::json(response)?]))
            }
            _ => Err(McpError::invalid_params(
                "Invalid action. Must be 'start' or 'stop'.",
                Some(json!({ "provided_action": args.action })),
            )),
        }
    }

    #[tool(
        description = "Stops active element highlights immediately. If an ID is provided, stops that specific highlight; otherwise stops all."
    )]
    async fn stop_highlighting(
        &self,
        Parameters(_args): Parameters<crate::utils::StopHighlightingArgs>,
    ) -> Result<CallToolResult, McpError> {
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
        Ok(CallToolResult::success(vec![Content::json(response)?]))
    }

    #[tool(
        description = "Executes multiple tools in sequence. Useful for automating complex workflows that require multiple steps. Each tool in the sequence can have its own error handling and delay configuration. Tool names can be provided either in short form (e.g., 'click_element') or full form (e.g., 'mcp_terminator-mcp-agent_click_element'). When using run_command with engine mode, data can be passed between steps using set_env - return { set_env: { key: value } } from one step and access with {{env.key}} in subsequent steps. Supports partial execution with 'start_from_step' and 'end_at_step' parameters to run specific step ranges. State is automatically persisted to .workflow_state folder in workflow's directory when using file:// URLs, allowing workflows to be resumed from any step."
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

    #[tool(
        description = "Edits workflow files using simple text find/replace operations. Works like sed - finds text patterns and replaces them, or appends content if no pattern specified."
    )]
    pub async fn export_workflow_sequence(
        &self,
        Parameters(args): Parameters<ExportWorkflowSequenceArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.export_workflow_sequence_impl(args).await
    }

    #[tool(description = "Load a YAML workflow file or scan folder for YAML workflow files")]
    pub async fn import_workflow_sequence(
        &self,
        Parameters(args): Parameters<ImportWorkflowSequenceArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.import_workflow_sequence_impl(args).await
    }

    #[tool(description = "Maximizes a window.")]
    async fn maximize_window(
        &self,
        Parameters(args): Parameters<MaximizeWindowArgs>,
    ) -> Result<CallToolResult, McpError> {
        let ((_result, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector,
                args.alternative_selectors.as_deref(),
                args.fallback_selectors.as_deref(),
                args.timeout_ms,
                args.retries,
                |element| async move { element.maximize_window() },
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector,
                    args.alternative_selectors.as_deref(),
                    args.fallback_selectors.as_deref(),
                    e,
                )),
            }?;

        let element_info = build_element_info(&element);

        let mut result_json = json!({
            "action": "maximize_window",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector, args.alternative_selectors.as_deref(), args.fallback_selectors.as_deref()),
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        maybe_attach_tree(
            &self.desktop,
            Self::get_include_tree_default(args.include_tree),
            args.include_detailed_attributes,
            element.process_id().ok(),
            &mut result_json,
        );

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(description = "Minimizes a window.")]
    async fn minimize_window(
        &self,
        Parameters(args): Parameters<MinimizeWindowArgs>,
    ) -> Result<CallToolResult, McpError> {
        let ((_result, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector,
                args.alternative_selectors.as_deref(),
                args.fallback_selectors.as_deref(),
                args.timeout_ms,
                args.retries,
                |element| async move { element.minimize_window() },
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector,
                    args.alternative_selectors.as_deref(),
                    args.fallback_selectors.as_deref(),
                    e,
                )),
            }?;

        let element_info = build_element_info(&element);

        let mut result_json = json!({
            "action": "minimize_window",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector, args.alternative_selectors.as_deref(), args.fallback_selectors.as_deref()),
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        maybe_attach_tree(
            &self.desktop,
            Self::get_include_tree_default(args.include_tree),
            args.include_detailed_attributes,
            element.process_id().ok(),
            &mut result_json,
        );

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(description = "Zooms in on the current view (e.g., a web page).")]
    async fn zoom_in(
        &self,
        Parameters(args): Parameters<ZoomArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.desktop.zoom_in(args.level).await.map_err(|e| {
            McpError::internal_error("Failed to zoom in", Some(json!({"reason": e.to_string()})))
        })?;
        Ok(CallToolResult::success(vec![Content::json(json!({
            "action": "zoom_in",
            "status": "success",
            "level": args.level,
        }))?]))
    }

    #[tool(description = "Zooms out on the current view (e.g., a web page).")]
    async fn zoom_out(
        &self,
        Parameters(args): Parameters<ZoomArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.desktop.zoom_out(args.level).await.map_err(|e| {
            McpError::internal_error("Failed to zoom out", Some(json!({"reason": e.to_string()})))
        })?;
        Ok(CallToolResult::success(vec![Content::json(json!({
            "action": "zoom_out",
            "status": "success",
            "level": args.level,
        }))?]))
    }

    #[tool(
        description = "Sets the zoom level to a specific percentage (e.g., 100 for 100%, 150 for 150%, 50 for 50%). This is more precise than using zoom_in/zoom_out repeatedly."
    )]
    async fn set_zoom(
        &self,
        Parameters(args): Parameters<SetZoomArgs>,
    ) -> Result<CallToolResult, McpError> {
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
            Self::get_include_tree_default(args.include_tree),
            args.include_detailed_attributes,
            None, // No specific element for zoom operation
            &mut result_json,
        );

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Sets the text value of an editable control (e.g., an input field) directly using the underlying accessibility API. This action requires the application to be focused and may change the UI."
    )]
    async fn set_value(
        &self,
        Parameters(args): Parameters<SetValueArgs>,
    ) -> Result<CallToolResult, McpError> {
        let value_to_set = args.value.clone();
        let action = move |element: UIElement| {
            let value_to_set = value_to_set.clone();
            async move { element.set_value(&value_to_set) }
        };

        let ((_result, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector,
                args.alternative_selectors.as_deref(),
                args.fallback_selectors.as_deref(),
                args.timeout_ms,
                args.retries,
                action,
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => Err(build_element_not_found_error(
                    &args.selector,
                    args.alternative_selectors.as_deref(),
                    args.fallback_selectors.as_deref(),
                    e,
                )),
            }?;

        let element_info = build_element_info(&element);

        let mut result_json = json!({
            "action": "set_value",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector, args.alternative_selectors.as_deref(), args.fallback_selectors.as_deref()),
            "value_set_to": args.value,
        });
        maybe_attach_tree(
            &self.desktop,
            Self::get_include_tree_default(args.include_tree),
            args.include_detailed_attributes,
            Some(element.process_id().unwrap_or(0)),
            &mut result_json,
        );
        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    // Removed: run_javascript tool (merged into run_command with engine)

    #[tool(
        description = "Execute JavaScript in a browser using the Chrome extension bridge. Provides full access to the HTML DOM for data extraction, page analysis, and manipulation. Returns serializable data (strings, numbers, objects, arrays). 

Key uses:
- Extract full HTML DOM: document.documentElement.outerHTML
- Get structured page data: forms, links, meta tags, hidden inputs
- Analyze page structure: headings, images, element counts
- Debug accessibility tree gaps
- Scrape data not available via accessibility APIs
- Pass data between workflow steps using env/outputs parameters

Parameters:
- script: JavaScript code to execute (optional if script_file is provided)
- script_file: Path to JavaScript file to load and execute (optional)
- env: Environment variables to inject as 'var env = {...}' (optional)
- outputs: Outputs from previous steps to inject as 'var outputs = {...}' (optional)

Data injection: When env/outputs are provided, they're injected as JavaScript variables at the start of your script. Parse them if they're JSON strings:
const parsedEnv = typeof env === 'string' ? JSON.parse(env) : env;
const parsedOutputs = typeof outputs === 'string' ? JSON.parse(outputs) : outputs;

Returning data: Scripts can set environment variables for subsequent steps:
return JSON.stringify({
  set_env: { key: 'value' },
  result: 'success'
});

Size limits: Response must be <30KB. For large DOMs, use truncation:
const html = document.documentElement.outerHTML;
const max = 30000;
return html.length > max ? html.substring(0, max) + '...' : html;

Requires Chrome extension to be installed. See browser_dom_extraction.yml and demo_bidirectional_vars.yml for examples."
    )]
    async fn execute_browser_script(
        &self,
        Parameters(args): Parameters<ExecuteBrowserScriptArgs>,
    ) -> Result<CallToolResult, McpError> {
        use serde_json::json;
        let start_instant = std::time::Instant::now();

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

                    // Priority 3: Use as-is if still not found
                    if resolved_path.is_none() {
                        let candidate = script_path.to_path_buf();
                        resolution_attempts.push(format!("as-is: {}", candidate.display()));
                        tracing::info!(
                            "[execute_browser_script] Using path as-is (not found in base paths): {}",
                            script_file
                        );
                        resolved_path = Some(candidate);
                    }
                } else {
                    // Absolute path - use as-is
                    tracing::info!(
                        "[execute_browser_script] Using absolute path: {}",
                        script_file
                    );
                    resolved_path = Some(script_path.to_path_buf());
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
                return Err(McpError::invalid_params("Script cannot be empty", None));
            }
            script.clone()
        } else {
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
            format!("Object.assign({{}}, {accumulated_env_json}, {explicit_env_json})")
        } else {
            accumulated_env_json.clone()
        };

        // Track which variables will be injected
        let mut injected_vars = std::collections::HashSet::new();

        if let Ok(env_obj) =
            serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&merged_env)
        {
            for (key, value) in env_obj {
                if Self::is_valid_js_identifier(&key) {
                    if let Ok(value_json) = serde_json::to_string(&value) {
                        final_script.push_str(&format!("var {key} = {value_json};\n"));
                        injected_vars.insert(key.clone()); // Track this variable
                        tracing::debug!(
                            "[execute_browser_script] Injected env.{} as individual variable",
                            key
                        );
                    }
                }
            }
        }

        // Inject variables
        final_script.push_str(&format!("var variables = {variables_json};\n"));
        tracing::debug!("[execute_browser_script] Injected accumulated env, explicit env, individual vars, and workflow variables");

        // Smart replacement of declarations with assignments for already-injected variables
        let mut modified_script = script_content.clone();
        if !injected_vars.is_empty() {
            tracing::debug!(
                "[execute_browser_script] Checking for variable declarations to replace. Injected vars: {:?}",
                injected_vars
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

        // Append the modified script
        final_script.push_str(&modified_script);

        let script_len = final_script.len();
        let script_preview: String = final_script.chars().take(200).collect();
        tracing::info!(
            "[execute_browser_script] start selector='{}' timeout_ms={:?} retries={:?} script_bytes={}",
            args.selector,
            args.timeout_ms,
            args.retries,
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
                &args.selector,
                args.alternative_selectors.as_deref(),
                args.fallback_selectors.as_deref(),
                args.timeout_ms,
                args.retries,
                |el| {
                    let script = script_clone.clone();
                    async move { el.execute_browser_script(&script).await }
                },
            )
            .await
            {
                Ok(((result, element), selector)) => Ok(((result, element), selector)),
                Err(e) => {
                    tracing::error!(
                        "[execute_browser_script] failed selector='{}' alt='{:?}' fallback='{:?}' error={}",
                        args.selector,
                        args.alternative_selectors,
                        args.fallback_selectors,
                        e
                    );

                    // Check if this is a JavaScript execution error
                    if let Some(AutomationError::PlatformError(msg)) =
                        e.downcast_ref::<AutomationError>()
                    {
                        if msg.contains("JavaScript") || msg.contains("script") {
                            // Return JavaScript-specific error, not "Element not found"
                            return Err(McpError::invalid_params(
                                "Browser script execution failed",
                                Some(json!({
                                    "error_type": "script_execution_failure",
                                    "message": msg.clone(),
                                    "selector": args.selector,
                                    "selectors_tried": get_selectors_tried_all(
                                        &args.selector,
                                        args.alternative_selectors.as_deref(),
                                        args.fallback_selectors.as_deref(),
                                    ),
                                    "suggestion": "Check the browser console for JavaScript errors. The script may have timed out or encountered an error."
                                })),
                            ));
                        }
                    }

                    // For other errors, treat as element not found
                    Err(build_element_not_found_error(
                        &args.selector,
                        args.alternative_selectors.as_deref(),
                        args.fallback_selectors.as_deref(),
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
            &args.selector,
            args.alternative_selectors.as_deref(),
            args.fallback_selectors.as_deref(),
        );

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
            "result": script_result,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "duration_ms": elapsed_ms,
            "script_bytes": script_len,
        });

        // Always attach tree for better context
        maybe_attach_tree(
            &self.desktop,
            Self::get_include_tree_default(args.include_tree),
            args.include_detailed_attributes,
            None, // Don't filter by process since this could apply to any browser
            &mut result_json,
        );

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    pub(crate) async fn dispatch_tool(
        &self,
        _peer: Peer<RoleServer>,
        request_context: RequestContext<RoleServer>,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<CallToolResult, McpError> {
        use rmcp::handler::server::wrapper::Parameters;
        match tool_name {
            "get_window_tree" => {
                match serde_json::from_value::<GetWindowTreeArgs>(arguments.clone()) {
                    Ok(args) => self.get_window_tree(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for get_window_tree",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "get_focused_window_tree" => {
                match serde_json::from_value::<GetFocusedWindowTreeArgs>(arguments.clone()) {
                    Ok(args) => self.get_focused_window_tree(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for get_focused_window_tree",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "get_applications" => {
                match serde_json::from_value::<GetApplicationsArgs>(arguments.clone()) {
                    Ok(args) => self.get_applications(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for get_applications",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "click_element" => {
                match serde_json::from_value::<ClickElementArgs>(arguments.clone()) {
                    Ok(args) => self.click_element(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for click_element",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "type_into_element" => {
                match serde_json::from_value::<TypeIntoElementArgs>(arguments.clone()) {
                    Ok(args) => self.type_into_element(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for type_into_element",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "press_key" => match serde_json::from_value::<PressKeyArgs>(arguments.clone()) {
                Ok(args) => self.press_key(Parameters(args)).await,
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
                    Ok(args) => self.wait_for_element(Parameters(args)).await,
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
                    Ok(args) => self.navigate_browser(Parameters(args)).await,
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
                    tokio::spawn(async move {
                        ct_for_task.cancelled().await;
                        cancellation_token.cancel();
                    });

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
            "highlight_element" => {
                match serde_json::from_value::<HighlightElementArgs>(arguments.clone()) {
                    Ok(args) => self.highlight_element(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for highlight_element",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "close_element" => {
                match serde_json::from_value::<CloseElementArgs>(arguments.clone()) {
                    Ok(args) => self.close_element(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for close_element",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "select_option" => {
                match serde_json::from_value::<SelectOptionArgs>(arguments.clone()) {
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
                match serde_json::from_value::<ValidateElementArgs>(arguments.clone()) {
                    Ok(args) => self.capture_element_screenshot(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for capture_element_screenshot",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "invoke_element" => match serde_json::from_value::<LocatorArgs>(arguments.clone()) {
                Ok(args) => self.invoke_element(Parameters(args)).await,
                Err(e) => Err(McpError::invalid_params(
                    "Invalid arguments for invoke_element",
                    Some(json!({"error": e.to_string()})),
                )),
            },
            "record_workflow" => {
                match serde_json::from_value::<RecordWorkflowArgs>(arguments.clone()) {
                    Ok(args) => self.record_workflow(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for record_workflow",
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
            "zoom_in" => match serde_json::from_value::<ZoomArgs>(arguments.clone()) {
                Ok(args) => self.zoom_in(Parameters(args)).await,
                Err(e) => Err(McpError::invalid_params(
                    "Invalid arguments for zoom_in",
                    Some(json!({"error": e.to_string()})),
                )),
            },
            "zoom_out" => match serde_json::from_value::<ZoomArgs>(arguments.clone()) {
                Ok(args) => self.zoom_out(Parameters(args)).await,
                Err(e) => Err(McpError::invalid_params(
                    "Invalid arguments for zoom_out",
                    Some(json!({"error": e.to_string()})),
                )),
            },
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
            "export_workflow_sequence" => {
                match serde_json::from_value::<ExportWorkflowSequenceArgs>(arguments.clone()) {
                    Ok(args) => self.export_workflow_sequence(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for export_workflow_sequence",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "import_workflow_sequence" => {
                match serde_json::from_value::<ImportWorkflowSequenceArgs>(arguments.clone()) {
                    Ok(args) => self.import_workflow_sequence(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for import_workflow_sequence",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "execute_sequence" => {
                // For execute_sequence, we need peer and request_context
                // Since we don't have them here, this is a special case that should be handled differently
                Err(McpError::internal_error(
                    "execute_sequence requires special handling",
                    Some(json!({"error": "Cannot dispatch execute_sequence through this method"})),
                ))
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
            _ => Err(McpError::internal_error(
                "Unknown tool called",
                Some(json!({"tool_name": tool_name})),
            )),
        }
    }
}

#[tool_handler]
impl ServerHandler for DesktopWrapper {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::LATEST,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_logging()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(crate::prompt::get_server_instructions().to_string()),
        }
    }
}
