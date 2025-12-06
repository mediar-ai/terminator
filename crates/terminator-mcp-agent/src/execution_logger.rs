//! MCP Tool Execution Logger
//!
//! Logs all MCP tool requests and responses to flat files in %LOCALAPPDATA%\mediar\executions\
//! with associated before/after screenshots. 7-day retention with automatic cleanup.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{debug, error, info, warn};

/// Whether execution logging is enabled (can be disabled via env var)
static LOGGING_ENABLED: AtomicBool = AtomicBool::new(true);

/// Retention period in days
const RETENTION_DAYS: i64 = 7;

/// Execution log entry combining request and response
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutionLog {
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_index: Option<usize>,
    pub tool_name: String,
    pub request: Value,
    pub response: ExecutionResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshots: Option<ScreenshotRefs>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutionResponse {
    pub status: String,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScreenshotRefs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub after: Vec<String>,
}

/// Context for logging an execution (passed between request and response logging)
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub timestamp: chrono::DateTime<Local>,
    pub workflow_id: Option<String>,
    pub step_id: Option<String>,
    pub step_index: Option<usize>,
    pub tool_name: String,
    pub request: Value,
    pub file_prefix: String,
}

/// Get the executions directory path
pub fn get_executions_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("terminator")
        .join("executions")
}

/// Initialize execution logging (create dir, check env var, run cleanup)
pub fn init() {
    // Check if logging is disabled via env var
    if std::env::var("TERMINATOR_DISABLE_EXECUTION_LOGS")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false)
    {
        LOGGING_ENABLED.store(false, Ordering::Relaxed);
        info!("[execution_logger] Execution logging disabled via TERMINATOR_DISABLE_EXECUTION_LOGS");
        return;
    }

    let dir = get_executions_dir();
    if let Err(e) = fs::create_dir_all(&dir) {
        error!("[execution_logger] Failed to create executions dir: {}", e);
        LOGGING_ENABLED.store(false, Ordering::Relaxed);
        return;
    }

    info!("[execution_logger] Execution logs will be written to: {}", dir.display());

    // Run cleanup in background
    tokio::spawn(async {
        cleanup_old_executions().await;
    });
}

/// Check if logging is enabled
pub fn is_enabled() -> bool {
    LOGGING_ENABLED.load(Ordering::Relaxed)
}

/// Generate file prefix: YYYYMMDD_HHMMSS_workflowId_toolName
fn generate_file_prefix(
    timestamp: &chrono::DateTime<Local>,
    workflow_id: Option<&str>,
    tool_name: &str,
) -> String {
    let date_time = timestamp.format("%Y%m%d_%H%M%S").to_string();
    let wf_id = workflow_id.unwrap_or("standalone");
    // Sanitize tool name (remove mcp__ prefix if present)
    let clean_tool = tool_name
        .strip_prefix("mcp__terminator-mcp-agent__")
        .unwrap_or(tool_name);
    format!("{}_{}_{}",date_time, wf_id, clean_tool)
}

/// Start logging an execution (call before tool dispatch)
/// Returns context to pass to log_response
pub fn log_request(
    tool_name: &str,
    arguments: &Value,
    workflow_id: Option<&str>,
    step_id: Option<&str>,
    step_index: Option<usize>,
) -> Option<ExecutionContext> {
    if !is_enabled() {
        return None;
    }

    let timestamp = Local::now();
    let file_prefix = generate_file_prefix(&timestamp, workflow_id, tool_name);

    Some(ExecutionContext {
        timestamp,
        workflow_id: workflow_id.map(String::from),
        step_id: step_id.map(String::from),
        step_index,
        tool_name: tool_name.to_string(),
        request: arguments.clone(),
        file_prefix,
    })
}

/// Complete logging an execution (call after tool dispatch)
pub fn log_response(
    ctx: ExecutionContext,
    result: Result<&Value, &str>,
    duration_ms: u64,
) {
    if !is_enabled() {
        return;
    }

    let dir = get_executions_dir();
    let json_path = dir.join(format!("{}.json", ctx.file_prefix));

    // Extract screenshots from result and save them
    let screenshots = if let Ok(result_value) = result {
        extract_and_save_screenshots(&dir, &ctx.file_prefix, result_value)
    } else {
        None
    };

    // Build response, stripping screenshot base64 from result
    let clean_result = result.ok().map(|v| strip_screenshot_base64(v));

    let log = ExecutionLog {
        timestamp: ctx.timestamp.to_rfc3339(),
        workflow_id: ctx.workflow_id,
        step_id: ctx.step_id,
        step_index: ctx.step_index,
        tool_name: ctx.tool_name.clone(),
        request: ctx.request,
        response: ExecutionResponse {
            status: if result.is_ok() { "success" } else { "error" }.to_string(),
            duration_ms,
            result: clean_result,
            error: result.err().map(String::from),
        },
        screenshots,
    };

    // Write JSON
    match serde_json::to_string_pretty(&log) {
        Ok(json) => {
            if let Err(e) = fs::write(&json_path, json) {
                warn!("[execution_logger] Failed to write {}: {}", json_path.display(), e);
            } else {
                debug!("[execution_logger] Logged: {}", json_path.display());
            }
        }
        Err(e) => {
            warn!("[execution_logger] Failed to serialize log: {}", e);
        }
    }
}

