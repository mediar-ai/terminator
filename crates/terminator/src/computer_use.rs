//! Gemini Computer Use - AI-powered autonomous desktop automation
//!
//! This module provides an agentic loop that uses Gemini's vision model
//! to autonomously control desktop applications to achieve a goal.

use crate::Desktop;
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageBuffer, ImageEncoder, Rgba};
use image::imageops::FilterType;
use serde::{Deserialize, Serialize};
use std::env;
use std::io::Cursor;
use std::time::Duration;
use sysinfo::{ProcessesToUpdate, System};
use tracing::{debug, info, warn};

// ===== Types =====

/// Function call from Gemini Computer Use model
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ComputerUseFunctionCall {
    /// Name of the action (e.g., "click_at", "type_text_at", "scroll_document")
    pub name: String,
    /// Arguments for the action
    #[serde(default)]
    pub args: serde_json::Value,
    /// Optional ID for the function call
    pub id: Option<String>,
}

/// Response from Computer Use backend
#[derive(Debug, Clone)]
pub struct ComputerUseResponse {
    /// True if task is complete (no more actions needed)
    pub completed: bool,
    /// Function call if action is needed
    pub function_call: Option<ComputerUseFunctionCall>,
    /// Text response from model (reasoning or final answer)
    pub text: Option<String>,
    /// Safety decision if confirmation required
    pub safety_decision: Option<String>,
}

