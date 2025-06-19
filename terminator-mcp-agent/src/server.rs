use crate::utils::{
    get_timeout, ClipboardArgs, DesktopWrapper, EmptyArgs, GetClipboardArgs, GetWindowTreeArgs,
    GetWindowsArgs, HighlightElementArgs, LocatorArgs, MouseDragArgs, NavigateBrowserArgs,
    OpenApplicationArgs, PressKeyArgs, RunCommandArgs, ScrollElementArgs, TypeIntoElementArgs,
    ValidateElementArgs, WaitForElementArgs, GlobalKeyArgs,
};
use chrono::Local;
use rmcp::model::{
    CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
};
use rmcp::{tool, Error as McpError, ServerHandler};
use serde_json::json;
use std::env;
use terminator::{Browser, Desktop, Selector};


#[tool(tool_box)]
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

        Ok(Self { desktop })
    }

    #[tool(
        description = "Get the complete UI tree for an application by PID and optional window title."
    )]
    async fn get_window_tree(
        &self,
        #[tool(param)] args: GetWindowTreeArgs,
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
                    Some(json!({"reason": e.to_string()})),
                )
            })?;

        Ok(CallToolResult::success(vec![Content::json(&tree)?]))
    }

    #[tool(description = "Get all applications currently running.")]
    async fn get_applications(
        &self,
        #[tool(param)] _args: EmptyArgs,
    ) -> Result<CallToolResult, McpError> {
        let apps = self.desktop.applications().map_err(|e| {
            McpError::resource_not_found(
                "Failed to get applications",
                Some(json!({"reason": e.to_string()})),
            )
        })?;

        let app_info: Vec<_> = apps
            .iter()
            .map(|app| {
                json!({
                    "name": app.name().unwrap_or_default(),
                    "id": app.id().unwrap_or_default(),
                    "role": app.role(),
                    "pid": app.process_id().unwrap_or(0),
                    "suggested_selector": format!("name:{}", app.name().unwrap_or_default())
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::json(json!({
            "applications": app_info,
            "count": apps.len()
        }))?]))
    }

    #[tool(description = "Get windows for a specific application by name.")]
    async fn get_windows_for_application(
        &self,
        #[tool(param)] args: GetWindowsArgs,
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

    #[tool(description = "Types text into a UI element.")]
    async fn type_into_element(
        &self,
        #[tool(param)] args: TypeIntoElementArgs,
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

        // Get element details before typing for better feedback
        let element_info = json!({
            "name": element.name().unwrap_or_default(),
            "role": element.role(),
            "id": element.id().unwrap_or_default(),
            "bounds": element.bounds().map(|b| json!({
                "x": b.0, "y": b.1, "width": b.2, "height": b.3
            })).unwrap_or(json!(null)),
            "enabled": element.is_enabled().unwrap_or(false),
        });

        element.type_text(&args.text_to_type, false).map_err(|e| {
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

        Ok(CallToolResult::success(vec![Content::json(json!({
            "action": "type",
            "status": "success",
            "text_typed": args.text_to_type,
            "element": element_info,
            "selector": args.selector,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))?]))
    }

    #[tool(description = "Clicks a UI element.")]
    async fn click_element(
        &self,
        #[tool(param)] args: LocatorArgs,
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

        // Get element details before clicking for better feedback
        let element_info = json!({
            "name": element.name().unwrap_or_default(),
            "role": element.role(),
            "id": element.id().unwrap_or_default(),
            "bounds": element.bounds().map(|b| json!({
                "x": b.0, "y": b.1, "width": b.2, "height": b.3
            })).unwrap_or(json!(null)),
            "enabled": element.is_enabled().unwrap_or(false),
        });

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

        Ok(CallToolResult::success(vec![Content::json(json!({
            "action": "click",
            "status": "success",
            "element": element_info,
            "selector": args.selector,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))?]))
    }

    #[tool(description = "Sends a key press to a UI element.")]
    async fn press_key(
        &self,
        #[tool(param)] args: PressKeyArgs,
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
        // Get element details before pressing key for better feedback
        let element_info = json!({
            "name": element.name().unwrap_or_default(),
            "role": element.role(),
            "id": element.id().unwrap_or_default(),
            "bounds": element.bounds().map(|b| json!({
                "x": b.0, "y": b.1, "width": b.2, "height": b.3
            })).unwrap_or(json!(null)),
            "enabled": element.is_enabled().unwrap_or(false),
        });

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

        Ok(CallToolResult::success(vec![Content::json(json!({
            "action": "press_key",
            "status": "success",
            "key_pressed": args.key,
            "element": element_info,
            "selector": args.selector,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))?]))
    }

    #[tool(description = "Sends a key press to the currently focused element (no selector required).")]
    async fn press_key_global(
        &self,
        #[tool(param)] args: GlobalKeyArgs,
    ) -> Result<CallToolResult, McpError> {
        // Identify focused element
        let focused = self.desktop.focused_element().map_err(|e| {
            McpError::internal_error(
                "Failed to get focused element",
                Some(json!({"reason": e.to_string()})),
            )
        })?;

        // Gather metadata for debugging / result payload
        let element_info = json!({
            "name": focused.name().unwrap_or_default(),
            "role": focused.role(),
            "id": focused.id().unwrap_or_default(),
            "bounds": focused.bounds().map(|b| json!({
                "x": b.0, "y": b.1, "width": b.2, "height": b.3
            })).unwrap_or(json!(null)),
            "enabled": focused.is_enabled().unwrap_or(false),
        });

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

        Ok(CallToolResult::success(vec![Content::json(json!({
            "action": "press_key_global",
            "status": "success",
            "key_pressed": args.key,
            "element": element_info,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))?]))
    }

    #[tool(description = "Executes a shell command.")]
    async fn run_command(
        &self,
        #[tool(param)] args: RunCommandArgs,
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
        #[tool(param)] args: LocatorArgs,
    ) -> Result<CallToolResult, McpError> {
        let locator = self.desktop.locator(Selector::from(args.selector.as_str()));
        let element = locator
            .wait(get_timeout(args.timeout_ms))
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "Failed to locate element",
                    Some(json!({"reason": e.to_string()})),
                )
            })?;
        element.activate_window().map_err(|e| {
            McpError::resource_not_found(
                "Failed to activate window with that element",
                Some(json!({"reason": e.to_string()})),
            )
        })?;
        Ok(CallToolResult::success(vec![Content::json(
            "Window with that element activated successfully",
        )?]))
    }

    #[tool(
        description = "Captures a screenshot of the primary monitor and returns the recognized text content (OCR)."
    )]
    async fn capture_screen(
        &self,
        #[tool(param)] _args: EmptyArgs,
    ) -> Result<CallToolResult, McpError> {
        let screenshot = self.desktop.capture_screen().await.map_err(|e| {
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
        #[tool(param)] args: ClipboardArgs,
    ) -> Result<CallToolResult, McpError> {
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
        #[tool(param)] _args: GetClipboardArgs,
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

    #[tool(description = "Performs a mouse drag operation from start to end coordinates.")]
    async fn mouse_drag(
        &self,
        #[tool(param)] args: MouseDragArgs,
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
        let element_info = json!({
            "name": element.name().unwrap_or_default(),
            "role": element.role(),
            "id": element.id().unwrap_or_default(),
            "bounds": element.bounds().map(|b| json!({
                "x": b.0, "y": b.1, "width": b.2, "height": b.3
            })).unwrap_or(json!(null)),
            "enabled": element.is_enabled().unwrap_or(false),
        });

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

        Ok(CallToolResult::success(vec![Content::json(json!({
            "action": "mouse_drag",
            "status": "success",
            "element": element_info,
            "selector": args.selector,
            "start": (args.start_x, args.start_y),
            "end": (args.end_x, args.end_y),
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))?]))
    }

    #[tool(
        description = "Validates that an element exists and provides detailed information about it."
    )]
    async fn validate_element(
        &self,
        #[tool(param)] args: ValidateElementArgs,
    ) -> Result<CallToolResult, McpError> {
        let locator = self.desktop.locator(Selector::from(args.selector.as_str()));

        match locator.wait(get_timeout(args.timeout_ms)).await {
            Ok(element) => {
                let element_info = json!({
                    "exists": true,
                    "name": element.name().unwrap_or_default(),
                    "role": element.role(),
                    "id": element.id().unwrap_or_default(),
                    "bounds": element.bounds().map(|b| json!({
                        "x": b.0, "y": b.1, "width": b.2, "height": b.3
                    })).unwrap_or(json!(null)),
                    "enabled": element.is_enabled().unwrap_or(false),
                    "visible": element.is_visible().unwrap_or(false),
                    "focused": element.is_focused().unwrap_or(false),
                    "keyboard_focusable": element.is_keyboard_focusable().unwrap_or(false),
                    "text": element.text(1).unwrap_or_default(),
                    "value": element.attributes().value.unwrap_or_default(),
                });

                Ok(CallToolResult::success(vec![Content::json(json!({
                    "action": "validate_element",
                    "status": "success",
                    "element": element_info,
                    "selector": args.selector,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }))?]))
            }
            Err(e) => Ok(CallToolResult::success(vec![Content::json(json!({
                "action": "validate_element",
                "status": "failed",
                "exists": false,
                "reason": e.to_string(),
                "selector": args.selector,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }))?])),
        }
    }

    #[tool(description = "Highlights an element with a colored border for visual confirmation.")]
    async fn highlight_element(
        &self,
        #[tool(param)] args: HighlightElementArgs,
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

        let element_info = json!({
            "name": element.name().unwrap_or_default(),
            "role": element.role(),
            "id": element.id().unwrap_or_default(),
            "bounds": element.bounds().map(|b| json!({
                "x": b.0, "y": b.1, "width": b.2, "height": b.3
            })).unwrap_or(json!(null)),
        });

        Ok(CallToolResult::success(vec![Content::json(json!({
            "action": "highlight_element",
            "status": "success",
            "element": element_info,
            "selector": args.selector,
            "color": args.color.unwrap_or(0x0000FF),
            "duration_ms": args.duration_ms.unwrap_or(1000),
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))?]))
    }

    #[tool(
        description = "Waits for an element to meet a specific condition (visible, enabled, focused, exists)."
    )]
    async fn wait_for_element(
        &self,
        #[tool(param)] args: WaitForElementArgs,
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
        #[tool(param)] args: NavigateBrowserArgs,
    ) -> Result<CallToolResult, McpError> {
        let browser = args.browser.clone().map(Browser::Custom);
        self.desktop
            .open_url(&args.url, browser)
            .map_err(|e| {
                McpError::internal_error(
                    "Failed to open URL",
                    Some(
                        json!({"reason": e.to_string(), "url": args.url, "browser": args.browser}),
                    ),
                )
            })?;

        Ok(CallToolResult::success(vec![Content::json(json!({
            "action": "navigate_browser",
            "status": "success",
            "url": args.url,
            "browser": args.browser,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))?]))
    }

    #[tool(description = "Opens an application by name (uses SDK's built-in app launcher).")]
    async fn open_application(
        &self,
        #[tool(param)] args: OpenApplicationArgs,
    ) -> Result<CallToolResult, McpError> {
        let result = self.desktop.open_application(&args.app_name).map_err(|e| {
            McpError::internal_error(
                "Failed to open application",
                Some(json!({"reason": e.to_string(), "app_name": args.app_name})),
            )
        })?;

        let element_info = json!({
            "name": result.name().unwrap_or_default(),
            "role": result.role(),
            "id": result.id().unwrap_or_default(),
            "pid": result.process_id().unwrap_or(0),
        });

        Ok(CallToolResult::success(vec![Content::json(json!({
            "action": "open_application",
            "status": "success",
            "app_name": args.app_name,
            "application": element_info,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))?]))
    }

    #[tool(
        description = "Closes a UI element (window, application, dialog, etc.) if it's closable."
    )]
    async fn close_element(
        &self,
        #[tool(param)] args: LocatorArgs,
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
        let element_info = json!({
            "name": element.name().unwrap_or_default(),
            "role": element.role(),
            "id": element.id().unwrap_or_default(),
            "bounds": element.bounds().map(|b| json!({
                "x": b.0, "y": b.1, "width": b.2, "height": b.3
            })).unwrap_or(json!(null)),
            "application": element.application_name(),
            "window_title": element.window_title(),
        });

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
        #[tool(param)] args: ScrollElementArgs,
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
        let element_info = json!({
            "name": element.name().unwrap_or_default(),
            "role": element.role(),
            "id": element.id().unwrap_or_default(),
            "bounds": element.bounds().map(|b| json!({
                "x": b.0, "y": b.1, "width": b.2, "height": b.3
            })).unwrap_or(json!(null)),
        });

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

        Ok(CallToolResult::success(vec![Content::json(json!({
            "action": "scroll_element",
            "status": "success",
            "element": element_info,
            "selector": args.selector,
            "direction": args.direction,
            "amount": args.amount,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))?]))
    }
}

#[tool(tool_box)]
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

**Core Workflow: Discover, then Act**

Your most reliable strategy is to inspect the application's UI structure *before* trying to interact with it. Do not guess selectors.

1.  **Discover Running Applications:** Use `get_applications` to see what's running. This gives you the `name` and `pid` (Process ID) for each application.

2.  **Get the UI Tree:** This is the most important step. Once you have the `pid` of your target application, call `get_window_tree`. This returns a complete, JSON-like structure of all UI elements in that application.

3.  **Find Your Target Element in the Tree:** Parse the tree to locate the element you need. Each element in the tree has several properties, but you should prioritize them in this order:
    *   `id`: This is the most reliable way to find an element. It's a unique identifier.
    *   `name`: The visible text or label of the element (e.g., \"Save\", \"File\").
    *   `role`: The type of the element (e.g., \"Button\", \"Window\", \"Edit\").

4.  **Construct a Selector String:** Create a `selector` string to target the element. You can chain selectors using ` >> `.
    *   **CRITICAL BEST PRACTICE:** When an element has an `id`, ALWAYS use JUST the ID with a hash prefix. For example, if ID is `12345`, use the selector `\"#12345\"`. Do NOT chain selectors when you have a reliable, unique ID.
    *   **Chaining Example:** To find a button named \"OK\" inside a window named \"My App\", you could use `\"window:'My App' >> button:OK\"`.

5.  **Interact with the Element:** Once you have a reliable `selector`, use an action tool:
    *   `click_element`: To click buttons, links, etc.
    *   `type_into_element`: To type text into input fields.
    *   `press_key`: Sends a key press to a UI element. **Key Syntax: Use curly braces for special keys!**
    *   `activate_element`: To bring a window to the foreground.
    *   `mouse_drag`: To perform drag and drop operations.
    *   `set_clipboard`: To set text to the system clipboard.
    *   `get_clipboard`: To get text from the system clipboard.
    *   `scroll_element`: To scroll within elements like web pages, documents, or lists.

6.  **Handle Scrolling for Full Context:** When working with pages or long content, ALWAYS scroll to see all content. Use `scroll_element` to scroll pages up/down to get the full context before making decisions or extracting information.

**Important: Key Syntax for press_key Tool**
When using the `press_key` tool, you MUST use curly braces for special keys:
- Single special key: Enter, Tab, Escape, Delete - wrap each in curly braces
- Key combinations: For Ctrl+V use curly-Ctrl curly-V, for Alt+F4 use curly-Alt curly-F4
- Windows key: For Win+D use curly-Win curly-D (shows desktop)
- Function keys: F1, F5 - wrap in curly braces
- Arrow keys: Up, Down, Left, Right - wrap in curly braces
- Regular text can be mixed with special keys wrapped in curly braces

**Example Scenario:**
1.  User: \"Type hello into Notepad.\"
2.  AI: Calls `get_applications` -> Finds Notepad, gets `pid`.
3.  AI: Calls `get_window_tree` with Notepad's `pid`.
4.  AI: Looks through the tree and finds the text area element with `id: \"edit_pane\"`.
5.  AI: Calls `type_into_element` with `selector: \"#edit_pane\"` and `text_to_type: \"hello\"`.

**Playbook for Robust Automation**

To make your automation scripts more reliable and easier to debug, follow these plays:

- **Favor Shell Commands for File Operations:** When working with files and folders (e.g., in File Explorer), use `run_command` with PowerShell (on Windows) or bash commands. This is significantly more reliable than simulating UI clicks for creating folders, moving files, or deleting them.
    - *Example:* To create a directory, use `run_command` with `\"mkdir 'my-folder'\"` instead of trying to find and click the \"New folder\" button.

- **Disambiguate Between Multiple Application Windows:** It's common for an application like `explorer.exe` or a web browser to have multiple processes or windows. If `get_applications` shows multiple entries, use `get_window_tree` on each `pid` to inspect them. Check the `title` or other unique elements in the tree to identify the correct window before proceeding.

- **Validate Before You Act:** Before you call `click_element` or `type_into_element`, consider using `validate_element` first. This helps confirm that your `selector` is correct and the element is present and enabled, preventing unnecessary failures.

- **Ensure Window is Active:** If clicks or key presses are not registering, the target window may not be in the foreground. Use `activate_element` on the window or a known element within it to bring it into focus before sending interactions.

**Available Tools:**

*   `get_applications`: Lists all currently running applications and their PIDs.
*   `get_window_tree`: Retrieves the entire UI element tree for an application, given its PID. **(Your primary discovery tool)**
*   `get_windows_for_application`: Get windows for a specific application by name.
*   `click_element`: Clicks a UI element specified by its `selector`.
*   `type_into_element`: Types text into a UI element.
*   `press_key`: Sends a key press to a UI element. **Key Syntax: Use curly braces for special keys!**
*   `activate_element`: Brings the window containing the element to the foreground.
*   `close_element`: Closes a UI element (window, application, dialog, etc.) if it's closable.
*   `scroll_element`: Scrolls a UI element in specified direction (up, down, left, right) by given amount.
*   `run_command`: Executes a shell command. Use this for file operations, etc., instead of UI automation.
*   `capture_screen`: Captures the screen and performs OCR.
*   `set_clipboard`: Sets text to the system clipboard using native commands.
*   `get_clipboard`: Gets text from the system clipboard using native commands.
*   `mouse_drag`: Performs a mouse drag operation from start to end coordinates.
*   `validate_element`: Validates that an element exists and provides detailed information.
*   `highlight_element`: Highlights an element with a colored border for visual confirmation.
*   `wait_for_element`: Waits for an element to meet a specific condition (visible, enabled, focused, exists).
*   `navigate_browser`: Opens a URL in the specified browser.
*   `open_application`: Opens an application by name.

Contextual information:
- The current date and time is {}.
- Current operating system: {}.
- Current working directory: {}.

**Golden Rules:** 
1. Always call `get_window_tree` to understand the UI landscape before you try to act on it. 
2. When an element has an `id`, use ONLY that ID with a hash prefix.
3. Always scroll pages to get full context when working with web pages or long documents.
",
        current_date_time, current_os, current_working_dir
    )
}