/// Extract screenshots from result and save as PNG files
/// Returns screenshot references for the JSON
fn extract_and_save_screenshots(
    dir: &PathBuf,
    file_prefix: &str,
    result: &Value,
) -> Option<ScreenshotRefs> {
    let mut refs = ScreenshotRefs {
        before: None,
        after: Vec::new(),
    };
    let mut screenshot_counter = 0usize;

    // Look for screenshot in various locations in the result
    // Common patterns: result.screenshot, result.screenshot_before, result.screenshot_after
    // Also in content array: content[].screenshot, content[].image

    // Try direct screenshot field (usually "after" or single screenshot)
    if let Some(screenshot) = extract_base64_image(result, &["screenshot", "image", "screenshot_base64"]) {
        let filename = format!("{}_after.png", file_prefix);
        if save_screenshot(dir, &filename, &screenshot) {
            refs.after.push(filename);
            screenshot_counter += 1;
        }
    }

    // Try screenshot_before
    if let Some(screenshot) = extract_base64_image(result, &["screenshot_before", "before_screenshot"]) {
        let filename = format!("{}_before.png", file_prefix);
        if save_screenshot(dir, &filename, &screenshot) {
            refs.before = Some(filename);
        }
    }

    // Try screenshot_after (explicit) - only if no screenshots saved yet
    if refs.after.is_empty() {
        if let Some(screenshot) = extract_base64_image(result, &["screenshot_after", "after_screenshot"]) {
            let filename = format!("{}_after.png", file_prefix);
            if save_screenshot(dir, &filename, &screenshot) {
                refs.after.push(filename);
                screenshot_counter += 1;
            }
        }
    }

    // Check in content array (MCP response format) - save ALL images
    if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
        for item in content {
            // Image content type
            if item.get("type").and_then(|t| t.as_str()) == Some("image") {
                if let Some(data) = item.get("data").and_then(|d| d.as_str()) {
                    // Generate unique filename for each screenshot
                    let filename = if screenshot_counter == 0 {
                        format!("{}_after.png", file_prefix)
                    } else {
                        format!("{}_after_{}.png", file_prefix, screenshot_counter)
                    };
                    if save_screenshot(dir, &filename, data) {
                        refs.after.push(filename);
                        screenshot_counter += 1;
                    }
                }
            }
            // Text content with embedded screenshot
            if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                    if let Some(screenshot) = extract_base64_image(&parsed, &["screenshot", "image"]) {
                        let filename = if screenshot_counter == 0 {
                            format!("{}_after.png", file_prefix)
                        } else {
                            format!("{}_after_{}.png", file_prefix, screenshot_counter)
                        };
                        if save_screenshot(dir, &filename, &screenshot) {
                            refs.after.push(filename);
                            screenshot_counter += 1;
                        }
                    }
                }
            }
        }
    }

    if refs.before.is_some() || !refs.after.is_empty() {
        Some(refs)
    } else {
        None
    }
}

/// Extract base64 image from value by trying multiple field names
fn extract_base64_image(value: &Value, field_names: &[&str]) -> Option<String> {
    for name in field_names {
        if let Some(img) = value.get(*name).and_then(|v| v.as_str()) {
            // Check if it looks like base64 image data (minimum ~80 chars for minimal 1x1 PNG)
            if img.len() >= 80 && (img.starts_with("iVBOR") || img.starts_with("/9j/") || img.contains("base64,")) {
                // Strip data URL prefix if present
                let data = if let Some(pos) = img.find("base64,") {
                    &img[pos + 7..]
                } else {
                    img
                };
                return Some(data.to_string());
            }
        }
    }
    None
}

