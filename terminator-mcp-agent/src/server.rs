use crate::utils::{
    get_timeout, CaptureScreenArgs, DesktopWrapper, EmptyArgs, ExploreArgs, GetWindowTreeArgs,
    GetWindowsArgs, LocatorArgs, PressKeyArgs, RunCommandArgs, TypeIntoElementArgs,
};
use chrono::Local;
use rmcp::model::{
    CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
};
use rmcp::{tool, Error as McpError, ServerHandler};
use serde_json::json;
use std::env;
use terminator::{Desktop, Locator, Selector};

#[tool(tool_box)]
impl DesktopWrapper {
    pub async fn new() -> Result<Self, McpError> {
        #[cfg(target_os = "windows")]
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

        Ok(CallToolResult::success(vec![Content::json(&json!({
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

        Ok(CallToolResult::success(vec![Content::json(&json!({
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
        let locator = self.create_locator_for_chain(&args.selector_chain)?;
        let element = locator
            .wait(get_timeout(args.timeout_ms))
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "Failed to locate element",
                    Some(json!({"reason": e.to_string()})),
                )
            })?;
        element.type_text(&args.text_to_type, false).map_err(|e| {
            McpError::resource_not_found(
                "Failed to type text",
                Some(json!({"reason": e.to_string()})),
            )
        })?;

        Ok(CallToolResult::success(vec![Content::json(
            "Text typed successfully",
        )?]))
    }

    #[tool(description = "Clicks a UI element.")]
    async fn click_element(
        &self,
        #[tool(param)] args: LocatorArgs,
    ) -> Result<CallToolResult, McpError> {
        let locator = self.create_locator_for_chain(&args.selector_chain)?;
        let element = locator
            .wait(get_timeout(args.timeout_ms))
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "Failed to locate element",
                    Some(json!({"reason": e.to_string()})),
                )
            })?;
        element.click().map_err(|e| {
            McpError::resource_not_found(
                "Failed to click on element",
                Some(json!({"reason": e.to_string()})),
            )
        })?;

        Ok(CallToolResult::success(vec![Content::json(
            "Element clicked successfully",
        )?]))
    }

    #[tool(description = "Sends a key press to a UI element.")]
    async fn press_key(
        &self,
        #[tool(param)] args: PressKeyArgs,
    ) -> Result<CallToolResult, McpError> {
        let locator = self.create_locator_for_chain(&args.selector_chain)?;
        let element = locator
            .wait(get_timeout(args.timeout_ms))
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "Failed to locate element",
                    Some(json!({"reason": e.to_string()})),
                )
            })?;
        element.press_key(&args.key).map_err(|e| {
            McpError::resource_not_found(
                "Failed to press key",
                Some(json!({"reason": e.to_string()})),
            )
        })?;

        Ok(CallToolResult::success(vec![Content::json(
            "Key pressed successfully",
        )?]))
    }

    #[tool(description = "Executes a shell command.")]
    async fn run_command(
        &self,
        #[tool(param)] args: RunCommandArgs,
    ) -> Result<CallToolResult, McpError> {
        let output = self
            .desktop
            .run_command(args.unix_command.as_deref(), args.unix_command.as_deref())
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

    #[tool(description = "Explores UI elements and their children.")]
    async fn explore(&self, #[tool(param)] args: ExploreArgs) -> Result<CallToolResult, McpError> {
        let root_element = if let Some(selector_chain) = &args.selector_chain {
            if !selector_chain.is_empty() {
                let locator = self.create_locator_for_chain(selector_chain)?;
                locator
                    .wait(get_timeout(args.timeout_ms))
                    .await
                    .map_err(|e| {
                        McpError::internal_error(
                            "Failed to locate element",
                            Some(json!({"reason": e.to_string()})),
                        )
                    })?
            } else {
                self.desktop.root()
            }
        } else {
            self.desktop.root()
        };

        // Get children of the element with timeout protection
        let timeout_duration =
            get_timeout(args.timeout_ms).unwrap_or(std::time::Duration::from_secs(30));
        let root_element_clone = root_element.clone();
        let children_result = tokio::time::timeout(timeout_duration, async {
            tokio::task::spawn_blocking(move || root_element_clone.children()).await
        })
        .await;

        let children = match children_result {
            Ok(Ok(children)) => children,
            Ok(Err(join_err)) => {
                return Err(McpError::internal_error(
                    "Task join error",
                    Some(json!({"reason": join_err.to_string()})),
                ));
            }
            Err(_) => {
                return Err(McpError::internal_error(
                    "Timeout getting children",
                    Some(json!({"timeout_ms": timeout_duration.as_millis()})),
                ));
            }
        }
        .map_err(|e| {
            McpError::internal_error(
                "Failed to get children",
                Some(json!({"reason": e.to_string()})),
            )
        })?;

        // Convert to a format similar to Playwright's accessibility snapshot
        let mut child_elements = Vec::new();
        for (index, child) in children.iter().enumerate() {
            let element_info = json!({
                "index": index,
                "role": child.role(),
                "name": child.name().unwrap_or_default(),
                "id": child.id().unwrap_or_default(),
                "suggested_selector": format!("name:{}", child.name().unwrap_or_default()),
                "bounds": child.bounds().map(|b| json!({
                    "x": b.0, "y": b.1, "width": b.2, "height": b.3
                })).unwrap_or(json!(null)),
                "enabled": child.is_enabled().unwrap_or(false),
                "visible": true, // Assume visible for now
            });
            child_elements.push(element_info);
        }

        let parent_info = json!({
            "role": root_element.role(),
            "name": root_element.name().unwrap_or_default(),
            "id": root_element.id().unwrap_or_default(),
            "suggested_selector": format!("name:{}", root_element.name().unwrap_or_default()),
        });

        Ok(CallToolResult::success(vec![Content::json(&json!({
            "parent": parent_info,
            "children": child_elements,
            "total_children": children.len(),
        }))?]))
    }

    #[tool(
        description = "Activates the window containing the specified element, bringing it to the foreground."
    )]
    async fn activate_element(
        &self,
        #[tool(param)] args: LocatorArgs,
    ) -> Result<CallToolResult, McpError> {
        let locator = self.create_locator_for_chain(&args.selector_chain)?;
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
        #[tool(param)] _args: CaptureScreenArgs,
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

    // keep in wrapperr to avoid creating new instance
    fn create_locator_for_chain(&self, selector_chain: &[String]) -> Result<Locator, McpError> {
        if selector_chain.is_empty() {
            return Err(McpError::invalid_params(
                "selector_chain cannot be empty",
                None,
            ));
        }

        let selectors: Vec<Selector> = selector_chain.iter().map(|s| s.as_str().into()).collect();
        let mut locator = self.desktop.locator(selectors[0].clone());

        // Chain subsequent locators
        for selector in selectors.iter().skip(1) {
            locator = locator.locator(selector.clone());
        }

        Ok(locator)
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
        .unwrap_or_else(|_| "Unknown".to_string()); // Get current working directory

    format!(
        r#"
You are an AI assistant capable of controlling a computer desktop using the available tools.
You can interact with UI elements, run commands, and read text.

**Workflow:**
1.  **Identify Target Window:** Use 'find_window' to locate the main window of the application you need to interact with (e.g., by title). Note its selector (e.g., the `suggested_selector` field in the result, which often looks like '#12345...').
2.  **(Optional but Recommended) Explore:** Use 'explore' with the window's selector chain (e.g., ['window:"My App"'] or ['#windowId']) to understand its structure and find specific child elements (buttons, inputs, etc.). Pay close attention to the `suggested_selector` provided for each child element.
3.  **Interact:** Use tools like 'click_element', 'get_element_text', 'type_into_element', or 'press_key' with the appropriate **selector chain**.
    *   A selector chain is an array starting with the window selector, followed by selectors for child elements, e.g., ['window:"My App"', '#saveButtonId'].
    *   **Crucially, prefer using the exact `suggested_selector` (like '#12345...') returned by 'explore' or 'find_window'.** This ID is calculated based on multiple properties and is usually the most reliable way to target an element.
    *   If a suggested selector fails, you can try simpler selectors like `text:"Save"` or `role:"button"`, but these might match multiple elements or be less reliable.
    *   **Selector Failures:** If interaction fails (e.g., timeout), ensure the element is visible, try increasing the `timeout_ms` parameter (e.g., 10000 for 10 seconds), re-explore the parent element, or verify you have the correct window selector.
4.  **Execute Commands:** Use 'run_command' for non-UI tasks.
    *   **This is the preferred method for running shell commands (like `ls`, `dir`, `git status`, etc.)** instead of trying to type into a terminal UI element, which can be unreliable. Specify the command for Windows (`windows_command`) or Unix (`unix_command`).

**Available Tools:**

- **find_window**: Finds a top-level window by its title. Returns the window element's details, including a `suggested_selector`.
- **explore**: Lists child elements within a given element (or the screen). Use its output to find the `suggested_selector` for child elements needed in other tools.
- **click_element**: Clicks a UI element specified by its selector chain.
- **get_element_text**: Reads text content from a UI element specified by its selector chain.
- **type_into_element**: Types text into a UI element (like an input field) specified by its selector chain. Requires a reliable selector for the input element.
- **press_key**: Sends a key press (like 'Enter', 'Tab', 'Ctrl+C') to a UI element specified by its selector chain.
- **run_command**: Executes a shell command directly on the system (specify `windows_command` or `unix_command`). Ideal for terminal tasks.

Contextual information:
- The current date and time is {}.
- Current operating system: {}.
- Current working directory: {}.

**Important:** Always provide the full selector chain when interacting with elements inside a window. Start the chain with the window selector. **Prioritize using the `suggested_selector` from `explore` results.** Use `run_command` for shell operations.
"#,
        current_date_time, current_os, current_working_dir
    )
}
