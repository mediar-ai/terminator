use crate::expression_eval;
use crate::helpers::*;
use crate::output_parser;
use crate::scripting_engine;
use crate::utils::find_and_execute_with_retry_with_fallback;
pub use crate::utils::DesktopWrapper;
use crate::utils::WaitForOutputParserArgs;
use crate::utils::{
    get_timeout, ActivateElementArgs, ClickElementArgs, CloseElementArgs, DelayArgs,
    ExecuteSequenceArgs, ExportWorkflowSequenceArgs, GetApplicationsArgs, GetFocusedWindowTreeArgs,
    GetWindowTreeArgs, GlobalKeyArgs, HighlightElementArgs, LocatorArgs, MaximizeWindowArgs,
    MinimizeWindowArgs, MouseDragArgs, NavigateBrowserArgs, OpenApplicationArgs, PressKeyArgs,
    RecordWorkflowArgs, RunCommandArgs, RunJavascriptArgs, ScrollElementArgs, SelectOptionArgs,
    SetRangeValueArgs, SetSelectedArgs, SetToggledArgs, SetValueArgs, SetZoomArgs,
    TypeIntoElementArgs, ValidateElementArgs, WaitForElementArgs, ZoomArgs,
};
use image::{ExtendedColorType, ImageEncoder};
use rmcp::handler::server::tool::Parameters;
use rmcp::model::{
    CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
};
use rmcp::{tool, Error as McpError, ServerHandler};
use rmcp::{tool_handler, tool_router};
use serde_json::{json, Value};
use std::future::Future;
use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;
use futures;
use terminator::{Browser, Desktop, Selector, UIElement};
use terminator_workflow_recorder::{PerformanceMode, WorkflowRecorder, WorkflowRecorderConfig};
use tokio::sync::Mutex;
use tracing::{info, warn};

// New imports for image encoding
use base64::{engine::general_purpose, Engine as _};
use image::codecs::png::PngEncoder;

/// Extracts JSON data from Content objects without double serialization
pub fn extract_content_json(content: &Content) -> Result<serde_json::Value, serde_json::Error> {
    // First serialize the Content to understand its structure
    let content_value = serde_json::to_value(content)?;

    // Check if it's a JSON content type with a "text" field containing JSON string
    if let Some(content_obj) = content_value.as_object() {
        if let Some(content_type) = content_obj.get("type") {
            if content_type == "text" {
                if let Some(text_value) = content_obj.get("text") {
                    if let Some(text_str) = text_value.as_str() {
                        // Try to parse the text as JSON
                        if let Ok(parsed_json) = serde_json::from_str::<serde_json::Value>(text_str)
                        {
                            return Ok(parsed_json);
                        }
                    }
                }
            }
        }
    }

    // Fallback: return the content as-is if it's not a JSON text type
    Ok(content_value)
}