/// Save base64 screenshot as PNG file
fn save_screenshot(dir: &PathBuf, filename: &str, base64_data: &str) -> bool {
    match BASE64.decode(base64_data.trim()) {
        Ok(bytes) => {
            let path = dir.join(filename);
            match fs::write(&path, bytes) {
                Ok(_) => {
                    debug!("[execution_logger] Saved screenshot: {}", filename);
                    true
                }
                Err(e) => {
                    warn!("[execution_logger] Failed to save screenshot {}: {}", filename, e);
                    false
                }
            }
        }
        Err(e) => {
            warn!("[execution_logger] Failed to decode screenshot base64: {}", e);
            false
        }
    }
}

/// Strip screenshot base64 data from result to keep JSON small
fn strip_screenshot_base64(value: &Value) -> Value {
    let mut result = value.clone();

    // Fields to strip
    let screenshot_fields = [
        "screenshot",
        "screenshot_base64",
        "screenshot_before",
        "screenshot_after",
        "before_screenshot",
        "after_screenshot",
        "image",
    ];

    if let Some(obj) = result.as_object_mut() {
        for field in &screenshot_fields {
            if obj.contains_key(*field) {
                obj.insert(field.to_string(), Value::String("[extracted to file]".to_string()));
            }
        }
    }

    // Also strip from content array
    if let Some(content) = result.get_mut("content").and_then(|c| c.as_array_mut()) {
        for item in content.iter_mut() {
            if item.get("type").and_then(|t| t.as_str()) == Some("image") {
                if let Some(obj) = item.as_object_mut() {
                    obj.insert("data".to_string(), Value::String("[extracted to file]".to_string()));
                }
            }
            // Handle text content with embedded JSON
            if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                if let Ok(mut parsed) = serde_json::from_str::<Value>(text) {
                    let mut modified = false;
                    if let Some(obj) = parsed.as_object_mut() {
                        for field in &screenshot_fields {
                            if obj.contains_key(*field) {
                                obj.insert(field.to_string(), Value::String("[extracted to file]".to_string()));
                                modified = true;
                            }
                        }
                    }
                    if modified {
                        if let Some(obj) = item.as_object_mut() {
                            if let Ok(new_text) = serde_json::to_string(&parsed) {
                                obj.insert("text".to_string(), Value::String(new_text));
                            }
                        }
                    }
                }
            }
        }
    }

    result
}

/// Clean up execution logs older than RETENTION_DAYS
async fn cleanup_old_executions() {
    let dir = get_executions_dir();
    if !dir.exists() {
        return;
    }

    let cutoff_date = Local::now().date_naive() - chrono::Duration::days(RETENTION_DAYS);
    let cutoff_prefix = cutoff_date.format("%Y%m%d").to_string();

    debug!("[execution_logger] Cleaning up files older than {} (prefix < {})", cutoff_date, cutoff_prefix);

    let mut deleted_count = 0;
    let mut error_count = 0;

    match fs::read_dir(&dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let filename = entry.file_name().to_string_lossy().to_string();

                // Extract date prefix (first 8 chars: YYYYMMDD)
                if filename.len() >= 8 {
                    let file_date_prefix = &filename[..8];

                    // Compare lexicographically (works for YYYYMMDD format)
                    if file_date_prefix < cutoff_prefix.as_str() {
                        match fs::remove_file(entry.path()) {
                            Ok(_) => {
                                deleted_count += 1;
                                debug!("[execution_logger] Deleted old file: {}", filename);
                            }
                            Err(e) => {
                                error_count += 1;
                                warn!("[execution_logger] Failed to delete {}: {}", filename, e);
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            warn!("[execution_logger] Failed to read executions dir for cleanup: {}", e);
        }
    }

    if deleted_count > 0 || error_count > 0 {
        info!(
            "[execution_logger] Cleanup complete: deleted {} files, {} errors",
            deleted_count, error_count
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_generate_file_prefix() {
        let ts = Local::now();
        let prefix = generate_file_prefix(&ts, Some("198"), "click_element");
        assert!(prefix.contains("198"));
        assert!(prefix.contains("click_element"));

        let prefix_standalone = generate_file_prefix(&ts, None, "get_window_tree");
        assert!(prefix_standalone.contains("standalone"));
    }

    #[test]
    fn test_strip_screenshot_base64() {
        let value = json!({
            "status": "success",
            "screenshot": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==",
            "other_data": "keep this"
        });

        let stripped = strip_screenshot_base64(&value);
        assert_eq!(stripped["screenshot"], "[extracted to file]");
        assert_eq!(stripped["other_data"], "keep this");
    }

    #[test]
    fn test_extract_base64_image() {
        let value = json!({
            "screenshot": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="
        });

        let img = extract_base64_image(&value, &["screenshot"]);
        assert!(img.is_some());
        assert!(img.unwrap().starts_with("iVBOR"));
    }
}