/// Previous action to send back with screenshot (for multi-step)
#[derive(Debug, Serialize, Clone)]
pub struct ComputerUsePreviousAction {
    /// Action name that was executed
    pub name: String,
    /// Result of the action
    pub response: ComputerUseActionResponse,
    /// Screenshot after action (base64 PNG)
    pub screenshot: String,
    /// Current page URL (for browser contexts)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// Response for a previous action
#[derive(Debug, Serialize, Clone)]
pub struct ComputerUseActionResponse {
    /// Whether the action succeeded
    pub success: bool,
    /// Error message if action failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A single step in the computer use execution
#[derive(Debug, Clone, Serialize)]
pub struct ComputerUseStep {
    /// Step number (1-indexed)
    pub step: u32,
    /// Action that was executed
    pub action: String,
    /// Arguments passed to the action
    pub args: serde_json::Value,
    /// Whether the action succeeded
    pub success: bool,
    /// Error message if action failed
    pub error: Option<String>,
    /// Model's reasoning text for this step
    pub text: Option<String>,
}

/// Result of the computer use execution
#[derive(Debug, Clone, Serialize)]
pub struct ComputerUseResult {
    /// Status: "success", "failed", "needs_confirmation", "max_steps_reached"
    pub status: String,
    /// The goal that was attempted
    pub goal: String,
    /// Number of steps executed
    pub steps_executed: u32,
    /// Last action performed
    pub final_action: String,
    /// Final text response from model
    pub final_text: Option<String>,
    /// History of all steps
    pub steps: Vec<ComputerUseStep>,
    /// Pending confirmation info if status is "needs_confirmation"
    pub pending_confirmation: Option<serde_json::Value>,
}

/// Callback for progress updates during computer use execution
pub type ProgressCallback = Box<dyn Fn(&ComputerUseStep) + Send + Sync>;

/// Backend response structure
#[derive(Debug, Deserialize)]
struct ComputerUseBackendResponse {
    completed: bool,
    #[serde(default)]
    function_call: Option<ComputerUseFunctionCall>,
    text: Option<String>,
    safety_decision: Option<String>,
    #[allow(dead_code)]
    duration_ms: Option<u64>,
    #[allow(dead_code)]
    model_used: Option<String>,
    error: Option<String>,
}

/// Window capture data for computer use
struct WindowCaptureData {
    /// Base64 encoded PNG screenshot
    base64_image: String,
    /// Window bounds (x, y, width, height)
    window_bounds: (f64, f64, f64, f64),
    /// DPI scale factor
    dpi_scale: f64,
    /// Resize scale factor (if image was resized)
    resize_scale: f64,
    /// Browser URL if available
    browser_url: Option<String>,
}

// ===== Backend API =====

/// Call the Gemini Computer Use backend to get the next action.
async fn call_computer_use_backend(
    base64_image: &str,
    goal: &str,
    previous_actions: Option<&[ComputerUsePreviousAction]>,
) -> Result<ComputerUseResponse> {
    let backend_url = env::var("GEMINI_COMPUTER_USE_BACKEND_URL")
        .unwrap_or_else(|_| "https://app.mediar.ai/api/vision/computer-use".to_string());

    info!(
        "[computer_use] Calling backend at {} (goal: {})",
        backend_url,
        &goal[..goal.len().min(50)]
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()?;

    let payload = serde_json::json!({
        "image": base64_image,
        "goal": goal,
        "previous_actions": previous_actions.unwrap_or(&[])
    });

    let resp = client
        .post(&backend_url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        warn!("[computer_use] Backend error: {} - {}", status, text);
        return Err(anyhow!("Computer Use backend error ({}): {}", status, text));
    }

    let response_text = resp.text().await?;
    debug!(
        "[computer_use] Backend response: {}",
        &response_text[..response_text.len().min(500)]
    );

    let backend_response: ComputerUseBackendResponse = serde_json::from_str(&response_text)
        .map_err(|e| anyhow!("Failed to parse backend response: {}", e))?;

    if let Some(error) = backend_response.error {
        return Err(anyhow!("Computer Use error: {}", error));
    }

    Ok(ComputerUseResponse {
        completed: backend_response.completed,
        function_call: backend_response.function_call,
        text: backend_response.text,
        safety_decision: backend_response.safety_decision,
    })
}

// ===== Key Translation =====

/// Translate Gemini Computer Use key format to uiautomation format
/// Gemini: "enter", "control+a", "Meta+Shift+T"
/// uiautomation: "{Enter}", "{Ctrl}a", "{Win}{Shift}t"
fn translate_gemini_keys(gemini_keys: &str) -> Result<String, String> {
    let parts: Vec<&str> = gemini_keys.split('+').collect();
    let mut result = String::new();

    for (i, part) in parts.iter().enumerate() {
        let lower = part.trim().to_lowercase();
        let is_last = i == parts.len() - 1;

        let translated: &str = match lower.as_str() {
            // Modifiers
            "control" | "ctrl" => "{Ctrl}",
            "alt" => "{Alt}",
            "shift" => "{Shift}",
            "meta" | "cmd" | "command" | "win" | "windows" | "super" => "{Win}",

            // Common special keys
            "enter" | "return" => "{Enter}",
            "tab" => "{Tab}",
            "escape" | "esc" => "{Escape}",
            "backspace" | "back" => "{Backspace}",
            "delete" | "del" => "{Delete}",
            "space" => "{Space}",
            "insert" | "ins" => "{Insert}",
            "home" => "{Home}",
            "end" => "{End}",
            "pageup" | "page_up" | "pgup" => "{PageUp}",
            "pagedown" | "page_down" | "pgdn" => "{PageDown}",
            "capslock" | "caps" => "{CapsLock}",
            "numlock" => "{NumLock}",
            "scrolllock" => "{ScrollLock}",
            "printscreen" | "prtsc" => "{PrintScreen}",
            "pause" => "{Pause}",

            // Arrow keys
            "up" | "arrowup" => "{Up}",
            "down" | "arrowdown" => "{Down}",
            "left" | "arrowleft" => "{Left}",
            "right" | "arrowright" => "{Right}",

            // Function keys - handle dynamically
            s if s.starts_with('f') && s.len() <= 3 => {
                if let Ok(n) = s[1..].parse::<u8>() {
                    if (1..=24).contains(&n) {
                        result.push_str(&format!("{{F{}}}", n));
                        continue;
                    }
                }
                return Err(format!(
                    "Invalid function key '{}' in '{}'. Use f1-f24.",
                    part, gemini_keys
                ));
            }

            // Single character (a-z, 0-9) - only valid as last part of combination
            s if s.len() == 1 && is_last => {
                result.push_str(s);
                continue;
            }

            // Unknown key
            unknown => {
                return Err(format!(
                    "Unknown key '{}' in combination '{}'. Valid: enter, tab, escape, \
                     backspace, delete, space, up/down/left/right, home, end, pageup, \
                     pagedown, f1-f24, or modifiers (ctrl, alt, shift, meta) with letters.",
                    unknown, gemini_keys
                ));
            }
        };

        result.push_str(translated);
    }

    Ok(result)
}

// ===== Window Capture =====

/// Capture window screenshot for computer use
fn capture_window_for_computer_use(
    desktop: &Desktop,
    process: &str,
) -> Result<WindowCaptureData, String> {
    // Find the window element for this process using sysinfo to match process names
    let apps = desktop
        .applications()
        .map_err(|e| format!("Failed to get applications: {e}"))?;

    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);

    let window_element = apps
        .into_iter()
        .find(|app| {
            let app_pid = app.process_id().unwrap_or(0);
            if app_pid > 0 {
                system
                    .process(sysinfo::Pid::from_u32(app_pid))
                    .map(|p| {
                        let process_name = p.name().to_string_lossy().to_string();
                        process_name.to_lowercase().contains(&process.to_lowercase())
                    })
                    .unwrap_or(false)
            } else {
                false
            }
        })
        .ok_or_else(|| format!("No window found for process '{process}'"))?;

    // Get browser URL if available, or create synthetic URL for desktop apps
    // Gemini Computer Use API requires URL field for multi-step operations
    let browser_url = window_element.url().or_else(|| {
        // For desktop apps, create a synthetic URL: app://{process}/{window_name}
        let window_name = window_element
            .name()
            .unwrap_or_default()
            .replace(' ', "_")
            .replace('/', "_");
        Some(format!("app://{}/{}", process, window_name))
    });

    // Get window bounds (absolute screen coordinates)
    let bounds = window_element
        .bounds()
        .map_err(|e| format!("Failed to get window bounds: {e}"))?;
    let (window_x, window_y, win_w, _win_h) = bounds;

    // Capture screenshot
    let screenshot = window_element
        .capture()
        .map_err(|e| format!("Failed to capture screenshot: {e}"))?;

    let original_width = screenshot.width;
    let original_height = screenshot.height;

    // Calculate DPI scale
    let dpi_scale_w = original_width as f64 / win_w;

    // Convert BGRA to RGBA
    let rgba_data: Vec<u8> = screenshot
        .image_data
        .chunks_exact(4)
        .flat_map(|bgra| [bgra[2], bgra[1], bgra[0], bgra[3]])
        .collect();

    // Resize if needed (max 1920px)
    const MAX_DIM: u32 = 1920;
    let (final_width, final_height, final_rgba_data, resize_scale) =
        if original_width > MAX_DIM || original_height > MAX_DIM {
            let scale = (MAX_DIM as f32 / original_width.max(original_height) as f32).min(1.0);
            let new_width = (original_width as f32 * scale).round() as u32;
            let new_height = (original_height as f32 * scale).round() as u32;

            let img =
                ImageBuffer::<Rgba<u8>, _>::from_raw(original_width, original_height, rgba_data)
                    .ok_or("Failed to create image buffer")?;
            let resized = image::imageops::resize(&img, new_width, new_height, FilterType::Lanczos3);

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
        .map_err(|e| format!("Failed to encode PNG: {e}"))?;

    let base64_image = general_purpose::STANDARD.encode(&png_data);

    Ok(WindowCaptureData {
        base64_image,
        window_bounds: (window_x, window_y, final_width as f64, final_height as f64),
        dpi_scale: dpi_scale_w,
        resize_scale,
        browser_url,
    })
}

// ===== Action Execution =====

/// Execute a computer use action
async fn execute_action(
    desktop: &Desktop,
    action: &str,
    args: &serde_json::Value,
    window_bounds: (f64, f64, f64, f64),
    dpi_scale: f64,
    resize_scale: f64,
) -> Result<(), String> {
    let (window_x, window_y, screenshot_w, screenshot_h) = window_bounds;

    // Helper to get values from args
    let get_f64 = |key: &str| -> Option<f64> { args.get(key).and_then(|v| v.as_f64()) };
    let get_str = |key: &str| -> Option<&str> { args.get(key).and_then(|v| v.as_str()) };
    let get_bool = |key: &str| -> Option<bool> { args.get(key).and_then(|v| v.as_bool()) };

    // Helper to convert 0-999 normalized coords to absolute screen coords
    let convert_coord = |norm_x: f64, norm_y: f64| -> (f64, f64) {
        // Convert 0-999 to screenshot pixels
        let px_x = (norm_x / 1000.0) * screenshot_w;
        let px_y = (norm_y / 1000.0) * screenshot_h;
        // Apply inverse resize scale
        let px_x = px_x / resize_scale;
        let px_y = px_y / resize_scale;
        // Apply DPI conversion (physical to logical)
        let logical_x = px_x / dpi_scale;
        let logical_y = px_y / dpi_scale;
        // Add window offset
        let screen_x = window_x + logical_x;
        let screen_y = window_y + logical_y;
        (screen_x, screen_y)
    };

    match action {
        "click_at" => {
            let x = get_f64("x").ok_or("click_at requires x coordinate")?;
            let y = get_f64("y").ok_or("click_at requires y coordinate")?;
            let (screen_x, screen_y) = convert_coord(x, y);
            info!(
                "[computer_use] click_at ({}, {}) -> screen ({}, {})",
                x, y, screen_x, screen_y
            );
            desktop
                .click_at_coordinates(screen_x, screen_y)
                .map_err(|e| format!("Click failed: {e}"))?;
        }
        "type_text_at" => {
            let x = get_f64("x").ok_or("type_text_at requires x coordinate")?;
            let y = get_f64("y").ok_or("type_text_at requires y coordinate")?;
            let text = get_str("text").ok_or("type_text_at requires text")?;
            let press_enter = get_bool("press_enter").unwrap_or(false);
            let (screen_x, screen_y) = convert_coord(x, y);
            info!(
                "[computer_use] type_text_at ({}, {}) -> screen ({}, {}), text: {}",
                x, y, screen_x, screen_y, text
            );
            // Click first to focus
            desktop
                .click_at_coordinates(screen_x, screen_y)
                .map_err(|e| format!("Click before type failed: {e}"))?;
            tokio::time::sleep(Duration::from_millis(100)).await;
            // Select all (Ctrl+A) to clear existing text before typing
            desktop
                .press_key("{Ctrl}a")
                .await
                .map_err(|e| format!("Select all failed: {e}"))?;
            tokio::time::sleep(Duration::from_millis(50)).await;
            // Type text using root element (this will replace selected text)
            let root = desktop.root();
            root.type_text(text, false)
                .map_err(|e| format!("Type text failed: {e}"))?;
            // Press Enter if requested
            if press_enter {
                tokio::time::sleep(Duration::from_millis(50)).await;
                desktop
                    .press_key("{Enter}")
                    .await
                    .map_err(|e| format!("Press Enter failed: {e}"))?;
            }
        }
        "key_combination" => {
            let keys = get_str("keys").ok_or("key_combination requires keys")?;
            let translated = translate_gemini_keys(keys)?;
            info!("[computer_use] key_combination: {} -> {}", keys, translated);
            desktop
                .press_key(&translated)
                .await
                .map_err(|e| format!("Key press failed: {e}"))?;
        }
        "scroll_document" | "scroll_at" => {
            let direction = get_str("direction").ok_or("scroll requires direction")?;
            let magnitude = get_f64("magnitude").unwrap_or(3.0);
            let amount: f64 = match direction {
                "up" => -magnitude,
                "down" => magnitude,
                "left" => -magnitude,
                "right" => magnitude,
                _ => magnitude,
            };
            info!("[computer_use] scroll: {} (amount: {})", direction, amount);
            // If coordinates provided, click there first to focus
            if let (Some(x), Some(y)) = (get_f64("x"), get_f64("y")) {
                let (screen_x, screen_y) = convert_coord(x, y);
                desktop
                    .click_at_coordinates(screen_x, screen_y)
                    .map_err(|e| format!("Click before scroll failed: {e}"))?;
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            // Scroll using root element
            let root = desktop.root();
            root.scroll(direction, amount)
                .map_err(|e| format!("Scroll failed: {e}"))?;
        }
        "drag_and_drop" => {
            let start_x = get_f64("x")
                .or(get_f64("start_x"))
                .ok_or("drag_and_drop requires x/start_x")?;
            let start_y = get_f64("y")
                .or(get_f64("start_y"))
                .ok_or("drag_and_drop requires y/start_y")?;
            let end_x = get_f64("destination_x")
                .or(get_f64("end_x"))
                .ok_or("drag_and_drop requires destination_x/end_x")?;
            let end_y = get_f64("destination_y")
                .or(get_f64("end_y"))
                .ok_or("drag_and_drop requires destination_y/end_y")?;
            let (start_screen_x, start_screen_y) = convert_coord(start_x, start_y);
            let (end_screen_x, end_screen_y) = convert_coord(end_x, end_y);
            info!(
                "[computer_use] drag_and_drop from ({}, {}) to ({}, {})",
                start_screen_x, start_screen_y, end_screen_x, end_screen_y
            );
            let root = desktop.root();
            root.mouse_drag(start_screen_x, start_screen_y, end_screen_x, end_screen_y)
                .map_err(|e| format!("Drag failed: {e}"))?;
        }
        "wait_5_seconds" => {
            info!("[computer_use] waiting 5 seconds");
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        "hover_at" => {
            let x = get_f64("x").ok_or("hover_at requires x coordinate")?;
            let y = get_f64("y").ok_or("hover_at requires y coordinate")?;
            let (screen_x, screen_y) = convert_coord(x, y);
            info!(
                "[computer_use] hover_at ({}, {}) -> screen ({}, {})",
                x, y, screen_x, screen_y
            );
            let root = desktop.root();
            root.mouse_move(screen_x, screen_y)
                .map_err(|e| format!("Mouse move failed: {e}"))?;
        }
        "navigate" => {
            let url = get_str("url").ok_or("navigate requires url")?;
            info!("[computer_use] navigate to: {}", url);
            desktop
                .open_url(url, None)
                .map_err(|e| format!("Navigate failed: {e}"))?;
        }
        "search" => {
            let query = get_str("query")
                .or_else(|| get_str("text"))
                .or_else(|| get_str("q"))
                .unwrap_or("");
            info!("[computer_use] search: {}", query);
            if query.is_empty() {
                desktop
                    .press_key("{Enter}")
                    .await
                    .map_err(|e| format!("Press Enter for search failed: {e}"))?;
            } else {
                let encoded_query: String = query
                    .chars()
                    .map(|c| match c {
                        ' ' => "+".to_string(),
                        'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
                        _ => format!("%{:02X}", c as u32),
                    })
                    .collect();
                let search_url = format!("https://www.google.com/search?q={}", encoded_query);
                desktop
                    .open_url(&search_url, None)
                    .map_err(|e| format!("Open search URL failed: {e}"))?;
            }
        }
        _ => {
            warn!("[computer_use] Unknown action: {}", action);
            return Err(format!("Unknown action: {action}"));
        }
    }

    Ok(())
}

// ===== Main Implementation =====

impl Desktop {
    /// Run Gemini Computer Use agentic loop.
    ///
    /// Provide a goal and target process, and this will autonomously take actions
    /// (click, type, scroll, etc.) until the goal is achieved or max_steps is reached.
    ///
    /// # Arguments
    /// * `process` - Process name of the target application (e.g., "chrome", "notepad")
    /// * `goal` - What to achieve (e.g., "Open Notepad and type Hello World")
    /// * `max_steps` - Maximum number of steps before stopping (default: 20)
    /// * `on_step` - Optional callback for progress updates
    ///
    /// # Returns
    /// * `Ok(ComputerUseResult)` - Result with status, steps executed, and history
    /// * `Err(e)` - If the operation fails
    ///
    /// # Example
    /// ```ignore
    /// let desktop = Desktop::new(false, true)?;
    /// let result = desktop.gemini_computer_use(
    ///     "notepad",
    ///     "Type 'Hello World' and save the file",
    ///     Some(10),
    ///     Some(Box::new(|step| println!("Step {}: {}", step.step, step.action))),
    /// ).await?;
    /// println!("Status: {}", result.status);
    /// ```
    pub async fn gemini_computer_use(
        &self,
        process: &str,
        goal: &str,
        max_steps: Option<u32>,
        on_step: Option<ProgressCallback>,
    ) -> Result<ComputerUseResult> {
        let max_steps = max_steps.unwrap_or(20);
        let mut previous_actions: Vec<ComputerUsePreviousAction> = Vec::new();
        let mut steps: Vec<ComputerUseStep> = Vec::new();
        let mut final_status = "max_steps_reached";
        let mut final_action = String::new();
        let mut final_text: Option<String> = None;
        let mut pending_confirmation: Option<serde_json::Value> = None;

        info!(
            "[computer_use] Starting agentic loop for goal: {} (max_steps: {})",
            goal, max_steps
        );

        for step_num in 1..=max_steps {
            info!("[computer_use] Step {}/{}", step_num, max_steps);

            // 1. Capture screenshot of target window
            let capture_data = match capture_window_for_computer_use(self, process) {
                Ok(data) => data,
                Err(e) => {
                    warn!("[computer_use] Failed to capture screenshot: {}", e);
                    final_status = "failed";
                    break;
                }
            };

            // 2. Call backend to get next action
            let response = match call_computer_use_backend(
                &capture_data.base64_image,
                goal,
                if previous_actions.is_empty() {
                    None
                } else {
                    Some(&previous_actions)
                },
            )
            .await
            {
                Ok(r) => r,
                Err(e) => {
                    warn!("[computer_use] Backend error: {}", e);
                    final_status = "failed";
                    break;
                }
            };

            // Store text response
            if response.text.is_some() {
                final_text = response.text.clone();
            }

            // 3. Check for task completion
            if response.completed {
                final_status = "success";
                final_action = "completed".to_string();
                info!("[computer_use] Task completed. Text: {:?}", response.text);
                break;
            }

            // 4. Get function call
            let function_call = match response.function_call {
                Some(fc) => fc,
                None => {
                    final_status = "success";
                    final_action = "no_action".to_string();
                    break;
                }
            };

            final_action = function_call.name.clone();
            info!(
                "[computer_use] Action: {} (text: {:?})",
                function_call.name, response.text
            );

            // 5. Check for safety confirmation
            if response.safety_decision.as_deref() == Some("require_confirmation") {
                final_status = "needs_confirmation";
                pending_confirmation = Some(serde_json::json!({
                    "action": function_call.name,
                    "args": function_call.args,
                    "text": response.text,
                }));
                break;
            }

            // 6. Execute action
            let execute_result = execute_action(
                self,
                &function_call.name,
                &function_call.args,
                capture_data.window_bounds,
                capture_data.dpi_scale,
                capture_data.resize_scale,
            )
            .await;

            // 7. Record action result
            let (success, error_msg) = match &execute_result {
                Ok(_) => (true, None),
                Err(e) => (false, Some(e.to_string())),
            };

            let step = ComputerUseStep {
                step: step_num,
                action: function_call.name.clone(),
                args: function_call.args.clone(),
                success,
                error: error_msg.clone(),
                text: response.text.clone(),
            };

            // Call progress callback if provided
            if let Some(ref callback) = on_step {
                callback(&step);
            }

            steps.push(step);

            // 8. Wait for UI to settle before capturing post-action screenshot
            // This is critical for actions that cause page navigation (e.g., press Enter on search)
            tokio::time::sleep(Duration::from_millis(1000)).await;

            // 9. Capture new screenshot after action for next iteration
            let (post_action_screenshot, post_action_url) =
                match capture_window_for_computer_use(self, process) {
                    Ok(data) => (data.base64_image, data.browser_url),
                    Err(_) => (capture_data.base64_image.clone(), None),
                };

            previous_actions.push(ComputerUsePreviousAction {
                name: function_call.name,
                response: ComputerUseActionResponse {
                    success,
                    error: error_msg,
                },
                screenshot: post_action_screenshot,
                url: post_action_url,
            });

            // 10. Limit previous_actions to last 3 to avoid payload too large errors
            if previous_actions.len() > 3 {
                previous_actions.remove(0);
            }
        }

        info!(
            "[computer_use] Completed with status: {} ({} steps)",
            final_status,
            steps.len()
        );

        Ok(ComputerUseResult {
            status: final_status.to_string(),
            goal: goal.to_string(),
            steps_executed: steps.len() as u32,
            final_action,
            final_text,
            steps,
            pending_confirmation,
        })
    }
}
