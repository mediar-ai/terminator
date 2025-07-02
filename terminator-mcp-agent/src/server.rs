pub use crate::utils::DesktopWrapper;
use crate::utils::{
    get_timeout, ActivateElementArgs, ClickElementArgs, ClipboardArgs, DelayArgs, EmptyArgs,
    ExecuteSequenceArgs, ExportWorkflowSequenceArgs, GetApplicationsArgs, GetClipboardArgs,
    GetFocusedWindowTreeArgs, GetWindowTreeArgs, GetWindowsArgs, GlobalKeyArgs,
    HighlightElementArgs, LocatorArgs, MouseDragArgs, NavigateBrowserArgs, OpenApplicationArgs,
    PressKeyArgs, RunCommandArgs, ScrollElementArgs, SelectOptionArgs, SetRangeValueArgs,
    SetSelectedArgs, SetToggledArgs, ToolCall, TypeIntoElementArgs, ValidateElementArgs,
    WaitForElementArgs,
};
use chrono::Local;
use image::{ExtendedColorType, ImageEncoder};
use rmcp::handler::server::tool::Parameters;
use rmcp::model::{
    CallToolResult, Content, Implementation, ProgressNotification, ProgressNotificationParam,
    ProgressToken, ProtocolVersion, ServerCapabilities, ServerInfo,
};
use rmcp::{tool, Error as McpError, ServerHandler};
use rmcp::{tool_handler, tool_router};
use serde_json::{json, Value};
use std::env;
use std::future::Future;
use std::io::Cursor;
use std::sync::Arc;
use terminator::{Browser, Desktop, Selector, UIElement};

// New imports for image encoding
use base64::{engine::general_purpose, Engine as _};
use image::codecs::png::PngEncoder;