// Helper function to validate extracted data against success criteria
fn is_valid_extraction(extracted_data: &serde_json::Value, criteria: &serde_json::Value) -> bool {
    // Default validation: check if we have any data
    if extracted_data.is_null() {
        return false;
    }

    // If criteria is provided, validate against it
    if let Some(min_items) = criteria.get("min_items").and_then(|v| v.as_u64()) {
        let item_count = extracted_data.as_array().map_or(0, |arr| arr.len());
        if (item_count as u64) < min_items {
            return false;
        }
    }

    // Check for required fields
    if let Some(required_fields) = criteria.get("required_fields").and_then(|v| v.as_array()) {
        if let Some(data_array) = extracted_data.as_array() {
            if data_array.is_empty() {
                return false;
            }

            // Check if at least one item has all required fields
            let has_required_fields = data_array.iter().any(|item| {
                if let Some(item_obj) = item.as_object() {
                    required_fields.iter().all(|field| {
                        if let Some(field_name) = field.as_str() {
                            item_obj.contains_key(field_name)
                                && !item_obj.get(field_name).unwrap().is_null()
                        } else {
                            false
                        }
                    })
                } else {
                    false
                }
            });

            if !has_required_fields {
                return false;
            }
        }
    }

    true
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
            recorder: Arc::new(Mutex::new(None)),
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
                    format!("#{app_id}")
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

    #[tool(
        description = "Types text into a UI element with smart clipboard optimization and verification. Much faster than press key. This action requires the application to be focused and may change the UI."
    )]
    async fn type_into_element(
        &self,
        Parameters(args): Parameters<TypeIntoElementArgs>,
    ) -> Result<CallToolResult, McpError> {
        let text_to_type = args.text_to_type.clone();
        let should_clear = args.clear_before_typing.unwrap_or(true);

        let action = move |element: UIElement| {
            let text_to_type = text_to_type.clone();
            async move {
                if should_clear {
                    if let Err(clear_error) = element.set_value("") {
                        warn!(
                            "Warning: Failed to clear element before typing: {}",
                            clear_error
                        );
                    }
                }
                element.type_text(&text_to_type, true)
            }
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

        let mut result_json = json!({
            "action": "type_into_element",
            "status": "success",
            "text_typed": args.text_to_type,
            "cleared_before_typing": args.clear_before_typing.unwrap_or(true),
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
            args.include_tree.unwrap_or(true),
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
        let ((_click_result, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector,
                args.alternative_selectors.as_deref(),
                args.fallback_selectors.as_deref(),
                args.timeout_ms,
                args.retries,
                |element| async move { element.click() },
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
        let original_element_id = element.id_or_empty();

        // --- Action Consequence Verification ---
        let consequence = wait_for_ui_change(
            &self.desktop,
            &original_element_id,
            std::time::Duration::from_millis(300),
        )
        .await;

        // Build base result
        let mut result_json = json!({
            "action": "click",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector, args.alternative_selectors.as_deref(), args.fallback_selectors.as_deref()),
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "consequence": consequence
        });

        // Always attach tree for better context
        maybe_attach_tree(
            &self.desktop,
            args.include_tree.unwrap_or(true),
            element.process_id().ok(),
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
        let key_to_press = args.key.clone();
        let action = move |element: UIElement| {
            let key_to_press = key_to_press.clone();
            async move { element.press_key(&key_to_press) }
        };

        let ((_result, element), successful_selector) =
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
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector, None, args.fallback_selectors.as_deref()),
            "timestamp": chrono::Utc::now().to_rfc3339()
        });
        maybe_attach_tree(
            &self.desktop,
            true, // press_key_global does not have include_tree option
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
            true, // press_key_global does not have include_tree option
            element.process_id().ok(),
            &mut result_json,
        );

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
            args.include_tree.unwrap_or(true),
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
            args.include_tree.unwrap_or(true),
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
                    args.include_tree.unwrap_or(true),
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

        let action = |element: UIElement| async move { element.highlight(color, duration) };

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
            "action": "highlight_element",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector, args.alternative_selectors.as_deref(), args.fallback_selectors.as_deref()),
            "color": args.color.unwrap_or(0x0000FF),
            "duration_ms": args.duration_ms.unwrap_or(1000),
            "timestamp": chrono::Utc::now().to_rfc3339()
        });
        maybe_attach_tree(
            &self.desktop,
            args.include_tree.unwrap_or(true),
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
                        args.include_tree.unwrap_or(false),
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
                            args.include_tree.unwrap_or(false),
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
        description = "Waits for output parser to successfully extract data from UI tree, polling until success or timeout. This is useful for waiting for dynamic content to load and be ready for data extraction."
    )]
    async fn wait_for_output_parser(
        &self,
        Parameters(args): Parameters<WaitForOutputParserArgs>,
    ) -> Result<CallToolResult, McpError> {
        let start_time = std::time::Instant::now();
        let timeout_duration = Duration::from_millis(args.timeout_ms.unwrap_or(10000));
        let poll_interval = Duration::from_millis(args.poll_interval_ms.unwrap_or(1000));

        info!(
            "[wait_for_output_parser] Starting with timeout: {:?}, poll_interval: {:?}",
            timeout_duration, poll_interval
        );

        loop {
            // Check timeout
            if start_time.elapsed() > timeout_duration {
                return Err(McpError::internal_error(
                    "Timeout waiting for output parser to extract data",
                    Some(json!({
                        "timeout_ms": args.timeout_ms.unwrap_or(10000),
                        "elapsed_ms": start_time.elapsed().as_millis(),
                        "poll_interval_ms": args.poll_interval_ms.unwrap_or(1000)
                    })),
                ));
            }

            // Get current UI tree
            let tree = if let Some(selector) = &args.selector {
                // Get tree from specific element
                info!(
                    "[wait_for_output_parser] Getting UI tree from selector: '{}'",
                    selector
                );
                let locator = self
                    .desktop
                    .locator(terminator::Selector::from(selector.as_str()));
                match locator.wait(Some(Duration::from_millis(500))).await {
                    Ok(element) => {
                        let pid = element.process_id().unwrap_or(0);
                        self.desktop.get_window_tree(pid, None, None).ok()
                    }
                    Err(_) => {
                        info!(
                            "[wait_for_output_parser] Failed to find element with selector: '{}'",
                            selector
                        );
                        None
                    }
                }
            } else {
                // Get tree from focused window
                info!("[wait_for_output_parser] Getting UI tree from focused window");
                match self.desktop.focused_element() {
                    Ok(element) => match element.process_id() {
                        Ok(pid) => self.desktop.get_window_tree(pid, None, None).ok(),
                        Err(_) => None,
                    },
                    Err(_) => None,
                }
            };

            if let Some(tree) = tree {
                // Try to run output parser
                let tree_json = json!({ "ui_tree": tree });

                match output_parser::run_output_parser(&args.output_parser, &tree_json).await {
                    Ok(Some(parsed_data)) => {
                        // Check if we got the expected data
                        let is_valid = if let Some(criteria) = &args.success_criteria {
                            is_valid_extraction(&parsed_data, criteria)
                        } else {
                            // Default validation: check if we got any data at all
                            !parsed_data.is_null()
                                && parsed_data.as_array().is_none_or(|arr| !arr.is_empty())
                        };

                        if is_valid {
                            info!(
                                "[wait_for_output_parser] Successfully extracted data after {}ms",
                                start_time.elapsed().as_millis()
                            );

                            let mut result_json = json!({
                                "action": "wait_for_output_parser",
                                "status": "success",
                                "elapsed_ms": start_time.elapsed().as_millis(),
                                "extracted_data": parsed_data,
                                "data_item_count": parsed_data.as_array().map_or(0, |arr| arr.len()),
                                "timestamp": chrono::Utc::now().to_rfc3339()
                            });

                            // Include tree if requested
                            if args.include_tree.unwrap_or(false) {
                                if let Some(obj) = result_json.as_object_mut() {
                                    obj.insert("ui_tree".to_string(), json!(tree));
                                }
                            }

                            return Ok(CallToolResult::success(vec![Content::json(result_json)?]));
                        } else {
                            info!(
                                "[wait_for_output_parser] Parser extracted data but validation failed, continuing to poll..."
                            );
                        }
                    }
                    Ok(_) => {
                        info!(
                            "[wait_for_output_parser] Parser found no data, continuing to poll..."
                        );
                    }
                    Err(e) => {
                        info!(
                            "[wait_for_output_parser] Parser error: {}, continuing to poll...",
                            e
                        );
                    }
                }
            } else {
                info!("[wait_for_output_parser] Failed to get UI tree, continuing to poll...");
            }

            // Wait before next poll
            tokio::time::sleep(poll_interval).await;
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
            args.include_tree.unwrap_or(false),
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
        let direction = args.direction.clone();
        let amount = args.amount;
        let action = move |element: UIElement| {
            let direction = direction.clone();
            async move { element.scroll(&direction, amount) }
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
            "action": "scroll_element",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector, args.alternative_selectors.as_deref(), args.fallback_selectors.as_deref()),
            "direction": args.direction,
            "amount": args.amount,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });
        maybe_attach_tree(
            &self.desktop,
            args.include_tree.unwrap_or(true),
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
            async move { element.select_option(&option_name) }
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
            "action": "select_option",
            "status": "success",
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector, args.alternative_selectors.as_deref(), args.fallback_selectors.as_deref()),
            "option_selected": args.option_name,
        });
        maybe_attach_tree(
            &self.desktop,
            args.include_tree.unwrap_or(true),
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
            args.include_tree.unwrap_or(true),
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
        let action = move |element: UIElement| async move { element.set_toggled(state) };

        let ((_result, element), successful_selector) =
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
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector, None, args.fallback_selectors.as_deref()),
            "state_set_to": args.state,
        });
        maybe_attach_tree(
            &self.desktop,
            args.include_tree.unwrap_or(true),
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
        let action = move |element: UIElement| async move { element.set_range_value(value) };

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
            args.include_tree.unwrap_or(true),
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
        let action = move |element: UIElement| async move { element.set_selected(state) };

        let ((_result, element), successful_selector) =
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
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector, None, args.fallback_selectors.as_deref()),
            "state_set_to": args.state,
        });
        maybe_attach_tree(
            &self.desktop,
            args.include_tree.unwrap_or(true),
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
            args.include_tree.unwrap_or(true),
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
            args.include_tree.unwrap_or(true),
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
            args.include_tree.unwrap_or(true),
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
        let ((_result, element), successful_selector) =
            match find_and_execute_with_retry_with_fallback(
                &self.desktop,
                &args.selector,
                args.alternative_selectors.as_deref(),
                args.fallback_selectors.as_deref(),
                args.timeout_ms,
                args.retries,
                |element| async move { element.invoke() },
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
            "element": element_info,
            "selector_used": successful_selector,
            "selectors_tried": get_selectors_tried_all(&args.selector, args.alternative_selectors.as_deref(), args.fallback_selectors.as_deref()),
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        maybe_attach_tree(
            &self.desktop,
            args.include_tree.unwrap_or(true),
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
                    // text input completion tracking, a feature the user found caused lag. [[memory:523310]]
                    PerformanceMode::low_energy_config()
                } else {
                    WorkflowRecorderConfig::default()
                };

                let mut recorder = WorkflowRecorder::new(workflow_name.clone(), config);
                recorder.start().await.map_err(|e| {
                    McpError::internal_error(
                        "Failed to start recorder",
                        Some(json!({ "error": e.to_string() })),
                    )
                })?;

                *recorder_guard = Some(recorder);

                Ok(CallToolResult::success(vec![Content::json(json!({
                    "action": "record_workflow",
                    "status": "started",
                    "workflow_name": workflow_name,
                    "message": "Recording started. Perform the UI actions you want to record. Call this tool again with action: 'stop' to finish."
                }))?]))
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

                let file_content = std::fs::read_to_string(&file_path).unwrap_or_default();

                Ok(CallToolResult::success(vec![Content::json(json!({
                    "action": "record_workflow",
                    "status": "stopped",
                    "workflow_name": workflow_name,
                    "message": "Recording stopped and workflow saved.",
                    "file_path": file_path,
                    "file_content": file_content
                }))?]))
            }
            _ => Err(McpError::invalid_params(
                "Invalid action. Must be 'start' or 'stop'.",
                Some(json!({ "provided_action": args.action })),
            )),
        }
    }

    #[tool(
        description = "Executes multiple tools in sequence. Useful for automating complex workflows that require multiple steps. Each tool in the sequence can have its own error handling and delay configuration. Tool names can be provided either in short form (e.g., 'click_element') or full form (e.g., 'mcp_terminator-mcp-agent_click_element')."
    )]
    pub async fn execute_sequence(
        &self,
        Parameters(args): Parameters<ExecuteSequenceArgs>,
    ) -> Result<CallToolResult, McpError> {
        use crate::utils::{SequenceItem, ToolCall, ToolGroup};

        let stop_on_error = args.stop_on_error.unwrap_or(true);
        let include_detailed = args.include_detailed_results.unwrap_or(true);

        // Re-enabling validation logic
        if let Some(variable_schema) = &args.variables {
            let inputs_map = args
                .inputs
                .as_ref()
                .and_then(|v| v.as_object())
                .cloned()
                .unwrap_or_default();

            for (key, def) in variable_schema {
                let value = inputs_map.get(key).or(def.default.as_ref());

                match value {
                    Some(val) => {
                        // Validate the value against the definition
                        match def.r#type {
                            crate::utils::VariableType::String => {
                                if !val.is_string() {
                                    return Err(McpError::invalid_params(
                                        format!("Variable '{key}' must be a string."),
                                        Some(json!({"value": val})),
                                    ));
                                }
                                // TODO broken
                                // if let (Some(regex_str), Some(val_str)) =
                                //     (def.regex.as_ref(), val.as_str())
                                // {
                                //     let re = Regex::new(regex_str).map_err(|e| {
                                //         McpError::invalid_params(
                                //             format!("Invalid regex for '{key}'"),
                                //             Some(json!({
                                //                 "regex": regex_str,
                                //                 "error": e.to_string()
                                //             })),
                                //         )
                                //     })?;
                                //     if !re.is_match(val_str) {
                                //         return Err(McpError::invalid_params(
                                //             format!(
                                //                 "Variable '{key}' does not match regex pattern."
                                //             ),
                                //             Some(
                                //                 json!({"value": val_str, "regex": regex_str}),
                                //             ),
                                //         ));
                                //     }
                                // }
                            }
                            crate::utils::VariableType::Number => {
                                if !val.is_number() {
                                    return Err(McpError::invalid_params(
                                        format!("Variable '{key}' must be a number."),
                                        Some(json!({"value": val})),
                                    ));
                                }
                            }
                            crate::utils::VariableType::Boolean => {
                                if !val.is_boolean() {
                                    return Err(McpError::invalid_params(
                                        format!("Variable '{key}' must be a boolean."),
                                        Some(json!({"value": val})),
                                    ));
                                }
                            }
                            crate::utils::VariableType::Enum => {
                                let val_str = val.as_str().ok_or_else(|| {
                                    McpError::invalid_params(
                                        format!("Enum variable '{key}' must be a string."),
                                        Some(json!({"value": val})),
                                    )
                                })?;
                                if let Some(options) = &def.options {
                                    if !options.contains(&val_str.to_string()) {
                                        return Err(McpError::invalid_params(
                                            format!("Variable '{key}' has an invalid value."),
                                            Some(json!({
                                                "value": val_str,
                                                "allowed_options": options
                                            })),
                                        ));
                                    }
                                }
                            }
                            crate::utils::VariableType::Array => {
                                if !val.is_array() {
                                    return Err(McpError::invalid_params(
                                        format!("Variable '{key}' must be an array."),
                                        Some(json!({"value": val})),
                                    ));
                                }
                            }
                            crate::utils::VariableType::Object => {
                                if !val.is_object() {
                                    return Err(McpError::invalid_params(
                                        format!("Variable '{key}' must be an object."),
                                        Some(json!({"value": val})),
                                    ));
                                }
                            }
                        }
                    }
                    None => {
                        if def.required.unwrap_or(true) {
                            return Err(McpError::invalid_params(
                                format!("Required variable '{key}' is missing."),
                                None,
                            ));
                        }
                    }
                }
            }
        }

        // Build the execution context. It's a combination of the 'inputs' and 'selectors' from the arguments.
        // The context is a simple, flat map of variables that will be used for substitution in tool arguments.
        let mut execution_context_map = serde_json::Map::new();

        // First, populate with default values from variables schema
        if let Some(variable_schema) = &args.variables {
            for (key, def) in variable_schema {
                if let Some(default_value) = &def.default {
                    execution_context_map.insert(key.clone(), default_value.clone());
                }
            }
        }

        // Then override with user-provided inputs (inputs take precedence over defaults)
        if let Some(inputs) = &args.inputs {
            // Validate inputs is an object
            if let Err(err) = crate::utils::validate_inputs(inputs) {
                return Err(McpError::invalid_params(
                    format!(
                        "Invalid inputs: {} expected {}, got {}",
                        err.field, err.expected, err.actual
                    ),
                    None,
                ));
            }
            if let Some(inputs_map) = inputs.as_object() {
                for (key, value) in inputs_map {
                    execution_context_map.insert(key.clone(), value.clone());
                }
            }
        }

        if let Some(selectors) = args.selectors.clone() {
            // Validate selectors
            if let Err(err) = crate::utils::validate_selectors(&selectors) {
                return Err(McpError::invalid_params(
                    format!(
                        "Invalid selectors: {} expected {}, got {}",
                        err.field, err.expected, err.actual
                    ),
                    None,
                ));
            }
            // If selectors is a string, parse it as JSON first
            let selectors_value = if let serde_json::Value::String(s) = &selectors {
                match serde_json::from_str::<serde_json::Value>(s) {
                    Ok(parsed) => parsed,
                    Err(_) => selectors, // If parsing fails, treat it as a raw string
                }
            } else {
                selectors
            };
            execution_context_map.insert("selectors".to_string(), selectors_value);
        }
        let execution_context = serde_json::Value::Object(execution_context_map);

        info!(
            "Executing sequence with context: {}",
            serde_json::to_string_pretty(&execution_context).unwrap_or_default()
        );

        // Convert flattened SequenceStep to internal SequenceItem representation
        let mut sequence_items = Vec::new();
        for step in &args.steps {
            let item = if let Some(tool_name) = &step.tool_name {
                let tool_call = ToolCall {
                    tool_name: tool_name.clone(),
                    arguments: step.arguments.clone().unwrap_or(serde_json::json!({})),
                    continue_on_error: step.continue_on_error,
                    delay_ms: step.delay_ms,
                    id: step.id.clone(),
                };
                SequenceItem::Tool { tool_call }
            } else if let Some(group_name) = &step.group_name {
                let tool_group = ToolGroup {
                    group_name: group_name.clone(),
                    steps: step
                        .steps
                        .clone()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|s| ToolCall {
                            tool_name: s.tool_name,
                            arguments: s.arguments,
                            continue_on_error: s.continue_on_error,
                            delay_ms: s.delay_ms,
                            id: s.id,
                        })
                        .collect(),
                    skippable: step.skippable,
                };
                SequenceItem::Group { tool_group }
            } else {
                return Err(McpError::invalid_params(
                "Each step must have either tool_name (for single tools) or group_name (for groups)",
                Some(json!({"invalid_step": step})),
            ));
            };
            sequence_items.push(item);
        }

        // ---------------------------
        // Check for parallel execution mode
        // ---------------------------
        
        let enable_parallel = args.parallel_execution.unwrap_or(false);
        
        if enable_parallel {
            // Use simple parallel execution path
            return self.execute_sequence_simple_parallel(
                args,
                execution_context,
                stop_on_error,
                include_detailed,
            ).await;
        }

        // ---------------------------
        // Fallback-enabled execution loop (while-based) - Original Sequential Mode
        // ---------------------------

        let mut results = Vec::new();
        let mut sequence_had_errors = false;
        let mut critical_error_occurred = false;
        let start_time = chrono::Utc::now();

        let mut current_index: usize = 0;
        let max_iterations = sequence_items.len() * 10; // Prevent infinite fallback loops
        let mut iterations = 0usize;

        // Build a map from step ID to its index for quick fallback lookup
        use std::collections::HashMap;
        let mut id_to_index: HashMap<String, usize> = HashMap::new();
        for (idx, step) in args.steps.iter().enumerate() {
            if let Some(id) = &step.id {
                if id_to_index.insert(id.clone(), idx).is_some() {
                    warn!(
                        "Duplicate step id '{}' found; later occurrence overrides earlier.",
                        id
                    );
                }
            }
        }

        while current_index < sequence_items.len() && iterations < max_iterations {
            iterations += 1;

            let original_step = &args.steps[current_index];
            let (if_expr, retries, fallback_id_opt) = (
                original_step.r#if.clone(),
                original_step.retries.unwrap_or(0),
                original_step.fallback_id.clone(),
            );

            let is_always_step = if_expr.as_deref().is_some_and(|s| s.trim() == "always()");

            // If a critical error occurred and this step is NOT an 'always' step, skip it.
            if critical_error_occurred && !is_always_step {
                results.push(json!({
                    "index": current_index,
                    "status": "skipped",
                    "reason": "Skipped due to a previous unrecoverable error in the sequence."
                }));
                current_index += 1;
                continue;
            }

            // 1. Evaluate condition, unless it's an 'always' step.
            if let Some(cond_str) = &if_expr {
                if !is_always_step && !expression_eval::evaluate(cond_str, &execution_context) {
                    info!(
                        "Skipping step {} due to if expression not met: `{}`",
                        current_index, cond_str
                    );
                    results.push(json!({
                        "index": current_index,
                        "status": "skipped",
                        "reason": format!("if_expr not met: {}", cond_str)
                    }));
                    current_index += 1;
                    continue;
                }
            }

            // 2. Execute with retries
            let mut final_result = json!(null);
            let mut step_error_occurred = false;

            for attempt in 0..=retries {
                let item = &mut sequence_items[current_index];
                match item {
                    SequenceItem::Tool { tool_call } => {
                        // Substitute variables in arguments before execution
                        let mut substituted_args = tool_call.arguments.clone();
                        substitute_variables(&mut substituted_args, &execution_context);

                        let (result, error_occurred) = self
                            .execute_single_tool(
                                &tool_call.tool_name,
                                &substituted_args,
                                tool_call.continue_on_error.unwrap_or(false),
                                current_index,
                                include_detailed,
                                original_step.id.as_deref(),
                            )
                            .await;

                        final_result = result.clone();
                        if result["status"] == "success" {
                            break;
                        }

                        if error_occurred {
                            critical_error_occurred = true;
                        }
                        step_error_occurred = true;
                        sequence_had_errors = true;

                        if let Some(delay_ms) = tool_call.delay_ms {
                            if delay_ms > 0 {
                                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                            }
                        }
                    }
                    SequenceItem::Group { tool_group } => {
                        let mut group_had_errors = false;
                        let mut group_results = Vec::new();
                        let is_skippable = tool_group.skippable.unwrap_or(false);

                        for (step_index, step_tool_call) in tool_group.steps.iter_mut().enumerate()
                        {
                            // Substitute variables in arguments before execution
                            let mut substituted_args = step_tool_call.arguments.clone();
                            substitute_variables(&mut substituted_args, &execution_context);

                            let (result, error_occurred) = self
                                .execute_single_tool(
                                    &step_tool_call.tool_name,
                                    &substituted_args,
                                    step_tool_call.continue_on_error.unwrap_or(false),
                                    step_index,
                                    include_detailed,
                                    step_tool_call.id.as_deref(), // Use step ID if available
                                )
                                .await;

                            group_results.push(result.clone());

                            if let Some(delay_ms) = step_tool_call.delay_ms {
                                if delay_ms > 0 {
                                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                                }
                            }

                            let tool_failed = result["status"] != "success";
                            if tool_failed {
                                group_had_errors = true;
                                if error_occurred || is_skippable {
                                    if error_occurred && !is_skippable {
                                        critical_error_occurred = true;
                                    }
                                    break;
                                }
                            }
                        }

                        let group_status = if group_had_errors {
                            "partial_success"
                        } else {
                            "success"
                        };

                        if group_status != "success" {
                            sequence_had_errors = true;
                            step_error_occurred = true;
                        }

                        if group_had_errors && !is_skippable && stop_on_error {
                            critical_error_occurred = true;
                        }

                        final_result = json!({
                            "group_name": &tool_group.group_name,
                            "status": group_status,
                            "results": group_results
                        });

                        if !group_had_errors {
                            break; // Group succeeded, break retry loop.
                        }
                    }
                }
                if attempt < retries {
                    warn!(
                        "Step {} failed on attempt {}/{}. Retrying...",
                        current_index,
                        attempt + 1,
                        retries
                    );
                    tokio::time::sleep(Duration::from_millis(500)).await; // Wait before retry
                }
            }

            results.push(final_result);

            // Decide next index based on success or fallback
            let step_succeeded = !step_error_occurred;

            if step_succeeded {
                current_index += 1;
            } else if let Some(fb_id) = fallback_id_opt {
                if let Some(&fb_idx) = id_to_index.get(&fb_id) {
                    info!(
                        "Step {} failed. Jumping to fallback step with id '{}' (index {}).",
                        current_index, fb_id, fb_idx
                    );
                    current_index = fb_idx;
                } else {
                    warn!(
                        "fallback_id '{}' for step {} not found. Continuing to next step.",
                        fb_id, current_index
                    );
                    current_index += 1;
                }
            } else {
                current_index += 1;
            }
        }

        if iterations >= max_iterations {
            warn!("Maximum iteration count reached. Possible infinite fallback loop detected.");
        }

        let total_duration = (chrono::Utc::now() - start_time).num_milliseconds();

        let final_status = if !sequence_had_errors {
            "success"
        } else if critical_error_occurred {
            "partial_success"
        } else {
            "completed_with_errors"
        };

        let mut summary = json!({
            "action": "execute_sequence",
            "status": final_status,
            "total_tools": sequence_items.len(),
            "executed_tools": results.len(),
            "total_duration_ms": total_duration,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "results": results,
        });

        if let Some(parser_def) = args.output_parser.as_ref() {
            // Apply variable substitution to the output_parser field
            let mut parser_json = parser_def.clone();
            substitute_variables(&mut parser_json, &execution_context);

            match output_parser::run_output_parser(&parser_json, &summary).await {
                Ok(Some(parsed_data)) => {
                    if let Some(obj) = summary.as_object_mut() {
                        obj.insert("parsed_output".to_string(), parsed_data);
                    }
                }
                Ok(None) => {
                    if let Some(obj) = summary.as_object_mut() {
                        obj.insert("parsed_output".to_string(), json!({}));
                    }
                    // UI tree not found, which is not an error, just means nothing to parse.
                }
                Err(e) => {
                    if let Some(obj) = summary.as_object_mut() {
                        obj.insert("parser_error".to_string(), json!(e.to_string()));
                    }
                }
            }
        }

        let mut maybe_base64_image: Option<String> = None;
        if final_status != "success" {
            let debug_info = json!({});

            // Capture a screenshot on failure for visual context
            if let Ok(monitor) = self.desktop.get_primary_monitor().await {
                if let Ok(screenshot_result) = self.desktop.capture_monitor(&monitor).await {
                    let mut png_data = Vec::new();
                    let encoder = PngEncoder::new(Cursor::new(&mut png_data));
                    if encoder
                        .write_image(
                            &screenshot_result.image_data,
                            screenshot_result.width,
                            screenshot_result.height,
                            ExtendedColorType::Rgba8,
                        )
                        .is_ok()
                    {
                        let base64_image = general_purpose::STANDARD.encode(&png_data);
                        maybe_base64_image = Some(base64_image);
                    }
                }
            }

            if let Some(obj) = summary.as_object_mut() {
                obj.insert("debug_info_on_failure".to_string(), debug_info);
            }
        }

        let mut contents = vec![Content::json(summary)?];
        if let Some(base64_image) = maybe_base64_image {
            contents.push(Content::image(base64_image, "image/png".to_string()));
        }

        Ok(CallToolResult::success(contents))
    }

    // Simple parallel execution implementation
    async fn execute_sequence_simple_parallel(
        &self,
        args: ExecuteSequenceArgs,
        execution_context: serde_json::Value,
        stop_on_error: bool,
        include_detailed: bool,
    ) -> Result<CallToolResult, McpError> {
        use crate::utils::create_simple_execution_plan;
        
        let start_time = chrono::Utc::now();
        let (sequential_steps, parallel_groups) = create_simple_execution_plan(&args.steps);
        
        let mut results = Vec::new();
        let mut sequence_had_errors = false;
        let mut critical_error_occurred = false;
        let mut step_counter = 0;
        
        info!("Executing sequence with {} sequential steps and {} parallel groups", 
              sequential_steps.len(), parallel_groups.len());
        
        // Process steps in order, alternating between sequential and parallel
        let mut seq_idx = 0;
        let mut par_idx = 0;
        
        for original_step_idx in 0..args.steps.len() {
            if critical_error_occurred && stop_on_error {
                break;
            }
            
            // Check if this step is sequential
            if seq_idx < sequential_steps.len() && sequential_steps[seq_idx] == original_step_idx {
                let step = &args.steps[original_step_idx];
                let (result, error_occurred) = self.execute_single_tool(
                    &step.tool_name.as_ref().unwrap(),
                    &step.arguments.clone().unwrap_or(json!({})),
                    step.continue_on_error.unwrap_or(false),
                    original_step_idx,
                    include_detailed,
                    step.id.as_deref(),
                ).await;
                
                results.push(result);
                if error_occurred {
                    sequence_had_errors = true;
                    if stop_on_error {
                        critical_error_occurred = true;
                    }
                }
                
                if let Some(delay_ms) = step.delay_ms {
                    if delay_ms > 0 {
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    }
                }
                
                seq_idx += 1;
            }
            // Check if this step starts a parallel group
            else if par_idx < parallel_groups.len() && parallel_groups[par_idx].contains(&original_step_idx) {
                let group = &parallel_groups[par_idx];
                let mut handles = Vec::new();
                
                for &step_index in group {
                    let step = args.steps[step_index].clone();
                    let execution_context = execution_context.clone();
                    let desktop = self.clone();
                    
                    let handle = tokio::spawn(async move {
                        let mut substituted_args = step.arguments.clone().unwrap_or(json!({}));
                        substitute_variables(&mut substituted_args, &execution_context);
                        
                        desktop.execute_single_tool(
                            &step.tool_name.as_ref().unwrap(),
                            &substituted_args,
                            step.continue_on_error.unwrap_or(false),
                            step_index,
                            include_detailed,
                            step.id.as_deref(),
                        ).await
                    });
                    
                    handles.push((step_index, handle));
                }
                
                // Wait for all parallel tasks and collect results in order
                let mut group_results = Vec::new();
                for (step_index, handle) in handles {
                    match handle.await {
                        Ok((result, error_occurred)) => {
                            group_results.push((step_index, result, error_occurred));
                        }
                        Err(e) => {
                            group_results.push((step_index, json!({
                                "status": "error",
                                "error": format!("Task join error: {}", e)
                            }), true));
                        }
                    }
                }
                
                // Sort results by step index and add them
                group_results.sort_by_key(|(step_index, _, _)| *step_index);
                for (_, result, error_occurred) in group_results {
                    results.push(result);
                    if error_occurred {
                        sequence_had_errors = true;
                        if stop_on_error {
                            critical_error_occurred = true;
                        }
                    }
                }
                
                // Skip to end of this parallel group
                step_counter = *group.iter().max().unwrap();
                par_idx += 1;
            }
        }
        
        let total_duration = (chrono::Utc::now() - start_time).num_milliseconds();
        
        let final_status = if !sequence_had_errors {
            "success"
        } else if critical_error_occurred {
            "partial_success"  
        } else {
            "completed_with_errors"
        };
        
        let mut summary = json!({
            "action": "execute_sequence",
            "status": final_status,
            "execution_mode": "parallel",
            "total_tools": args.steps.len(),
            "executed_tools": results.len(),
            "total_duration_ms": total_duration,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "results": results,
        });
        
        // Apply output parser if specified
        if let Some(parser_def) = args.output_parser.as_ref() {
            let mut parser_json = parser_def.clone();
            substitute_variables(&mut parser_json, &execution_context);
            
            match output_parser::run_output_parser(&parser_json, &summary).await {
                Ok(Some(parsed_data)) => {
                    if let Some(obj) = summary.as_object_mut() {
                        obj.insert("parsed_output".to_string(), parsed_data);
                    }
                }
                Ok(None) => {
                    if let Some(obj) = summary.as_object_mut() {
                        obj.insert("parsed_output".to_string(), json!({}));
                    }
                }
                Err(e) => {
                    warn!("Output parser failed: {}", e);
                    if let Some(obj) = summary.as_object_mut() {
                        obj.insert("parser_error".to_string(), json!(e.to_string()));
                    }
                }
            }
        }
        
        Ok(CallToolResult::success(vec![Content::json(summary)?]))
    }


    async fn execute_single_tool(
        &self,
        tool_name: &str,
        arguments: &Value,
        is_skippable: bool,
        index: usize,
        include_detailed: bool,
        step_id: Option<&str>,
    ) -> (serde_json::Value, bool) {
        let tool_start_time = chrono::Utc::now();
        let tool_name_short = tool_name
            .strip_prefix("mcp_terminator-mcp-agent_")
            .unwrap_or(tool_name);

        // The substitution is now done at the higher level in `execute_sequence`.
        // This function now receives arguments with variables already substituted.

        let tool_result = self.dispatch_tool(tool_name_short, arguments).await;

        let (processed_result, error_occurred) = match tool_result {
            Ok(result) => {
                let mut extracted_content = Vec::new();

                for content in &result.content {
                    match extract_content_json(content) {
                        Ok(json_content) => extracted_content.push(json_content),
                        Err(_) => extracted_content.push(
                            json!({ "type": "unknown", "data": "Content extraction failed" }),
                        ),
                    }
                }

                let content_summary = if include_detailed {
                    json!({ "type": "tool_result", "content_count": result.content.len(), "content": extracted_content })
                } else {
                    json!({ "type": "summary", "content": "Tool executed successfully", "content_count": result.content.len() })
                };
                let duration_ms = (chrono::Utc::now() - tool_start_time).num_milliseconds();
                let mut result_json = json!({
                    "tool_name": tool_name,
                    "index": index,
                    "status": "success",
                    "duration_ms": duration_ms,
                    "result": content_summary,
                });

                // Add step_id if provided
                if let Some(id) = step_id {
                    if let Some(obj) = result_json.as_object_mut() {
                        obj.insert("step_id".to_string(), json!(id));
                    }
                }

                let result_json =
                    serde_json::Value::Object(result_json.as_object().unwrap().clone());
                (result_json, false)
            }
            Err(e) => {
                let duration_ms = (chrono::Utc::now() - tool_start_time).num_milliseconds();
                let mut error_result = json!({
                    "tool_name": tool_name,
                    "index": index,
                    "status": if is_skippable { "skipped" } else { "error" },
                    "duration_ms": duration_ms,
                    "error": format!("{}", e),
                });

                // Add step_id if provided
                if let Some(id) = step_id {
                    if let Some(obj) = error_result.as_object_mut() {
                        obj.insert("step_id".to_string(), json!(id));
                    }
                }

                let error_result =
                    serde_json::Value::Object(error_result.as_object().unwrap().clone());

                if !is_skippable {
                    warn!(
                        "Tool '{}' at index {} failed. Reason: {}",
                        tool_name, index, e
                    );
                }
                (error_result, !is_skippable)
            }
        };

        (processed_result, error_occurred)
    }

    async fn dispatch_tool(
        &self,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<CallToolResult, McpError> {
        use rmcp::handler::server::tool::Parameters;
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
            "wait_for_output_parser" => {
                match serde_json::from_value::<WaitForOutputParserArgs>(arguments.clone()) {
                    Ok(args) => self.wait_for_output_parser(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for wait_for_output_parser",
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
                Ok(args) => self.run_command(Parameters(args)).await,
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
            "run_javascript" => {
                match serde_json::from_value::<RunJavascriptArgs>(arguments.clone()) {
                    Ok(args) => self.run_javascript(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for run_javascript",
                        Some(json!({"error": e.to_string()})),
                    )),
                }
            }
            "export_workflow_sequence" => {
                match serde_json::from_value::<ExportWorkflowSequenceArgs>(arguments.clone()) {
                    Ok(args) => self.export_workflow_sequence(Parameters(args)).await,
                    Err(e) => Err(McpError::invalid_params(
                        "Invalid arguments for export_workflow_sequence",
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

    #[tool(
        description = "Edits workflow files using simple text find/replace operations. Works like sed - finds text patterns and replaces them, or appends content if no pattern specified."
    )]
    pub async fn export_workflow_sequence(
        &self,
        Parameters(args): Parameters<ExportWorkflowSequenceArgs>,
    ) -> Result<CallToolResult, McpError> {
        let use_regex = args.use_regex.unwrap_or(false);
        let create_if_missing = args.create_if_missing.unwrap_or(true);

        // Read existing file or start with empty content
        let current_content = match std::fs::read_to_string(&args.file_path) {
            Ok(content) => content,
            Err(_) => {
                if create_if_missing {
                    String::new()
                } else {
                    return Err(McpError::invalid_params(
                        "File does not exist and create_if_missing is false",
                        Some(json!({"file_path": args.file_path})),
                    ));
                }
            }
        };

        let new_content = if let Some(find_pattern) = &args.find_pattern {
            // Replace mode - find and replace pattern with content
            if use_regex {
                // Use regex replacement
                match regex::Regex::new(find_pattern) {
                    Ok(re) => {
                        let result = re.replace_all(&current_content, args.content.as_str());
                        if result == current_content {
                            return Err(McpError::invalid_params(
                                "Pattern not found in file",
                                Some(json!({"pattern": find_pattern, "file": args.file_path})),
                            ));
                        }
                        result.to_string()
                    }
                    Err(e) => {
                        return Err(McpError::invalid_params(
                            "Invalid regex pattern",
                            Some(json!({"pattern": find_pattern, "error": e.to_string()})),
                        ));
                    }
                }
            } else {
                // Simple string replacement
                if !current_content.contains(find_pattern) {
                    return Err(McpError::invalid_params(
                        "Pattern not found in file",
                        Some(json!({"pattern": find_pattern, "file": args.file_path})),
                    ));
                }
                current_content.replace(find_pattern, &args.content)
            }
        } else {
            // Append mode - add content to end of file
            if current_content.is_empty() {
                args.content
            } else if current_content.ends_with('\n') {
                format!("{}{}", current_content, args.content)
            } else {
                format!("{}\n{}", current_content, args.content)
            }
        };

        // Write back to file
        std::fs::write(&args.file_path, &new_content).map_err(|e| {
            McpError::internal_error(
                "Failed to write file",
                Some(json!({"error": e.to_string(), "path": args.file_path})),
            )
        })?;

        // Return success
        Ok(CallToolResult::success(vec![Content::json(json!({
            "action": "edit_workflow_file",
            "status": "success",
            "file_path": args.file_path,
            "operation": if args.find_pattern.is_some() { "replace" } else { "append" },
            "pattern_type": if use_regex { "regex" } else { "string" },
            "file_size": new_content.len(),
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))?]))
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
            args.include_tree.unwrap_or(true),
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
            args.include_tree.unwrap_or(true),
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
            args.include_tree.unwrap_or(false),
            None,
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
            args.include_tree.unwrap_or(true),
            Some(element.process_id().unwrap_or(0)),
            &mut result_json,
        );
        Ok(CallToolResult::success(vec![Content::json(result_json)?]))
    }

    #[tool(
        description = "Executes arbitrary JavaScript inside an embedded JS engine. The script receives a synchronous helper `callTool(name, argsJsonString)` that can invoke any other MCP tool and return its JSON result. The final value of the script is serialized to JSON and returned as the tool output.  NOTE: This is EXPERIMENTAL and currently uses a sandboxed QuickJS runtime; only standard JavaScript plus the provided helper is available."
    )]
    async fn run_javascript(
        &self,
        Parameters(args): Parameters<RunJavascriptArgs>,
    ) -> Result<CallToolResult, McpError> {
        use serde_json::json;

        let engine = args.engine.clone().unwrap_or_else(|| "nodejs".to_string());

        if engine == "boa" {
            let self_clone = self.clone();

            // Create a tool dispatcher closure that calls our dispatch_tool method
            let tool_dispatcher = move |tool_name: String, args_val: serde_json::Value| {
                let self_clone = self_clone.clone();
                async move {
                    match self_clone.dispatch_tool(&tool_name, &args_val).await {
                        Ok(call_result) => serde_json::to_string(&call_result).map_err(|e| {
                            McpError::internal_error(
                                "Failed to serialize tool result",
                                Some(json!({"error": e.to_string(), "tool": tool_name})),
                            )
                        }),
                        Err(e) => Err(e),
                    }
                }
            };

            // Execute JavaScript using the new terminator_js module
            let execution_result =
                scripting_engine::execute_javascript(args.script, tool_dispatcher).await?;

            return Ok(CallToolResult::success(vec![Content::json(json!({
                "action": "run_javascript",
                "status": "success",
                "engine": "boa",
                "result": execution_result
            }))?]));
        } else if engine == "nodejs" {
            let execution_result =
                scripting_engine::execute_javascript_with_nodejs(args.script).await?;
            return Ok(CallToolResult::success(vec![Content::json(json!({
                "action": "run_javascript",
                "status": "success",
                "engine": "nodejs",
                "result": execution_result
            }))?]));
        }

        Err(McpError::internal_error(
            "Unsupported JavaScript engine",
            Some(json!({"error": "Unsupported JavaScript engine"})),
        ))
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