/// Helper function to parse comma-separated alternative selectors into a Vec<String>
fn parse_alternative_selectors(alternatives: Option<&str>) -> Vec<String> {
    alternatives
        .map(|alts| {
            alts.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Helper function to get all selectors tried (primary + alternatives) for error reporting
fn get_selectors_tried(primary: &str, alternatives: Option<&str>) -> Vec<String> {
    let mut all = vec![primary.to_string()];
    all.extend(parse_alternative_selectors(alternatives));
    all
}

/// Builds a standardized JSON object with detailed information about a UIElement.
/// This includes a suggested selector that prioritizes role|name over just the ID.
fn build_element_info(element: &UIElement) -> Value {
    let id = element.id().unwrap_or_default();
    let role = element.role();
    let name = element.name().unwrap_or_default();

    let suggested_selector = if !name.is_empty() && role != "Unknown" {
        format!("{}|{}", &role, &name)
    } else {
        format!("#{}", id)
    };

    json!({
        "name": name,
        "role": role,
        "id": id,
        "suggested_selector": suggested_selector,
        "application": element.application_name(),
        "window_title": element.window_title(),
        "process_id": element.process_id().unwrap_or(0),
        "is_focused": element.is_focused().unwrap_or(false),
        "text": element.text(0).unwrap_or_default(),
        "bounds": element.bounds().map(|b| json!({
            "x": b.0, "y": b.1, "width": b.2, "height": b.3
        })).unwrap_or(json!(null)),
        "enabled": element.is_enabled().unwrap_or(false),
        "is_selected": element.is_selected().unwrap_or(false),
        "is_toggled": element.is_toggled().unwrap_or(false),
        "keyboard_focusable": element.is_keyboard_focusable().unwrap_or(false),
    })
}

/// Builds a standardized, actionable error when an element cannot be found.
fn build_element_not_found_error(
    primary_selector: &str,
    alternatives: Option<&str>,
    original_error: anyhow::Error,
) -> McpError {
    let selectors_tried = get_selectors_tried(primary_selector, alternatives);
    let error_payload = json!({
        "error_type": "ElementNotFound",
        "message": format!("The specified element could not be found after trying all selectors. Original error: {}", original_error),
        "selectors_tried": selectors_tried,
        "suggestions": [
            "Call `get_window_tree` again to get a fresh view of the UI; it might have changed.",
            "Verify the element's 'name' and 'role' in the new UI tree. The 'name' attribute might be empty or different from the visible text.",
            "If the element has no 'name', use its numeric ID selector (e.g., '#12345'). This is required for many clickable 'Group' elements.",
            "Use `validate_element` with your selectors to debug existence issues before calling an action tool."
        ]
    });

    McpError::resource_not_found("Element not found", Some(error_payload))
}

#[tool_router]
impl DesktopWrapper {
    pub async fn new() -> Result<Self, McpError> {
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
        })
    }

    #[tool(
        description = "Get the complete UI tree for an application by PID and optional window title. This is your primary tool for understanding the application's current state. This is a read-only operation."
    )]
    pub async fn get_window_tree(
        &self,
        Parameters(args): Parameters<GetWindowTreeArgs>,
    ) -> Result<CallToolResult, McpError> {
        let tree = self
            .desktop
            .get_window_tree(
                args.pid,
                args.title.as_deref(),
                None, // Use default config for now
            )
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
        Parameters(_args): Parameters<crate::utils::GetFocusedWindowTreeArgs>,
    ) -> Result<CallToolResult, McpError> {
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
            .get_window_tree(
                pid,
                Some(&window_title),
                None, // Use default config
            )
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

        let include_tree = args.include_tree.unwrap_or(false);

        let app_info_futures: Vec<_> = apps
            .iter()
            .map(|app| {
                let desktop = self.desktop.clone();
                let app_name = app.name().unwrap_or_default();
                let app_id = app.id().unwrap_or_default();
                let app_role = app.role();
                let app_pid = app.process_id().unwrap_or(0);
                let is_focused = app.is_focused().unwrap_or(false);

                let suggested_selector = if !app_name.is_empty() {
                    format!("{}|{}", &app_role, &app_name)
                } else {
                    format!("#{}", app_id)
                };

                tokio::spawn(async move {
                    let tree = if include_tree && app_pid > 0 {
                        desktop.get_window_tree(app_pid, None, None).ok()
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
                        "alternative_selectors": [
                            format!("#{}", app_id),
                            format!("name:{}", app_name)
                        ],
                        "ui_tree": tree.and_then(|t| serde_json::to_value(t).ok())
                    })
                })
            })
            .collect();

        let results = futures::future::join_all(app_info_futures).await;
        let app_info: Vec<Value> = results.into_iter().filter_map(Result::ok).collect();

        Ok(CallToolResult::success(vec![Content::json(json!({
            "applications": app_info,
            "count": app_info.len(),
            "recommendation": "For applications, the name is usually reliable. For elements inside the app, prefer role|name selectors and use the ID as a fallback. Use get_window_tree with the PID for details."
        }))?]))
    }

    #[tool(description = "Get windows for a specific application by name.")]
    async fn get_windows_for_application(
        &self,
        Parameters(args): Parameters<GetWindowsArgs>,
    ) -> Result<CallToolResult, McpError> {
        let windows = self
            .desktop
            .windows_for_application(&args.app_name)
            .await
            .map_err(|e| {
                McpError::resource_not_found(
                    "Failed to get windows for application",
                    Some(json!({"reason": e.to_string()})),
                )
            })?;

        let window_info: Vec<_> = windows
            .iter()
            .map(|window| {
                json!({
                    "title": window.name().unwrap_or_default(),
                    "id": window.id().unwrap_or_default(),
                    "role": window.role(),
                    "bounds": window.bounds().map(|b| json!({
                        "x": b.0, "y": b.1, "width": b.2, "height": b.3
                    })).unwrap_or(json!(null)),
                    "suggested_selector": format!("name:{}", window.name().unwrap_or_default())
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::json(json!({
            "windows": window_info,
            "count": windows.len(),
            "application": args.app_name
        }))?]))
    }

    #[tool(
        description = "Types text into a UI element with smart clipboard optimization and verification. Much faster than press key. This action requires the application to be focused and may change the UI."
    )]
    async fn type_into_element(
        &self,
        Parameters(args): Parameters<TypeIntoElementArgs>,
    ) -> Result<CallToolResult, McpError> {
        use crate::utils::find_element_with_fallbacks;

        let (element, successful_selector) = find_element_with_fallbacks(
            &self.desktop,
            &args.selector,
            args.alternative_selectors.as_deref(),
            args.timeout_ms,
        )
        .await
        .map_err(|e| {
            build_element_not_found_error(
                &args.selector,
                args.alternative_selectors.as_deref(),
                e.into(),
            )
        })?;

        let element_info = build_element_info(&element);

        // Clear the element before typing if requested (default: true)
        let should_clear = args.clear_before_typing.unwrap_or(true);
        if should_clear {
            // Select all existing text and delete it
            if let Err(clear_error) = element
                .press_key("{Ctrl}a")
                .and_then(|_| element.press_key("{Delete}"))
            {
                // If clearing fails, log it but continue with typing (non-fatal)
                eprintln!(
                    "Warning: Failed to clear element before typing: {}",
                    clear_error
                );
            }
        }

        element.type_text(&args.text_to_type, true).map_err(|e| {
            McpError::resource_not_found(
                "Failed to type text",
                Some(json!({
                    "reason": e.to_string(),
                    "selector": args.selector,
                    "text_to_type": args.text_to_type,
                    "element_info": element_info
                })),
            )
        })?;

        let mut result_json = json!({
            "action": "type_into_element",
            "status": "success",
            "text_typed": args.text_to_type,
            "cleared_before_typing": should_clear,
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried(&args.selector, args.alternative_selectors.as_deref()),
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
                let verification = json!({
                    "element_focused": updated_element.is_focused().unwrap_or(false),
                    "element_enabled": updated_element.is_enabled().unwrap_or(false),
                    "verification_timestamp": chrono::Utc::now().to_rfc3339()
                });
                if let Some(obj) = result_json.as_object_mut() {
                    obj.insert("verification".to_string(), verification);
                }
            }
        }

        // Always attach tree for better context, or if explicitly requested
        self.maybe_attach_tree(
            true,
            Some(element.process_id().unwrap_or(0)),
            &mut result_json,
        );

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Clicks a UI element. This action requires the application to be focused and may change the UI."
    )]
    async fn click_element(
        &self,
        Parameters(args): Parameters<ClickElementArgs>,
    ) -> Result<CallToolResult, McpError> {
        use crate::utils::find_element_with_fallbacks;

        let (element, successful_selector) = find_element_with_fallbacks(
            &self.desktop,
            &args.selector,
            args.alternative_selectors.as_deref(),
            args.timeout_ms,
        )
        .await
        .map_err(|e| {
            build_element_not_found_error(
                &args.selector,
                args.alternative_selectors.as_deref(),
                e.into(),
            )
        })?;

        let element_info = build_element_info(&element);

        element.click().map_err(|e| {
            McpError::resource_not_found(
                "Failed to click on element",
                Some(json!({
                    "reason": e.to_string(),
                    "selector": args.selector,
                    "element_info": element_info
                })),
            )
        })?;

        // Build base result
        let mut result_json = json!({
            "action": "click",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried(&args.selector, args.alternative_selectors.as_deref()),
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        // --- Action Consequence Verification ---
        let mut consequence = "no_significant_change".to_string();
        std::thread::sleep(std::time::Duration::from_millis(250)); // Wait for UI to react

        // Check 1: Did the element disappear?
        let post_click_locator = self
            .desktop
            .locator(Selector::from(successful_selector.as_str()));
        if post_click_locator
            .wait(Some(std::time::Duration::from_millis(100)))
            .await
            .is_err()
        {
            consequence = "element_disappeared".to_string();
        } else {
            // Check 2: Did focus change?
            if let Ok(focused_element) = self.desktop.focused_element() {
                if focused_element.id_or_empty() != element.id_or_empty() {
                    consequence = format!("focus_changed_to: #{}", focused_element.id_or_empty());
                }
            }
        }

        if let Some(obj) = result_json.as_object_mut() {
            obj.insert("consequence".to_string(), json!(consequence));
        }
        // --- End Consequence Verification ---

        // Always attach tree for better context
        self.maybe_attach_tree(true, element.process_id().ok(), &mut result_json);

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Sends a key press to a UI element. Use curly brace format: '{Ctrl}c', '{Alt}{F4}', '{Enter}', '{PageDown}', etc. This action requires the application to be focused and may change the UI."
    )]
    async fn press_key(
        &self,
        Parameters(args): Parameters<PressKeyArgs>,
    ) -> Result<CallToolResult, McpError> {
        let locator = self.desktop.locator(Selector::from(args.selector.as_str()));
        let element = locator
            .wait(get_timeout(args.timeout_ms))
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "Failed to locate element",
                    Some(json!({"reason": e.to_string(), "selector": args.selector})),
                )
            })?;
        let element_info = build_element_info(&element);

        element.press_key(&args.key).map_err(|e| {
            McpError::resource_not_found(
                "Failed to press key",
                Some(json!({
                    "reason": e.to_string(),
                    "selector": args.selector,
                    "key_pressed": args.key,
                    "element_info": element_info
                })),
            )
        })?;

        let mut result_json = json!({
            "action": "press_key",
            "status": "success",
            "key_pressed": args.key,
            "element": element_info,
            "selector": args.selector,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });
        self.maybe_attach_tree(true, element.process_id().ok(), &mut result_json);

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
        let focused = self.desktop.focused_element().map_err(|e| {
            McpError::internal_error(
                "Failed to get focused element",
                Some(json!({"reason": e.to_string()})),
            )
        })?;

        // Gather metadata for debugging / result payload
        let element_info = build_element_info(&focused);

        // Perform the key press
        focused.press_key(&args.key).map_err(|e| {
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
        self.maybe_attach_tree(true, focused.process_id().ok(), &mut result_json);

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(description = "Executes a shell command.")]
    async fn run_command(
        &self,
        Parameters(args): Parameters<RunCommandArgs>,
    ) -> Result<CallToolResult, McpError> {
        let output = self
            .desktop
            .run_command(
                args.windows_command.as_deref(),
                args.unix_command.as_deref(),
            )
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "Failed to run command",
                    Some(json!({"reason": e.to_string()})),
                )
            })?;

        Ok(CallToolResult::success(vec![Content::json(json!({
            "exit_status": output.exit_status,
            "stdout": output.stdout,
            "stderr": output.stderr,
        }))?]))
    }

    #[tool(
        description = "Activates the window containing the specified element, bringing it to the foreground."
    )]
    async fn activate_element(
        &self,
        Parameters(args): Parameters<ActivateElementArgs>,
    ) -> Result<CallToolResult, McpError> {
        let locator = self.desktop.locator(Selector::from(args.selector.as_str()));
        let element = locator
            .wait(get_timeout(args.timeout_ms))
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "Failed to locate element",
                    Some(json!({"reason": e.to_string(), "selector": args.selector})),
                )
            })?;

        let element_info = build_element_info(&element);

        element.activate_window().map_err(|e| {
            McpError::resource_not_found(
                "Failed to activate window with that element",
                Some(json!({
                    "reason": e.to_string(),
                    "selector": args.selector,
                    "element_info": element_info
                })),
            )
        })?;

        let mut result_json = json!({
            "action": "activate_element",
            "status": "success",
            "element": element_info,
            "selector": args.selector,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "recommendation": "Window activated successfully. The UI tree is attached to help you find specific elements to interact with next."
        });

        // Always attach UI tree for activated elements to help with next actions
        self.maybe_attach_tree(
            true, // Always attach tree for activate_element since it's important for next actions
            Some(element.process_id().unwrap_or(0)),
            &mut result_json,
        );

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Captures a screenshot of the primary monitor and returns the recognized text content (OCR)."
    )]
    async fn capture_screen(
        &self,
        Parameters(_args): Parameters<EmptyArgs>,
    ) -> Result<CallToolResult, McpError> {
        let monitor = self.desktop.get_primary_monitor().await.map_err(|e| {
            McpError::internal_error(
                "Failed to get primary monitor",
                Some(json!({"reason": e.to_string()})),
            )
        })?;

        let screenshot = self.desktop.capture_monitor(&monitor).await.map_err(|e| {
            McpError::internal_error(
                "Failed to capture screen",
                Some(json!({"reason": e.to_string()})),
            )
        })?;

        let ocr_text = self
            .desktop
            .ocr_screenshot(&screenshot)
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "Failed to perform OCR",
                    Some(json!({"reason": e.to_string()})),
                )
            })?;

        Ok(CallToolResult::success(vec![Content::json(&ocr_text)?]))
    }

    #[tool(description = "Sets text to the system clipboard using shell commands.")]
    async fn set_clipboard(
        &self,
        Parameters(args): Parameters<ClipboardArgs>,
    ) -> Result<CallToolResult, McpError> {
        // todo use native clipbaord feature we implemented
        let result = if cfg!(target_os = "windows") {
            // Windows: echo "text" | clip
            let command = format!("echo \"{}\" | clip", args.text.replace("\"", "\\\""));
            self.desktop.run_command(Some(&command), None).await
        } else if cfg!(target_os = "macos") {
            // macOS: echo "text" | pbcopy
            let command = format!("echo \"{}\" | pbcopy", args.text.replace("\"", "\\\""));
            self.desktop.run_command(None, Some(&command)).await
        } else {
            // Linux: echo "text" | xclip -selection clipboard
            let command = format!(
                "echo \"{}\" | xclip -selection clipboard",
                args.text.replace("\"", "\\\"")
            );
            self.desktop.run_command(None, Some(&command)).await
        };

        result.map_err(|e| {
            McpError::internal_error(
                "Failed to set clipboard",
                Some(json!({"reason": e.to_string(), "text": args.text})),
            )
        })?;

        Ok(CallToolResult::success(vec![Content::json(json!({
            "action": "set_clipboard",
            "status": "success",
            "text": args.text,
            "method": "shell_command",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))?]))
    }

    #[tool(description = "Gets text from the system clipboard using shell commands.")]
    async fn get_clipboard(
        &self,
        Parameters(_args): Parameters<GetClipboardArgs>,
    ) -> Result<CallToolResult, McpError> {
        let command_result = if cfg!(target_os = "windows") {
            // Windows: powershell Get-Clipboard
            self.desktop
                .run_command(Some("powershell -command \"Get-Clipboard\""), None)
                .await
        } else if cfg!(target_os = "macos") {
            // macOS: pbpaste
            self.desktop.run_command(None, Some("pbpaste")).await
        } else {
            // Linux: xclip -selection clipboard -o
            self.desktop
                .run_command(None, Some("xclip -selection clipboard -o"))
                .await
        };

        match command_result {
            Ok(output) => Ok(CallToolResult::success(vec![Content::json(json!({
                "action": "get_clipboard",
                "status": "success",
                "text": output.stdout.trim(),
                "method": "shell_command",
                "timestamp": chrono::Utc::now().to_rfc3339()
            }))?])),
            Err(e) => Err(McpError::internal_error(
                "Failed to get clipboard text",
                Some(json!({"reason": e.to_string()})),
            )),
        }
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
        let locator = self.desktop.locator(Selector::from(args.selector.as_str()));
        let element = locator
            .wait(get_timeout(args.timeout_ms))
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "Failed to locate element",
                    Some(json!({"reason": e.to_string(), "selector": args.selector})),
                )
            })?;

        // Get element details before dragging for better feedback
        let element_info = build_element_info(&element);

        element
            .mouse_drag(args.start_x, args.start_y, args.end_x, args.end_y)
            .map_err(|e| {
                McpError::resource_not_found(
                    "Failed to perform mouse drag",
                    Some(json!({
                        "reason": e.to_string(),
                        "selector": args.selector,
                        "start": (args.start_x, args.start_y),
                        "end": (args.end_x, args.end_y),
                        "element_info": element_info
                    })),
                )
            })?;

        let mut result_json = json!({
            "action": "mouse_drag",
            "status": "success",
            "element": element_info,
            "selector": args.selector,
            "start": (args.start_x, args.start_y),
            "end": (args.end_x, args.end_y),
            "timestamp": chrono::Utc::now().to_rfc3339()
        });
        self.maybe_attach_tree(true, element.process_id().ok(), &mut result_json);

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Validates that an element exists and provides detailed information about it. This is a read-only operation."
    )]
    pub async fn validate_element(
        &self,
        Parameters(args): Parameters<ValidateElementArgs>,
    ) -> Result<CallToolResult, McpError> {
        use crate::utils::find_element_with_fallbacks;

        match find_element_with_fallbacks(
            &self.desktop,
            &args.selector,
            args.alternative_selectors.as_deref(),
            args.timeout_ms,
        )
        .await
        {
            Ok((element, successful_selector)) => {
                let mut element_info = build_element_info(&element);
                if let Some(obj) = element_info.as_object_mut() {
                    obj.insert("exists".to_string(), json!(true));
                }

                let mut result_json = json!({
                    "action": "validate_element",
                    "status": "success",
                    "element": element_info,
                    "selector_used": successful_selector,
                    "selectors_tried": get_selectors_tried(&args.selector, args.alternative_selectors.as_deref()),
                    "timestamp": chrono::Utc::now().to_rfc3339()
                });
                self.maybe_attach_tree(true, element.process_id().ok(), &mut result_json);

                Ok(CallToolResult::success(vec![Content::json(result_json)?]))
            }
            Err(e) => {
                let selectors_tried =
                    get_selectors_tried(&args.selector, args.alternative_selectors.as_deref());
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
        let locator = self.desktop.locator(Selector::from(args.selector.as_str()));
        let element = locator
            .wait(get_timeout(args.timeout_ms))
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "Failed to locate element for highlighting",
                    Some(json!({"reason": e.to_string(), "selector": args.selector})),
                )
            })?;

        let duration = args.duration_ms.map(std::time::Duration::from_millis);
        element.highlight(args.color, duration).map_err(|e| {
            McpError::internal_error(
                "Failed to highlight element",
                Some(json!({"reason": e.to_string(), "selector": args.selector})),
            )
        })?;

        let element_info = build_element_info(&element);

        let mut result_json = json!({
            "action": "highlight_element",
            "status": "success",
            "element": element_info,
            "selector": args.selector,
            "color": args.color.unwrap_or(0x0000FF),
            "duration_ms": args.duration_ms.unwrap_or(1000),
            "timestamp": chrono::Utc::now().to_rfc3339()
        });
        self.maybe_attach_tree(true, element.process_id().ok(), &mut result_json);

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Waits for an element to meet a specific condition (visible, enabled, focused, exists)."
    )]
    async fn wait_for_element(
        &self,
        Parameters(args): Parameters<WaitForElementArgs>,
    ) -> Result<CallToolResult, McpError> {
        let locator = self.desktop.locator(Selector::from(args.selector.as_str()));
        let timeout = get_timeout(args.timeout_ms);

        let condition_lower = args.condition.to_lowercase();
        let result = match condition_lower.as_str() {
            "exists" => match locator.wait(timeout).await {
                Ok(_) => Ok(true),
                Err(_) => Ok(false),
            },
            "visible" => match locator.wait(timeout).await {
                Ok(element) => element.is_visible().map_err(|e| {
                    McpError::internal_error(
                        "Failed to check visibility",
                        Some(json!({"reason": e.to_string()})),
                    )
                }),
                Err(e) => Err(McpError::internal_error(
                    "Element not found",
                    Some(json!({"reason": e.to_string()})),
                )),
            },
            "enabled" => match locator.wait(timeout).await {
                Ok(element) => element.is_enabled().map_err(|e| {
                    McpError::internal_error(
                        "Failed to check enabled state",
                        Some(json!({"reason": e.to_string()})),
                    )
                }),
                Err(e) => Err(McpError::internal_error(
                    "Element not found",
                    Some(json!({"reason": e.to_string()})),
                )),
            },
            "focused" => match locator.wait(timeout).await {
                Ok(element) => element.is_focused().map_err(|e| {
                    McpError::internal_error(
                        "Failed to check focus state",
                        Some(json!({"reason": e.to_string()})),
                    )
                }),
                Err(e) => Err(McpError::internal_error(
                    "Element not found",
                    Some(json!({"reason": e.to_string()})),
                )),
            },
            _ => Err(McpError::invalid_params(
                "Invalid condition. Valid conditions: exists, visible, enabled, focused",
                Some(json!({"provided_condition": args.condition})),
            )),
        };

        match result {
            Ok(condition_met) => Ok(CallToolResult::success(vec![Content::json(json!({
                "action": "wait_for_element",
                "status": "success",
                "condition": args.condition,
                "condition_met": condition_met,
                "selector": args.selector,
                "timeout_ms": args.timeout_ms.unwrap_or(5000),
                "timestamp": chrono::Utc::now().to_rfc3339()
            }))?])),
            Err(e) => Err(e),
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

        let tree = self
            .desktop
            .get_window_tree(
                ui_element.process_id().unwrap_or(0),
                ui_element.name().as_deref(),
                None,
            )
            .unwrap_or_default();

        Ok(CallToolResult::success(vec![Content::json(json!({
            "action": "navigate_browser",
            "status": "success",
            "url": args.url,
            "browser": args.browser,
            "element": element_info,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "ui_tree": tree
        }))?]))
    }

    #[tool(description = "Opens an application by name (uses SDK's built-in app launcher).")]
    async fn open_application(
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
    async fn close_element(
        &self,
        Parameters(args): Parameters<LocatorArgs>,
    ) -> Result<CallToolResult, McpError> {
        let locator = self.desktop.locator(Selector::from(args.selector.as_str()));
        let element = locator
            .wait(get_timeout(args.timeout_ms))
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "Failed to locate element for closing",
                    Some(json!({"reason": e.to_string(), "selector": args.selector})),
                )
            })?;

        // Get element details before closing for better feedback
        let element_info = build_element_info(&element);

        element.close().map_err(|e| {
            McpError::resource_not_found(
                "Failed to close element",
                Some(json!({
                    "reason": e.to_string(),
                    "selector": args.selector,
                    "element_info": element_info
                })),
            )
        })?;

        Ok(CallToolResult::success(vec![Content::json(json!({
            "action": "close_element",
            "status": "success",
            "element": element_info,
            "selector": args.selector,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))?]))
    }

    #[tool(description = "Scrolls a UI element in the specified direction by the given amount.")]
    async fn scroll_element(
        &self,
        Parameters(args): Parameters<ScrollElementArgs>,
    ) -> Result<CallToolResult, McpError> {
        let locator = self.desktop.locator(Selector::from(args.selector.as_str()));
        let element = locator
            .wait(get_timeout(args.timeout_ms))
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "Failed to locate element for scrolling",
                    Some(json!({"reason": e.to_string(), "selector": args.selector})),
                )
            })?;

        // Get element details before scrolling for better feedback
        let element_info = build_element_info(&element);

        element.scroll(&args.direction, args.amount).map_err(|e| {
            McpError::resource_not_found(
                "Failed to scroll element",
                Some(json!({
                    "reason": e.to_string(),
                    "selector": args.selector,
                    "direction": args.direction,
                    "amount": args.amount,
                    "element_info": element_info
                })),
            )
        })?;

        let mut result_json = json!({
            "action": "scroll_element",
            "status": "success",
            "element": element_info,
            "selector": args.selector,
            "direction": args.direction,
            "amount": args.amount,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });
        self.maybe_attach_tree(true, element.process_id().ok(), &mut result_json);

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(description = "Selects an option in a dropdown or combobox by its visible text.")]
    async fn select_option(
        &self,
        Parameters(args): Parameters<SelectOptionArgs>,
    ) -> Result<CallToolResult, McpError> {
        use crate::utils::find_element_with_fallbacks;

        let (element, successful_selector) = find_element_with_fallbacks(
            &self.desktop,
            &args.selector,
            args.alternative_selectors.as_deref(),
            args.timeout_ms,
        )
        .await
        .map_err(|e| {
            build_element_not_found_error(
                &args.selector,
                args.alternative_selectors.as_deref(),
                e.into(),
            )
        })?;

        let element_info = build_element_info(&element);

        element.select_option(&args.option_name).map_err(|e| {
            McpError::resource_not_found(
                "Failed to select option",
                Some(
                    json!({ "reason": e.to_string(), "selector": args.selector, "option": args.option_name }),
                ),
            )
        })?;

        let mut result_json = json!({
            "action": "select_option",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "option_selected": args.option_name,
        });
        self.maybe_attach_tree(true, element.process_id().ok(), &mut result_json);
        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Lists all available option strings from a dropdown, list box, or similar control. This is a read-only operation."
    )]
    async fn list_options(
        &self,
        Parameters(args): Parameters<LocatorArgs>,
    ) -> Result<CallToolResult, McpError> {
        use crate::utils::find_element_with_fallbacks;

        let (element, successful_selector) = find_element_with_fallbacks(
            &self.desktop,
            &args.selector,
            args.alternative_selectors.as_deref(),
            args.timeout_ms,
        )
        .await
        .map_err(|e| {
            build_element_not_found_error(
                &args.selector,
                args.alternative_selectors.as_deref(),
                e.into(),
            )
        })?;

        let element_info = build_element_info(&element);
        let options = element.list_options().map_err(|e| {
            McpError::internal_error(
                "Failed to list options",
                Some(json!({ "reason": e.to_string(), "selector": args.selector })),
            )
        })?;

        let mut result_json = json!({
            "action": "list_options",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "options": options,
            "count": options.len(),
        });
        self.maybe_attach_tree(true, element.process_id().ok(), &mut result_json);
        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Sets the state of a toggleable control (e.g., checkbox, switch). This action requires the application to be focused and may change the UI."
    )]
    async fn set_toggled(
        &self,
        Parameters(args): Parameters<SetToggledArgs>,
    ) -> Result<CallToolResult, McpError> {
        use crate::utils::find_element_with_fallbacks;

        let (element, successful_selector) = find_element_with_fallbacks(
            &self.desktop,
            &args.selector,
            args.alternative_selectors.as_deref(),
            args.timeout_ms,
        )
        .await
        .map_err(|e| {
            build_element_not_found_error(
                &args.selector,
                args.alternative_selectors.as_deref(),
                e.into(),
            )
        })?;

        let element_info = build_element_info(&element);

        element.set_toggled(args.state).map_err(|e| {
            McpError::internal_error(
                "Failed to set toggle state",
                Some(
                    json!({ "reason": e.to_string(), "selector": args.selector, "state": args.state }),
                ),
            )
        })?;

        let mut result_json = json!({
            "action": "set_toggled",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "state_set_to": args.state,
        });
        self.maybe_attach_tree(true, element.process_id().ok(), &mut result_json);
        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Sets the value of a range-based control like a slider. This action requires the application to be focused and may change the UI."
    )]
    async fn set_range_value(
        &self,
        Parameters(args): Parameters<SetRangeValueArgs>,
    ) -> Result<CallToolResult, McpError> {
        use crate::utils::find_element_with_fallbacks;

        let (element, successful_selector) = find_element_with_fallbacks(
            &self.desktop,
            &args.selector,
            args.alternative_selectors.as_deref(),
            args.timeout_ms,
        )
        .await
        .map_err(|e| {
            build_element_not_found_error(
                &args.selector,
                args.alternative_selectors.as_deref(),
                e.into(),
            )
        })?;

        let element_info = build_element_info(&element);

        element.set_range_value(args.value).map_err(|e| {
            McpError::internal_error(
                "Failed to set range value",
                Some(
                    json!({ "reason": e.to_string(), "selector": args.selector, "value": args.value }),
                ),
            )
        })?;

        let mut result_json = json!({
            "action": "set_range_value",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "value_set_to": args.value,
        });
        self.maybe_attach_tree(true, element.process_id().ok(), &mut result_json);
        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Sets the selection state of a selectable item (e.g., in a list or calendar). This action requires the application to be focused and may change the UI."
    )]
    async fn set_selected(
        &self,
        Parameters(args): Parameters<SetSelectedArgs>,
    ) -> Result<CallToolResult, McpError> {
        use crate::utils::find_element_with_fallbacks;

        let (element, successful_selector) = find_element_with_fallbacks(
            &self.desktop,
            &args.selector,
            args.alternative_selectors.as_deref(),
            args.timeout_ms,
        )
        .await
        .map_err(|e| {
            build_element_not_found_error(
                &args.selector,
                args.alternative_selectors.as_deref(),
                e.into(),
            )
        })?;

        let element_info = build_element_info(&element);

        element.set_selected(args.state).map_err(|e| {
            McpError::internal_error(
                "Failed to set selected state",
                Some(
                    json!({ "reason": e.to_string(), "selector": args.selector, "state": args.state }),
                ),
            )
        })?;

        let mut result_json = json!({
            "action": "set_selected",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "state_set_to": args.state,
        });
        self.maybe_attach_tree(true, element.process_id().ok(), &mut result_json);
        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Checks if a control (like a checkbox or toggle switch) is currently toggled on. This is a read-only operation."
    )]
    async fn is_toggled(
        &self,
        Parameters(args): Parameters<LocatorArgs>,
    ) -> Result<CallToolResult, McpError> {
        use crate::utils::find_element_with_fallbacks;

        let (element, successful_selector) = find_element_with_fallbacks(
            &self.desktop,
            &args.selector,
            args.alternative_selectors.as_deref(),
            args.timeout_ms,
        )
        .await
        .map_err(|e| {
            build_element_not_found_error(
                &args.selector,
                args.alternative_selectors.as_deref(),
                e.into(),
            )
        })?;

        let element_info = build_element_info(&element);
        let is_toggled = element.is_toggled().map_err(|e| {
            McpError::internal_error(
                "Failed to get toggle state",
                Some(json!({ "reason": e.to_string(), "selector": args.selector })),
            )
        })?;

        let mut result_json = json!({
            "action": "is_toggled",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "is_toggled": is_toggled,
        });
        self.maybe_attach_tree(true, element.process_id().ok(), &mut result_json);
        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Gets the current value from a range-based control like a slider or progress bar. This is a read-only operation."
    )]
    async fn get_range_value(
        &self,
        Parameters(args): Parameters<LocatorArgs>,
    ) -> Result<CallToolResult, McpError> {
        use crate::utils::find_element_with_fallbacks;

        let (element, successful_selector) = find_element_with_fallbacks(
            &self.desktop,
            &args.selector,
            args.alternative_selectors.as_deref(),
            args.timeout_ms,
        )
        .await
        .map_err(|e| {
            build_element_not_found_error(
                &args.selector,
                args.alternative_selectors.as_deref(),
                e.into(),
            )
        })?;

        let element_info = build_element_info(&element);
        let value = element.get_range_value().map_err(|e| {
            McpError::internal_error(
                "Failed to get range value",
                Some(json!({ "reason": e.to_string(), "selector": args.selector })),
            )
        })?;

        let mut result_json = json!({
            "action": "get_range_value",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "value": value,
        });
        self.maybe_attach_tree(true, element.process_id().ok(), &mut result_json);
        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Checks if a selectable item (e.g., in a calendar, list, or tab) is currently selected. This is a read-only operation."
    )]
    async fn is_selected(
        &self,
        Parameters(args): Parameters<LocatorArgs>,
    ) -> Result<CallToolResult, McpError> {
        use crate::utils::find_element_with_fallbacks;

        let (element, successful_selector) = find_element_with_fallbacks(
            &self.desktop,
            &args.selector,
            args.alternative_selectors.as_deref(),
            args.timeout_ms,
        )
        .await
        .map_err(|e| {
            build_element_not_found_error(
                &args.selector,
                args.alternative_selectors.as_deref(),
                e.into(),
            )
        })?;

        let element_info = build_element_info(&element);
        let is_selected = element.is_selected().map_err(|e| {
            McpError::internal_error(
                "Failed to get selected state",
                Some(json!({ "reason": e.to_string(), "selector": args.selector })),
            )
        })?;

        let mut result_json = json!({
            "action": "is_selected",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "is_selected": is_selected,
        });
        self.maybe_attach_tree(true, element.process_id().ok(), &mut result_json);
        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(description = "Captures a screenshot of a specific UI element.")]
    async fn capture_element_screenshot(
        &self,
        Parameters(args): Parameters<ValidateElementArgs>,
    ) -> Result<CallToolResult, McpError> {
        use crate::utils::find_element_with_fallbacks;

        let (element, successful_selector) = find_element_with_fallbacks(
            &self.desktop,
            &args.selector,
            args.alternative_selectors.as_deref(),
            args.timeout_ms,
        )
        .await
        .map_err(|e| {
            let selectors_tried =
                get_selectors_tried(&args.selector, args.alternative_selectors.as_deref());
            McpError::internal_error(
                "Failed to locate element for screenshot",
                Some(json!({
                    "reason": e.to_string(),
                    "selectors_tried": selectors_tried,
                })),
            )
        })?;

        let screenshot_result = element.capture().map_err(|e| {
            McpError::internal_error(
                "Failed to capture element screenshot",
                Some(json!({"reason": e.to_string(), "selector_used": successful_selector})),
            )
        })?;

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
        use crate::utils::find_element_with_fallbacks;

        let (element, successful_selector) = find_element_with_fallbacks(
            &self.desktop,
            &args.selector,
            args.alternative_selectors.as_deref(),
            args.timeout_ms,
        )
        .await
        .map_err(|e| {
            build_element_not_found_error(
                &args.selector,
                args.alternative_selectors.as_deref(),
                e.into(),
            )
        })?;

        let element_info = build_element_info(&element);

        element.invoke().map_err(|e| {
            McpError::resource_not_found(
                "Failed to invoke element",
                Some(json!({
                    "reason": e.to_string(),
                    "selector": args.selector,
                    "element_info": element_info.clone()
                })),
            )
        })?;

        let mut result_json = json!({
            "action": "invoke",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried(&args.selector, args.alternative_selectors.as_deref()),
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        self.maybe_attach_tree(true, element.process_id().ok(), &mut result_json);

        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Executes a sequence of automation tools in order. Supports real-time progress notifications when a progress token is provided in the meta field. Progress updates are sent before and after each step, allowing clients to track execution status, current step, and completion percentage. Each step's result includes detailed timing and status information."
    )]
    pub async fn execute_sequence(
        &self,
        Parameters(args): Parameters<ExecuteSequenceArgs>,
        meta: rmcp::model::Meta,
        peer: rmcp::Peer<rmcp::RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        use rmcp::handler::server::tool::Parameters;

        let stop_on_error = args.stop_on_error.unwrap_or(true);
        let include_detailed = args.include_detailed_results.unwrap_or(true);

        // Check if we have a progress token for sending notifications
        let progress_token = meta.get_progress_token();

        // Parse the JSON string into an array of tool calls
        let tools_array: Vec<serde_json::Value> = serde_json::from_str(&args.tools_json)
            .map_err(|e| McpError::invalid_params(
                "Invalid JSON format for tools",
                Some(json!({
                    "error": e.to_string(),
                    "expected": "JSON array of tool call objects",
                    "example": "[{\"tool_name\": \"click_element\", \"arguments\": {\"selector\": \"#button\"}}]"
                })),
            ))?;

        // Parse the JSON values into ToolCall structs
        let mut parsed_tools = Vec::new();
        for (index, json_value) in tools_array.iter().enumerate() {
            match serde_json::from_value::<ToolCall>(json_value.clone()) {
                Ok(tool_call) => parsed_tools.push(tool_call),
                Err(e) => {
                    return Err(McpError::invalid_params(
                        "Invalid tool call format in sequence",
                        Some(json!({
                            "error": e.to_string(),
                            "index": index,
                            "expected_format": {
                                "tool_name": "string",
                                "arguments": "object",
                                "continue_on_error": "optional bool",
                                "delay_ms": "optional number"
                            }
                        })),
                    ))
                }
            }
        }

        let total_steps = parsed_tools.len() as u32;

        // Send initial progress notification if we have a token
        if let Some(ref token) = progress_token {
            let progress_notification = ProgressNotification {
                method: rmcp::model::ProgressNotificationMethod,
                params: ProgressNotificationParam {
                    progress_token: token.clone(),
                    progress: 0,
                    total: Some(total_steps),
                    message: Some(format!("Starting execution of {} steps", total_steps)),
                },
                extensions: Default::default(),
            };
            let _ = peer.send_notification(progress_notification.into()).await;
        }

        let mut results = Vec::new();
        let mut has_error = false;
        let start_time = chrono::Utc::now();

        // Create initial execution plan
        let execution_plan = json!({
            "total_steps": parsed_tools.len(),
            "steps": parsed_tools.iter().enumerate().map(|(i, tool)| {
                json!({
                    "step": i + 1,
                    "tool_name": tool.tool_name.clone(),
                    "description": self.generate_step_description(&tool.tool_name, &tool.arguments),
                    "status": "pending"
                })
            }).collect::<Vec<_>>()
        });

        // Include the execution plan in the initial response
        let mut step_results = Vec::new();

        for (index, tool_call) in parsed_tools.iter().enumerate() {
            let tool_start_time = chrono::Utc::now();
            let step_number = index + 1;
            let current_progress = step_number as u32;

            // Send progress notification for starting this step
            if let Some(ref token) = progress_token {
                let progress_notification = ProgressNotification {
                    method: rmcp::model::ProgressNotificationMethod,
                    params: ProgressNotificationParam {
                        progress_token: token.clone(),
                        progress: current_progress - 1, // Before execution
                        total: Some(total_steps),
                        message: Some(format!(
                            "Executing step {}/{}: {}",
                            step_number, total_steps, tool_call.tool_name
                        )),
                    },
                    extensions: Default::default(),
                };
                let _ = peer.send_notification(progress_notification.into()).await;
            }

            // Create step execution start info
            let step_info = json!({
                "step": step_number,
                "tool_name": tool_call.tool_name.clone(),
                "started_at": tool_start_time.to_rfc3339(),
                "status": "executing"
            });

            // Strip the mcp_terminator-mcp-agent_ prefix if present
            let tool_name = tool_call
                .tool_name
                .strip_prefix("mcp_terminator-mcp-agent_")
                .unwrap_or(&tool_call.tool_name);

            // Manually dispatch to the appropriate tool
            let tool_result = match tool_name {
                "get_window_tree" => {
                    match serde_json::from_value::<GetWindowTreeArgs>(tool_call.arguments.clone()) {
                        Ok(args) => self.get_window_tree(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for get_window_tree",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "get_focused_window_tree" => {
                    match serde_json::from_value::<GetFocusedWindowTreeArgs>(
                        tool_call.arguments.clone(),
                    ) {
                        Ok(args) => self.get_focused_window_tree(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for get_focused_window_tree",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "get_applications" => {
                    match serde_json::from_value::<GetApplicationsArgs>(tool_call.arguments.clone())
                    {
                        Ok(args) => self.get_applications(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for get_applications",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "click_element" => {
                    match serde_json::from_value::<ClickElementArgs>(tool_call.arguments.clone()) {
                        Ok(args) => self.click_element(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for click_element",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "type_into_element" => {
                    match serde_json::from_value::<TypeIntoElementArgs>(tool_call.arguments.clone())
                    {
                        Ok(args) => self.type_into_element(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for type_into_element",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "press_key" => {
                    match serde_json::from_value::<PressKeyArgs>(tool_call.arguments.clone()) {
                        Ok(args) => self.press_key(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for press_key",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "press_key_global" => {
                    match serde_json::from_value::<GlobalKeyArgs>(tool_call.arguments.clone()) {
                        Ok(args) => self.press_key_global(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for press_key_global",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "validate_element" => {
                    match serde_json::from_value::<ValidateElementArgs>(tool_call.arguments.clone())
                    {
                        Ok(args) => self.validate_element(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for validate_element",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "wait_for_element" => {
                    match serde_json::from_value::<WaitForElementArgs>(tool_call.arguments.clone())
                    {
                        Ok(args) => self.wait_for_element(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for wait_for_element",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "activate_element" => {
                    match serde_json::from_value::<ActivateElementArgs>(tool_call.arguments.clone())
                    {
                        Ok(args) => self.activate_element(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for activate_element",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "navigate_browser" => {
                    match serde_json::from_value::<NavigateBrowserArgs>(tool_call.arguments.clone())
                    {
                        Ok(args) => self.navigate_browser(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for navigate_browser",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "open_application" => {
                    match serde_json::from_value::<OpenApplicationArgs>(tool_call.arguments.clone())
                    {
                        Ok(args) => self.open_application(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for open_application",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "scroll_element" => {
                    match serde_json::from_value::<ScrollElementArgs>(tool_call.arguments.clone()) {
                        Ok(args) => self.scroll_element(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for scroll_element",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "set_clipboard" => {
                    match serde_json::from_value::<ClipboardArgs>(tool_call.arguments.clone()) {
                        Ok(args) => self.set_clipboard(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for set_clipboard",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "get_clipboard" => {
                    match serde_json::from_value::<GetClipboardArgs>(tool_call.arguments.clone()) {
                        Ok(args) => self.get_clipboard(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for get_clipboard",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "delay" => match serde_json::from_value::<DelayArgs>(tool_call.arguments.clone()) {
                    Ok(args) => self.delay(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for delay",
                        Some(json!({"error": e.to_string()})),
                    )),
                },
                "get_windows_for_application" => {
                    match serde_json::from_value::<GetWindowsArgs>(tool_call.arguments.clone()) {
                        Ok(args) => self.get_windows_for_application(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for get_windows_for_application",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "run_command" => {
                    match serde_json::from_value::<RunCommandArgs>(tool_call.arguments.clone()) {
                        Ok(args) => self.run_command(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for run_command",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "capture_screen" => {
                    match serde_json::from_value::<EmptyArgs>(tool_call.arguments.clone()) {
                        Ok(args) => self.capture_screen(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for capture_screen",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "mouse_drag" => {
                    match serde_json::from_value::<MouseDragArgs>(tool_call.arguments.clone()) {
                        Ok(args) => self.mouse_drag(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for mouse_drag",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "highlight_element" => {
                    match serde_json::from_value::<HighlightElementArgs>(
                        tool_call.arguments.clone(),
                    ) {
                        Ok(args) => self.highlight_element(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for highlight_element",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "close_element" => {
                    match serde_json::from_value::<LocatorArgs>(tool_call.arguments.clone()) {
                        Ok(args) => self.close_element(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for close_element",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "select_option" => {
                    match serde_json::from_value::<SelectOptionArgs>(tool_call.arguments.clone()) {
                        Ok(args) => self.select_option(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for select_option",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "list_options" => {
                    match serde_json::from_value::<LocatorArgs>(tool_call.arguments.clone()) {
                        Ok(args) => self.list_options(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for list_options",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "set_toggled" => {
                    match serde_json::from_value::<SetToggledArgs>(tool_call.arguments.clone()) {
                        Ok(args) => self.set_toggled(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for set_toggled",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "set_range_value" => {
                    match serde_json::from_value::<SetRangeValueArgs>(tool_call.arguments.clone()) {
                        Ok(args) => self.set_range_value(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for set_range_value",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "set_selected" => {
                    match serde_json::from_value::<SetSelectedArgs>(tool_call.arguments.clone()) {
                        Ok(args) => self.set_selected(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for set_selected",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "is_toggled" => {
                    match serde_json::from_value::<LocatorArgs>(tool_call.arguments.clone()) {
                        Ok(args) => self.is_toggled(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for is_toggled",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "get_range_value" => {
                    match serde_json::from_value::<LocatorArgs>(tool_call.arguments.clone()) {
                        Ok(args) => self.get_range_value(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for get_range_value",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "is_selected" => {
                    match serde_json::from_value::<LocatorArgs>(tool_call.arguments.clone()) {
                        Ok(args) => self.is_selected(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for is_selected",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "capture_element_screenshot" => {
                    match serde_json::from_value::<ValidateElementArgs>(tool_call.arguments.clone())
                    {
                        Ok(args) => self.capture_element_screenshot(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for capture_element_screenshot",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                "invoke_element" => {
                    match serde_json::from_value::<LocatorArgs>(tool_call.arguments.clone()) {
                        Ok(args) => self.invoke_element(Parameters(args)).await,
                        Err(e) => Err(McpError::invalid_params(
                            "Invalid arguments for invoke_element",
                            Some(json!({"error": e.to_string()})),
                        )),
                    }
                }
                _ => Err(McpError::internal_error(
                    "Unknown tool called",
                    Some(json!({"tool_name": tool_call.tool_name, "stripped_name": tool_name})),
                )),
            };

            // Process the result
            let processed_result = match tool_result {
                Ok(result) => {
                    // Extract actual content from the result
                    let mut extracted_content = Vec::new();

                    for content in &result.content {
                        // Try to extract the content as JSON since most tool results are JSON
                        if let Ok(json_content) = serde_json::to_value(content) {
                            extracted_content.push(json_content);
                        } else {
                            // Fallback to a generic representation
                            extracted_content.push(json!({
                                "type": "unknown",
                                "data": "Content extraction failed"
                            }));
                        }
                    }

                    let content_summary = if include_detailed {
                        json!({
                            "type": "tool_result",
                            "content_count": result.content.len(),
                            "content": extracted_content
                        })
                    } else {
                        json!({
                            "type": "summary",
                            "content": "Tool executed successfully",
                            "content_count": result.content.len()
                        })
                    };

                    let step_result = json!({
                        "step": step_number,
                        "tool_name": tool_call.tool_name,
                        "status": "success",
                        "started_at": tool_start_time.to_rfc3339(),
                        "completed_at": chrono::Utc::now().to_rfc3339(),
                        "duration_ms": (chrono::Utc::now() - tool_start_time).num_milliseconds(),
                        "result": content_summary,
                        "progress": format!("{}/{}", step_number, parsed_tools.len())
                    });

                    step_results.push(step_result.clone());
                    step_result
                }
                Err(e) => {
                    has_error = true;

                    let error_result = json!({
                        "step": step_number,
                        "tool_name": tool_call.tool_name,
                        "status": "error",
                        "started_at": tool_start_time.to_rfc3339(),
                        "completed_at": chrono::Utc::now().to_rfc3339(),
                        "duration_ms": (chrono::Utc::now() - tool_start_time).num_milliseconds(),
                        "error": e.to_string(),
                        "progress": format!("{}/{}", step_number, parsed_tools.len())
                    });

                    step_results.push(error_result.clone());

                    // Check if we should continue on error
                    let continue_on_error = tool_call.continue_on_error.unwrap_or(false);
                    if !continue_on_error && stop_on_error {
                        results.push(error_result);
                        break; // Stop execution
                    }

                    error_result
                }
            };

            results.push(processed_result.clone());

            // Send completion progress notification
            if let Some(ref token) = progress_token {
                let status_message = if processed_result["status"] == "success" {
                    format!(
                        "Completed step {}/{}: {}",
                        step_number, total_steps, tool_call.tool_name
                    )
                } else {
                    format!(
                        "Failed step {}/{}: {} - {}",
                        step_number, total_steps, tool_call.tool_name, processed_result["error"]
                    )
                };

                let progress_notification = ProgressNotification {
                    method: rmcp::model::ProgressNotificationMethod,
                    params: ProgressNotificationParam {
                        progress_token: token.clone(),
                        progress: current_progress,
                        total: Some(total_steps),
                        message: Some(status_message),
                    },
                    extensions: Default::default(),
                };
                let _ = peer.send_notification(progress_notification.into()).await;
            }

            // Handle delay after tool execution if specified
            if let Some(delay_ms) = tool_call.delay_ms {
                if delay_ms > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
            }
        }

        let total_duration = (chrono::Utc::now() - start_time).num_milliseconds();

        // Determine overall execution status
        let overall_status = if has_error && stop_on_error {
            "partial_success"
        } else if has_error {
            "completed_with_errors"
        } else {
            "success"
        };

        // Create comprehensive execution summary
        let summary = json!({
            "action": "execute_sequence",
            "status": overall_status,
            "execution_plan": execution_plan,
            "execution_summary": {
                "total_steps": parsed_tools.len(),
                "executed_steps": results.len(),
                "successful_steps": results.iter().filter(|r| r["status"] == "success").count(),
                "failed_steps": results.iter().filter(|r| r["status"] == "error").count(),
                "total_duration_ms": total_duration,
                "started_at": start_time.to_rfc3339(),
                "completed_at": chrono::Utc::now().to_rfc3339(),
            },
            "step_results": step_results,
            "detailed_results": if include_detailed { Some(results) } else { None },
        });

        Ok(CallToolResult::success(vec![Content::json(summary)?]))
    }

    #[tool(
        description = "Exports a sequence of successful tool calls into a structured, reliable workflow format that can be executed by another AI agent with minimal context. This tool analyzes the provided sequence and enhances it with intelligent error handling, validation steps, wait conditions, and fallback strategies to maximize success rate. The output can be in JSON or YAML format and includes comprehensive metadata to ensure reproducibility."
    )]
    pub async fn export_workflow_sequence(
        &self,
        Parameters(args): Parameters<ExportWorkflowSequenceArgs>,
    ) -> Result<CallToolResult, McpError> {
        let output_format = args.output_format.clone().unwrap_or("json".to_string());
        let include_ai_fallbacks = args.include_ai_fallbacks.unwrap_or(true);
        let add_validation_steps = args.add_validation_steps.unwrap_or(true);
        let include_tree_captures = args.include_tree_captures.unwrap_or(false);

        // Parse the JSON value as an array of tool calls
        let tool_calls_array: Vec<serde_json::Value> = serde_json::from_value(args.successful_tool_calls.clone())
            .map_err(|e| McpError::invalid_params(
                "successful_tool_calls must be a JSON array",
                Some(json!({
                    "error": e.to_string(),
                    "expected": "JSON array of tool call objects",
                    "example": "[{\"tool_name\": \"click_element\", \"arguments\": {\"selector\": \"#button\"}}]"
                })),
            ))?;

        // Parse the JSON values into ToolCall structs
        let mut parsed_tool_calls = Vec::new();
        for (index, json_value) in tool_calls_array.iter().enumerate() {
            match serde_json::from_value::<ToolCall>(json_value.clone()) {
                Ok(tool_call) => parsed_tool_calls.push(tool_call),
                Err(e) => {
                    return Err(McpError::invalid_params(
                        "Invalid tool call format",
                        Some(json!({
                            "error": e.to_string(),
                            "index": index,
                            "expected_format": {
                                "tool_name": "string",
                                "arguments": "object",
                                "continue_on_error": "optional bool",
                                "delay_ms": "optional number"
                            }
                        })),
                    ))
                }
            }
        }

        // Build the workflow steps with enhancements
        let mut enhanced_steps = Vec::new();
        let mut step_counter = 1;

        for (index, tool_call) in parsed_tool_calls.iter().enumerate() {
            // Analyze the tool to determine what enhancements to add
            let tool_name = &tool_call.tool_name;

            // Add focus check before UI interaction tools
            if matches!(
                tool_name.as_str(),
                "click_element"
                    | "type_into_element"
                    | "press_key"
                    | "invoke_element"
                    | "select_option"
            ) && (index == 0 || should_add_focus_check(&parsed_tool_calls, index))
            {
                enhanced_steps.push(json!({
                    "step": step_counter,
                    "action": "validate_focus",
                    "description": "Ensure the target application has focus",
                    "tool_name": "get_applications",
                    "condition": "Check if target app is_focused=true",
                    "fallback": "Use activate_element if not focused"
                }));
                step_counter += 1;
            }

            // Add wait after navigation or state-changing actions
            if matches!(tool_name.as_str(), "navigate_browser" | "open_application") {
                enhanced_steps.push(json!({
                    "step": step_counter,
                    "action": tool_name,
                    "description": tool_call.arguments.get("description").and_then(|v| v.as_str()).unwrap_or("Execute action"),
                    "tool_name": tool_name,
                    "arguments": tool_call.arguments.clone(),
                    "success_criteria": "Page/App loads successfully"
                }));
                step_counter += 1;

                // Add intelligent wait
                enhanced_steps.push(json!({
                    "step": step_counter,
                    "action": "wait_for_stability",
                    "description": "Wait for UI to stabilize after navigation",
                    "tool_name": "wait_for_element",
                    "arguments": {
                        "selector": "role:Document",
                        "condition": "exists",
                        "timeout_ms": 5000
                    },
                    "fallback": "If timeout, check get_window_tree for current state"
                }));
                step_counter += 1;
            } else {
                // Process the actual tool call with enhancements
                let mut enhanced_args = tool_call.arguments.clone();

                // Extract selectors and add alternatives if available
                if let Some(_selector) =
                    tool_call.arguments.get("selector").and_then(|v| v.as_str())
                {
                    // Look for alternative selectors from the arguments
                    if let Some(alternatives) = tool_call.arguments.get("alternative_selectors") {
                        enhanced_args["alternative_selectors"] = alternatives.clone();
                    }
                }

                enhanced_steps.push(json!({
                    "step": step_counter,
                    "action": tool_name,
                    "description": self.generate_step_description(tool_name, &tool_call.arguments),
                    "tool_name": tool_name,
                    "arguments": enhanced_args,
                    "wait_for": self.get_wait_condition(tool_name),
                    "verify_success": add_validation_steps
                }));
                step_counter += 1;
            }

            // Add validation after state-changing actions if requested
            if add_validation_steps && is_state_changing_action(tool_name) {
                if let Some(selector) = tool_call.arguments.get("selector") {
                    enhanced_steps.push(json!({
                        "step": step_counter,
                        "action": "validate_action_result",
                        "description": format!("Verify {} completed successfully", tool_name),
                        "tool_name": "validate_element",
                        "arguments": {
                            "selector": selector,
                            "timeout_ms": 1000
                        },
                        "condition": "Element still exists and state changed as expected"
                    }));
                    step_counter += 1;
                }
            }

            // Add tree capture at key points if requested
            if include_tree_captures
                && should_capture_tree(tool_name, index, parsed_tool_calls.len())
            {
                enhanced_steps.push(json!({
                    "step": step_counter,
                    "action": "capture_ui_state",
                    "description": "Capture UI tree for debugging/verification",
                    "tool_name": "get_window_tree",
                    "arguments": {
                        "include_tree": true
                    },
                    "purpose": "State checkpoint for recovery"
                }));
                step_counter += 1;
            }
        }

        // Build the complete workflow structure
        let workflow = json!({
            "workflow": {
                "name": args.workflow_name,
                "version": "1.0",
                "description": args.workflow_description,
                "goal": args.workflow_goal,
                "created_at": chrono::Utc::now().to_rfc3339(),
                "created_by": "terminator-mcp-agent",

                "prerequisites": {
                    "browser": "Chrome",
                    "platform": env::consts::OS,
                    "required_tools": self.extract_required_tools(&parsed_tool_calls)
                },

                "parameters": {
                    "credentials": args.credentials.unwrap_or(json!({})),
                    "form_data": args.expected_data.unwrap_or(json!({}))
                },

                "configuration": {
                    "include_ai_fallbacks": include_ai_fallbacks,
                    "add_validation_steps": add_validation_steps,
                    "default_timeout_ms": 3000,
                    "retry_on_failure": true,
                    "max_retries": 2
                },

                "steps": enhanced_steps,

                "error_handling": {
                    "known_errors": args.known_error_handlers.unwrap_or(json!([])),
                    "general_strategies": [
                        {
                            "error": "ElementNotFound",
                            "solution": "Call get_window_tree to refresh UI state, then retry with alternative selectors"
                        },
                        {
                            "error": "ElementDisabled",
                            "solution": "Check prerequisites - ensure all required fields are filled and conditions met"
                        },
                        {
                            "error": "Timeout",
                            "solution": "Increase timeout_ms or add explicit wait_for_element steps"
                        }
                    ]
                },

                "success_criteria": {
                    "final_validation": "Verify the workflow goal has been achieved",
                    "expected_outcomes": self.infer_expected_outcomes(&parsed_tool_calls),
                    "verification_steps": if add_validation_steps {
                        vec!["Check final UI state matches expected", "Verify data was processed correctly"]
                    } else {
                        vec![]
                    }
                },

                "ai_decision_points": if include_ai_fallbacks {
                    json!([
                        {
                            "condition": "Dialog or popup appears unexpectedly",
                            "action": "Analyze dialog content and decide whether to accept, cancel, or handle differently"
                        },
                        {
                            "condition": "Expected element not found after multiple retries",
                            "action": "Use get_window_tree to understand current state and find alternative path"
                        },
                        {
                            "condition": "Form validation errors",
                            "action": "Read error messages and adjust input data accordingly"
                        }
                    ])
                } else {
                    json!([])
                },

                "notes": [
                    "This workflow was automatically generated from successful tool executions",
                    "Selectors use exact IDs where possible for maximum reliability",
                    "Alternative selectors are included for robustness",
                    "Wait conditions and validations ensure each step completes before proceeding"
                ]
            }
        });

        // Convert to requested format
        let output = match output_format.to_lowercase().as_str() {
            "yaml" => {
                // For YAML output, we'll return instructions since we can't directly convert
                json!({
                    "format": "yaml",
                    "content": workflow,
                    "note": "Copy the 'content' field and convert to YAML using a JSON-to-YAML converter for proper formatting"
                })
            }
            _ => workflow,
        };

        Ok(CallToolResult::success(vec![Content::json(output)?]))
    }

    // Helper methods for export_workflow_sequence
    fn generate_step_description(&self, tool_name: &str, args: &serde_json::Value) -> String {
        match tool_name {
            "click_element" => format!(
                "Click on element: {}",
                args.get("selector")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
            ),
            "type_into_element" => format!(
                "Type '{}' into {}",
                args.get("text_to_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or(""),
                args.get("selector")
                    .and_then(|v| v.as_str())
                    .unwrap_or("field")
            ),
            "navigate_browser" => format!(
                "Navigate to {}",
                args.get("url").and_then(|v| v.as_str()).unwrap_or("URL")
            ),
            "select_option" => format!(
                "Select '{}' from dropdown",
                args.get("option_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("option")
            ),
            _ => format!("Execute {}", tool_name),
        }
    }

    fn get_wait_condition(&self, tool_name: &str) -> Option<String> {
        match tool_name {
            "click_element" => Some("Element state changes or UI updates".to_string()),
            "type_into_element" => Some("Text appears in field".to_string()),
            "navigate_browser" => Some("Page loads completely".to_string()),
            "open_application" => Some("Application window appears".to_string()),
            _ => None,
        }
    }

    fn extract_required_tools(&self, tool_calls: &[crate::utils::ToolCall]) -> Vec<String> {
        tool_calls
            .iter()
            .map(|tc| tc.tool_name.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect()
    }

    fn infer_expected_outcomes(&self, tool_calls: &[crate::utils::ToolCall]) -> Vec<String> {
        let mut outcomes = Vec::new();

        for call in tool_calls {
            match call.tool_name.as_str() {
                "navigate_browser" => {
                    outcomes.push("Target webpage loaded successfully".to_string())
                }
                "type_into_element" => outcomes.push("Form fields populated with data".to_string()),
                "click_element"
                    if call
                        .arguments
                        .get("selector")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .contains("Submit") =>
                {
                    outcomes.push("Form submitted successfully".to_string())
                }
                "select_option" => outcomes.push("Option selected in dropdown".to_string()),
                _ => {}
            }
        }

        outcomes
    }

    // Helper to optionally attach UI tree to response
    fn maybe_attach_tree(&self, include_tree: bool, pid_opt: Option<u32>, result_json: &mut Value) {
        if !include_tree {
            return;
        }
        if let Some(pid) = pid_opt {
            if let Ok(tree) = self.desktop.get_window_tree(pid, None, None) {
                if let Ok(tree_val) = serde_json::to_value(tree) {
                    if let Some(obj) = result_json.as_object_mut() {
                        obj.insert("ui_tree".to_string(), tree_val);
                    }
                }
            }
        }
    }
}

// Helper functions for export_workflow_sequence
fn should_add_focus_check(tool_calls: &[crate::utils::ToolCall], current_index: usize) -> bool {
    // Add focus check if:
    // 1. It's the first UI interaction
    // 2. Previous action was navigation or opened a new window
    // 3. There was a significant gap (e.g., after get_window_tree or wait)

    if current_index == 0 {
        return true;
    }

    let prev_tool = &tool_calls[current_index - 1].tool_name;
    matches!(
        prev_tool.as_str(),
        "navigate_browser"
            | "open_application"
            | "close_element"
            | "get_window_tree"
            | "get_applications"
            | "activate_element"
    )
}

fn is_state_changing_action(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "click_element"
            | "type_into_element"
            | "select_option"
            | "set_toggled"
            | "set_selected"
            | "set_range_value"
            | "invoke_element"
            | "press_key"
            | "mouse_drag"
            | "scroll_element"
    )
}

fn should_capture_tree(tool_name: &str, index: usize, total_steps: usize) -> bool {
    // Capture tree at key points:
    // 1. After major navigation
    // 2. Before complex sequences
    // 3. At regular intervals (every 5 steps)
    // 4. Before the final action

    matches!(tool_name, "navigate_browser" | "open_application")
        || index % 5 == 0
        || index == total_steps - 1
}

#[tool_handler]
impl ServerHandler for DesktopWrapper {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::LATEST,
            capabilities: ServerCapabilities::builder()
                .enable_prompts()
                .enable_resources()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(get_server_instructions().to_string()),
        }
    }
}

fn get_server_instructions() -> String {
    let current_date_time = Local::now().to_string();
    let current_os = env::consts::OS;
    let current_working_dir = env::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    format!(
        "
You are an AI assistant designed to control a computer desktop. Your primary goal is to understand the user's request and translate it into a sequence of tool calls to automate GUI interactions.

**Golden Rules for Robust Automation**

1.  **CHECK FOCUS FIRST:** Before any `click`, `type`, or `press_key` action, you **MUST** verify the target application `is_focused` using `get_applications`. If it's not, you **MUST** call `activate_element` before proceeding. This is the #1 way to prevent sending commands to the wrong window.

2.  **AVOID STALE STATE & CONTEXT COLLAPSE:** After any action that changes the UI context (closing a dialog, getting an error, a click that loads new content), the UI may have changed dramatically. **You MUST call `get_window_tree` again to get the current, fresh state before proceeding.** Failure to do so will cause you to act on a 'ghost' UI and fail. Do not trust a 'success' status alone; verify the outcome.

3.  **WAIT AFTER NAVIGATION:** After actions like `click_element` on a link or `navigate_browser`, the UI needs time to load. You **MUST** explicitly wait. The best method is to use `wait_for_element` targeting a known element on the new page. Do not call `get_window_tree` immediately.

4.  **CHECK BEFORE YOU ACT (Especially Toggles):** Before clicking a checkbox, radio button, or any toggleable item, **ALWAYS** use `is_toggled` or `is_selected` to check its current state. Only click if it's not already in the desired state to avoid accidentally undoing the action.

5.  **HANDLE DISABLED ELEMENTS:** Before attempting to click a button or interact with an element, you **MUST** check if it is enabled. The `validate_element` and `get_window_tree` tools return an `enabled` property. If an element is disabled (e.g., a grayed-out 'Submit' button), do not try to click it. Instead, you must investigate the UI to figure out why it's disabled. Look for unchecked checkboxes, empty required fields, or other dependencies that must be satisfied first.

6.  **USE PRECISE SELECTORS (ID IS YOUR FRIEND):** A `role|name` selector is good, but often, an element **does not have a `name` attribute** even if it contains visible text (the text is often a child element). Check the `get_window_tree` output carefully. If an element has an empty or generic name, you **MUST use its numeric ID (`\"#12345\"`) for selection.** Do not guess or hallucinate a `name` from the visual text; use the ID. This is critical for clickable `Group` elements which often lack a name.

7.  **PREFER INVOKE OVER CLICK FOR BUTTONS:** When dealing with buttons, especially those that might not be in the viewport, **prefer `invoke_element` over `click_element`**. The `invoke_element` action is more reliable because it doesn't require the element to be scrolled into view. Use `click_element` only when you specifically need mouse interaction behavior (e.g., for links or UI elements that respond differently to clicks).

8.  **USE SET_SELECTED FOR RADIO BUTTONS AND CHECKBOXES:** For radio buttons and selectable items, **always use `set_selected` with `state: true`** instead of `click_element`. This ensures the element reaches the desired state regardless of its current state. For checkboxes and toggle switches, use `set_toggled` with the desired state.


**Tool Behavior & Metadata**

Pay close attention to the tool descriptions for hints on their behavior.

*   **Read-only tools** are safe to use for inspection and will not change the UI state (e.g., `validate_element`, `get_window_tree`).
*   Tools that **may change the UI** require more care. After using one, consider calling `get_window_tree` again to get the latest UI state.
*   Tools that **require focus** must only be used on the foreground application. Use `get_applications` to check focus and `activate_element` to bring an application to the front.

**Core Workflow: Discover, then Act with Precision**

Your most reliable strategy is to inspect the application's UI structure *before* trying to interact with it. Never guess selectors.

1.  **Discover Running Applications:** Use `get_applications` to see what's running. This gives you the `name`, `id`, and `pid` (Process ID) for each application.

2.  **Get the UI Tree:** This is the most important step. Once you have the `pid` of your target application, call `get_window_tree` with `include_tree: true`. This returns a complete, JSON-like structure of all UI elements in that application.

3.  **Construct Smart Selector Strategies:** 
    *   **Primary Strategy:** Use `role:Type|name:Name` when available, otherwise use the numeric ID (`\"#12345\"`).
    *   **Multi-Selector Fallbacks:** Provide alternatives that are tried in parallel:
        ```json
        {{
          \"selector\": \"role:Button|name:Submit\",
          \"alternative_selectors\": \"#12345\"
        }}
        ```
    *   **Avoid:** Generic selectors like `\"role:Button\"` alone - they're too ambiguous.

**Action Examples**

*   **Invoking a button (preferred over clicking):**
    ```json
    {{
        \"tool_name\": \"invoke_element\",
        \"args\": {{\"selector\": \"role:button|name:Login\"}}
    }}
    ```
*   **Selecting a radio button (use set_selected, not click):**
    ```json
    {{
        \"tool_name\": \"set_selected\",
        \"args\": {{\"selector\": \"role:RadioButton|name:Male\", \"state\": true}}
    }}
    ```
*   **Typing an email into an email field:**
    ```json
    {{
        \"tool_name\": \"type_into_element\",
        \"args\": {{\"selector\": \"edit|Email\", \"text_to_type\": \"user@example.com\"}}
    }}
    ```
*   **Using alternative selectors for robustness:**
    ```json
    {{
        \"tool_name\": \"invoke_element\",
        \"args\": {{
            \"selector\": \"#17517999067772859239\",
            \"alternative_selectors\": \"role:Group|name:Run Quote\"
        }}
    }}
    ```

**Common Pitfalls & Solutions**

*   **Click fails on buttons not in viewport:** Use `invoke_element` instead of `click_element`.
*   **Radio button clicks don't register:** Use `set_selected` with `state: true`.
*   **Form validation errors:** Verify all fields AND radio buttons/checkboxes before submitting.
*   **Element not found after UI change:** Call `get_window_tree` again after UI changes.
*   **Selector matches wrong element:** Use numeric ID when name is empty.

Contextual information:
- The current date and time is {}.
- Current operating system: {}.
- Current working directory: {}.
",
        current_date_time, current_os, current_working_dir
    )
}
