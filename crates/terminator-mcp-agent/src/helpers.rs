use crate::expression_eval;
use crate::mcp_types::TreeOutputFormat;
use crate::tree_formatter::{format_tree_as_compact_yaml, format_ui_node_as_compact_yaml};
use crate::utils::ToolCall;
use regex::Regex;
use rmcp::ErrorData as McpError;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use terminator::{AutomationError, Desktop, Selector, UIElement};

/// Helper function to parse comma-separated alternative selectors into a Vec<String>
pub fn parse_alternative_selectors(alternatives: Option<&str>) -> Vec<String> {
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
pub fn get_selectors_tried(primary: &str, alternatives: Option<&str>) -> Vec<String> {
    let mut all = vec![primary.to_string()];
    all.extend(parse_alternative_selectors(alternatives));
    all
}

/// Returns all selectors tried, including primary, alternatives, and fallback selectors.
pub fn get_selectors_tried_all(
    primary: &str,
    alternatives: Option<&str>,
    fallback: Option<&str>,
) -> Vec<String> {
    let mut all = get_selectors_tried(primary, alternatives);
    all.extend(parse_alternative_selectors(fallback));
    all
}

/// Builds a standardized JSON object with detailed information about a UIElement.
/// This includes a suggested selector that prioritizes role|name over just the ID.
pub fn build_element_info(element: &UIElement) -> Value {
    let id = element.id().unwrap_or_default();
    let role = element.role();
    let name = element.name().unwrap_or_default();

    let suggested_selector = if !name.is_empty() && role != "Unknown" {
        format!("{}|{}", &role, &name)
    } else {
        format!("#{id}")
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
pub fn build_element_not_found_error(
    primary_selector: &str,
    alternatives: Option<&str>,
    fallback: Option<&str>,
    original_error: anyhow::Error,
) -> McpError {
    // Check if the underlying error is UIAutomationAPIError
    if let Some(AutomationError::UIAutomationAPIError {
        message,
        com_error,
        operation,
        is_retryable,
    }) = original_error.downcast_ref::<AutomationError>()
    {
        let error_details = json!({
            "error_type": "ui_automation_api_failure",
            "message": format!("Windows UI Automation API failure: {}", message),
            "com_error": com_error,
            "operation": operation,
            "is_retryable": is_retryable,
            "selector": primary_selector,
            "suggestion": if *is_retryable {
                "This is likely a transient Windows API error. Retry usually succeeds."
            } else {
                "Check if the application is responding and Windows UI Automation is working."
            }
        });

        return McpError::invalid_params("Windows UI Automation API failure", Some(error_details));
    }

    let selectors_tried = get_selectors_tried_all(primary_selector, alternatives, fallback);
    let error_payload = json!({
        "error_type": "ElementNotFound",
        "message": format!("The specified element could not be found after trying all selectors. Original error: {}", original_error),
        "selectors_tried": selectors_tried,
        "suggestions": [
            "Call `get_window_tree` again to get a fresh view of the UI; it might have changed.",
            "Verify the element's 'name' and 'role' in the new UI tree. The 'name' attribute might be empty or different from the visible text.",
            "If the element has no 'name', use its numeric ID selector (e.g., '#12345'). This is required for many clickable 'Group' elements.",
            "Use `validate_element` (which never throws errors) to debug existence issues, or check if the element is conditionally rendered and may not always be present."
        ]
    });

    McpError::invalid_params("Element not found", Some(error_payload))
}

/// Substitutes `{{variable}}` placeholders in a JSON value.
pub fn substitute_variables(args: &mut Value, variables: &Value) {
    use tracing::debug;

    match args {
        Value::Object(map) => {
            for (key, value) in map {
                debug!("Processing object key: {}", key);
                substitute_variables(value, variables);
            }
        }
        Value::Array(arr) => {
            for (i, value) in arr.iter_mut().enumerate() {
                debug!("Processing array index: {}", i);
                substitute_variables(value, variables);
            }
        }
        Value::String(s) => {
            debug!("Processing string: '{}'", s);
            // This regex finds all occurrences of {{...}} and ${{...}} non-greedily.
            // It supports the traditional `{{variable}}` style as well as the GitHub Actions
            // style `${{ variable }}` by making the leading `$` optional.
            // Examples matched:
            //   "{{my_var}}"
            //   "${{my_var}}"
            //   "role:Button|name:${{button_name}}"
            let re = Regex::new(r"\$?\{\{(.*?)\}\}").unwrap();

            // Handle full string replacement first, e.g., args is "{{my_var}}" or an expression.
            if let Some(caps) = re.captures(s) {
                if caps.get(0).unwrap().as_str() == s {
                    let inner_str = caps.get(1).unwrap().as_str().trim();
                    debug!(
                        "Found full string placeholder: '{}' with inner: '{}'",
                        s, inner_str
                    );

                    // Check if it's a simple variable path.
                    let is_simple_var = inner_str
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.');

                    if is_simple_var {
                        let pointer = format!("/{}", inner_str.replace('.', "/"));
                        debug!("Looking up simple variable with pointer: '{}'", pointer);
                        if let Some(replacement_val) = variables.pointer(&pointer) {
                            debug!("Found replacement value: {}", replacement_val);
                            *args = replacement_val.clone();
                        } else {
                            debug!("Variable '{}' not found in context", inner_str);
                        }
                        // If variable is not found, leave the placeholder as is.
                        return;
                    }

                    // Check if it looks like an expression we should evaluate.
                    let is_expression = inner_str.contains('(')
                        || inner_str.contains("==")
                        || inner_str.contains("!=")
                        || inner_str.contains("contains")
                        || inner_str.contains("startsWith")
                        || inner_str.contains("endsWith");

                    if is_expression {
                        debug!("Evaluating expression: '{}'", inner_str);
                        let eval_result = expression_eval::evaluate(inner_str, variables);
                        debug!("Expression result: {}", eval_result);
                        *args = Value::Bool(eval_result);
                    }
                    // If it's not a simple variable and not a recognized expression, leave it as is.
                    return;
                }
            }

            // Handle partial replacement within a larger string.
            let original_s = s.clone();
            let new_s = re
                .replace_all(s, |caps: &regex::Captures| {
                    // Because the regex allows an optional leading `$`, the capture group index
                    // for the inner contents remains at 1 regardless of whether the `$` is
                    // present. We therefore consistently pull out capture 1 here.
                    let inner_str = caps.get(1).unwrap().as_str().trim();
                    debug!(
                        "Found partial placeholder: '{}' with inner: '{}'",
                        caps.get(0).unwrap().as_str(),
                        inner_str
                    );

                    let is_simple_var = inner_str
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.');

                    if is_simple_var {
                        let pointer = format!("/{}", inner_str.replace('.', "/"));
                        debug!("Looking up simple variable with pointer: '{}'", pointer);
                        if let Some(val) = variables.pointer(&pointer) {
                            if val.is_string() {
                                debug!("Found string replacement: '{}'", val.as_str().unwrap());
                                val.as_str().unwrap().to_string()
                            } else {
                                debug!("Found non-string replacement: '{}'", val);
                                val.to_string()
                            }
                        } else {
                            debug!("Variable '{}' not found in context", inner_str);
                            // Variable not found, keep original placeholder.
                            caps.get(0).unwrap().as_str().to_string()
                        }
                    } else {
                        // Not a simple variable, assume it's either an expression or text to be ignored.
                        let is_expression = inner_str.contains('(')
                            || inner_str.contains("==")
                            || inner_str.contains("!=")
                            || inner_str.contains("contains")
                            || inner_str.contains("startsWith")
                            || inner_str.contains("endsWith");

                        if is_expression {
                            debug!("Evaluating partial expression: '{}'", inner_str);
                            let bool_val = expression_eval::evaluate(inner_str, variables);
                            debug!("Expression result: {}", bool_val);
                            bool_val.to_string()
                        } else {
                            debug!("Unknown placeholder type: '{}'", inner_str);
                            // Not a known expression type, keep original placeholder.
                            caps.get(0).unwrap().as_str().to_string()
                        }
                    }
                })
                .to_string();

            if original_s != new_s {
                debug!("String substitution: '{}' -> '{}'", original_s, new_s);
            }
            *s = new_s;
        }
        _ => {} // Other types are left as is
    }
}

/// Waits for a detectable UI change after an action, like an element disappearing or focus shifting.
/// This is more efficient than a fixed sleep, as it returns as soon as a change is detected.
pub async fn wait_for_ui_change(
    desktop: &Desktop,
    original_element_id: &str,
    timeout: Duration,
) -> String {
    let start = tokio::time::Instant::now();

    // If the element has no unique ID, we cannot reliably track it.
    // In this case, we fall back to a brief, fixed delay.
    if original_element_id.is_empty() {
        tokio::time::sleep(Duration::from_millis(150)).await;
        return "untracked_element_clicked_fixed_delay".to_string();
    }

    let original_selector = Selector::from(format!("#{original_element_id}").as_str());

    while start.elapsed() < timeout {
        // Check 1: Did focus change? This is often the quickest indicator.
        if let Ok(focused_element) = desktop.focused_element() {
            if focused_element.id_or_empty() != original_element_id {
                return format!("focus_changed_to: #{}", focused_element.id_or_empty());
            }
        }

        // Check 2: Did the original element disappear? (e.g., a dialog closed)
        if desktop
            .locator(original_selector.clone())
            .first(Some(Duration::from_millis(20)))
            .await
            .is_err()
        {
            return "element_disappeared".to_string();
        }

        // Yield to the scheduler and wait before the next poll.
        tokio::time::sleep(Duration::from_millis(30)).await;
    }

    "no_significant_change_detected".to_string()
}

// Helper methods for export_workflow_sequence
pub fn generate_step_description(tool_name: &str, args: &Value) -> String {
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
        _ => format!("Execute {tool_name}"),
    }
}

pub fn get_wait_condition(tool_name: &str) -> Option<String> {
    match tool_name {
        "click_element" => Some("Element state changes or UI updates".to_string()),
        "type_into_element" => Some("Text appears in field".to_string()),
        "navigate_browser" => Some("Page loads completely".to_string()),
        "open_application" => Some("Application window appears".to_string()),
        _ => None,
    }
}

pub fn extract_required_tools(tool_calls: &[ToolCall]) -> Vec<String> {
    tool_calls
        .iter()
        .map(|tc| tc.tool_name.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect()
}

pub fn infer_expected_outcomes(tool_calls: &[ToolCall]) -> Vec<String> {
    let mut outcomes = Vec::new();

    for call in tool_calls {
        match call.tool_name.as_str() {
            "navigate_browser" => outcomes.push("Target webpage loaded successfully".to_string()),
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

/// Type alias for UIA bounds cache
pub type UiaBoundsCache = HashMap<u32, (String, String, (f64, f64, f64, f64))>;

// Helper to optionally attach UI tree to response
// Returns the UIA bounds cache if tree was formatted in CompactYaml mode
#[allow(clippy::too_many_arguments)]
pub async fn maybe_attach_tree(
    desktop: &Desktop,
    include_tree_after_action: bool,
    tree_max_depth: Option<usize>,
    tree_from_selector: Option<&str>,
    include_detailed_attributes: Option<bool>,
    tree_output_format: Option<TreeOutputFormat>,
    pid_opt: Option<u32>,
    result_json: &mut Value,
    found_element: Option<&terminator::UIElement>,
    include_all_bounds: bool,
) -> Option<UiaBoundsCache> {
    use std::time::Duration;
    use terminator::Selector;

    // Check if tree should be included
    if !include_tree_after_action {
        return None;
    }

    // Add delay for UI to stabilize (same as ui_diff_before_after mode)
    // This ensures we capture the tree after animations/transitions complete
    tokio::time::sleep(Duration::from_millis(1500)).await;

    // Only proceed if we have a PID
    let pid = match pid_opt {
        Some(p) => p,
        None => return None,
    };

    // Build tree config with max_depth and other options
    let detailed = include_detailed_attributes.unwrap_or(true);

    let tree_config = terminator::platforms::TreeBuildConfig {
        property_mode: if detailed {
            terminator::platforms::PropertyLoadingMode::Complete
        } else {
            terminator::platforms::PropertyLoadingMode::Fast
        },
        timeout_per_operation_ms: Some(100),
        yield_every_n_elements: Some(25),
        batch_size: Some(25),
        max_depth: tree_max_depth,
        include_all_bounds,
    };

    // Determine output format (default to CompactYaml)
    let format = tree_output_format.unwrap_or(TreeOutputFormat::CompactYaml);

    // Track the bounds cache to return
    let mut bounds_cache: Option<UiaBoundsCache> = None;

    // Helper function to format tree based on output format
    // Returns (json_value, Option<bounds_cache>)
    let format_tree = |tree: terminator::element::SerializableUIElement| -> (Result<Value, String>, Option<UiaBoundsCache>) {
        match format {
            TreeOutputFormat::CompactYaml => {
                let result = format_tree_as_compact_yaml(&tree, 0);
                (Ok(json!(result.formatted)), Some(result.index_to_bounds))
            }
            TreeOutputFormat::VerboseJson => (serde_json::to_value(tree).map_err(|e| e.to_string()), None),
        }
    };

    // Handle from_selector logic
    if let Some(from_selector_value) = tree_from_selector {
        if from_selector_value == "true" {
            // Backward compatibility: use the found_element if available
            if let Some(element) = found_element {
                let max_depth = tree_max_depth.unwrap_or(100);
                let subtree = element.to_serializable_tree(max_depth);
                let (tree_result, cache) = format_tree(subtree);
                if let Ok(tree_val) = tree_result {
                    if let Some(obj) = result_json.as_object_mut() {
                        obj.insert("ui_tree".to_string(), tree_val);
                        obj.insert("tree_type".to_string(), json!("subtree"));
                    }
                    bounds_cache = cache;
                }
                return bounds_cache;
            }
        } else {
            // New behavior: treat from_selector as an actual selector string
            let selector = Selector::from(from_selector_value);
            let locator = desktop.locator(selector);

            match locator.first(Some(Duration::from_millis(1000))).await {
                Ok(from_element) => {
                    // Build tree from this different element
                    let max_depth = tree_max_depth.unwrap_or(100);
                    let subtree = from_element.to_serializable_tree(max_depth);
                    let (tree_result, cache) = format_tree(subtree);
                    if let Ok(tree_val) = tree_result {
                        if let Some(obj) = result_json.as_object_mut() {
                            obj.insert("ui_tree".to_string(), tree_val);
                            obj.insert("tree_type".to_string(), json!("subtree"));
                            obj.insert(
                                "from_selector_used".to_string(),
                                json!(from_selector_value),
                            );
                        }
                        bounds_cache = cache;
                    }
                    return bounds_cache;
                }
                Err(e) => {
                    // Log warning and return with error info
                    tracing::warn!("from_selector '{}' not found: {}", from_selector_value, e);
                    // Add error information to result
                    if let Some(obj) = result_json.as_object_mut() {
                        obj.insert(
                            "tree_error".to_string(),
                            json!(format!(
                                "from_selector '{}' not found: {}",
                                from_selector_value, e
                            )),
                        );
                        obj.insert("tree_type".to_string(), json!("none"));
                    }
                    return None;
                }
            }
        }
    }

    // Default: get the full window tree
    if let Ok(tree) = desktop.get_window_tree(pid, None, Some(tree_config)) {
        // Format UINode based on output format
        let (tree_val_result, cache) = match format {
            TreeOutputFormat::CompactYaml => {
                // Convert UINode to SerializableUIElement and use compact formatter
                let result = format_ui_node_as_compact_yaml(&tree, 0);
                (Ok(json!(result.formatted)), Some(result.index_to_bounds))
            }
            TreeOutputFormat::VerboseJson => (serde_json::to_value(tree), None),
        };

        if let Ok(tree_val) = tree_val_result {
            if let Some(obj) = result_json.as_object_mut() {
                obj.insert("ui_tree".to_string(), tree_val);
                obj.insert("tree_type".to_string(), json!("full_window"));
            }
            bounds_cache = cache;
        }
    }

    bounds_cache
}

/// Result structure for UI tree diff computation
#[derive(Debug, Clone)]
pub struct UiDiffResult {
    pub diff: String,
    pub tree_before: String,
    pub tree_after: String,
    pub has_changes: bool,
}

/// Helper to format tree based on output format
fn format_tree_string(tree: &terminator::UINode, format: TreeOutputFormat) -> String {
    match format {
        TreeOutputFormat::CompactYaml => format_ui_node_as_compact_yaml(tree, 0).formatted,
        TreeOutputFormat::VerboseJson => {
            serde_json::to_string_pretty(tree).unwrap_or_else(|_| "{}".to_string())
        }
    }
}

/// Execute an action and optionally capture before/after UI tree diff
///
/// This function wraps action execution to optionally capture UI state before and after,
/// computing a diff that shows what changed. Useful for workflow recording and verification.
#[allow(clippy::too_many_arguments)]
pub async fn execute_with_ui_diff<F, Fut>(
    desktop: &Desktop,
    action: F,
    ui_diff_before_after: bool,
    pid: u32,
    tree_max_depth: Option<usize>,
    include_detailed_attributes: Option<bool>,
    tree_output_format: TreeOutputFormat,
) -> Result<(Value, Option<UiDiffResult>), String>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<Value, String>>,
{
    if !ui_diff_before_after {
        // Normal execution without diff
        let result = action().await?;
        return Ok((result, None));
    }

    // Build tree config with max_depth and other options
    let detailed = include_detailed_attributes.unwrap_or(true);
    let tree_config = terminator::platforms::TreeBuildConfig {
        property_mode: if detailed {
            terminator::platforms::PropertyLoadingMode::Complete
        } else {
            terminator::platforms::PropertyLoadingMode::Fast
        },
        timeout_per_operation_ms: Some(100),
        yield_every_n_elements: Some(25),
        batch_size: Some(25),
        max_depth: tree_max_depth,
        include_all_bounds: false,
    };

    // Capture BEFORE tree
    tracing::debug!("Capturing UI tree before action (PID: {})", pid);
    let tree_before = desktop
        .get_window_tree(pid, None, Some(tree_config.clone()))
        .map_err(|e| format!("Failed to capture tree before action: {e}"))?;
    let before_str = format_tree_string(&tree_before, tree_output_format);

    // Execute action
    let result = action().await?;

    // Small delay for UI to settle (150ms similar to mediar-app)
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Capture AFTER tree
    tracing::debug!("Capturing UI tree after action (PID: {})", pid);
    let tree_after = desktop
        .get_window_tree(pid, None, Some(tree_config))
        .map_err(|e| format!("Failed to capture tree after action: {e}"))?;
    let after_str = format_tree_string(&tree_after, tree_output_format);

    // Compute diff using the ui_tree_diff module
    let diff_result = match crate::ui_tree_diff::simple_ui_tree_diff(&before_str, &after_str) {
        Ok(Some(diff)) => {
            tracing::info!("UI changes detected: {} characters in diff", diff.len());
            UiDiffResult {
                diff,
                tree_before: before_str,
                tree_after: after_str,
                has_changes: true,
            }
        }
        Ok(None) => {
            tracing::debug!("No UI changes detected");
            UiDiffResult {
                diff: "No UI changes detected".to_string(),
                tree_before: before_str,
                tree_after: after_str,
                has_changes: false,
            }
        }
        Err(e) => return Err(format!("Failed to compute UI diff: {e}")),
    };

    Ok((result, Some(diff_result)))
}

/// Find element and execute action with optional UI diff capture
///
/// This is a wrapper around find_and_execute_with_retry_with_fallback that adds UI diff support.
/// When ui_diff_before_after is true, it captures the UI tree before and after the action,
/// then computes the diff showing what changed.
///
/// Returns: ((action_result, element), successful_selector, optional_ui_diff)
#[allow(clippy::too_many_arguments)]
pub async fn find_and_execute_with_ui_diff<F, Fut, T>(
    desktop: &Desktop,
    primary_selector: &str,
    alternatives: Option<&str>,
    fallback_selectors: Option<&str>,
    timeout_ms: Option<u64>,
    retries: Option<u32>,
    action: F,
    ui_diff_before_after: bool,
    tree_max_depth: Option<usize>,
    include_detailed_attributes: Option<bool>,
    tree_output_format: TreeOutputFormat,
) -> Result<((T, UIElement), String, Option<UiDiffResult>), anyhow::Error>
where
    F: Fn(UIElement) -> Fut,
    Fut: std::future::Future<Output = Result<T, AutomationError>>,
{
    use crate::utils::find_element_with_fallbacks;

    // If diff not requested, use standard path
    if !ui_diff_before_after {
        let ((result, element), selector) =
            crate::utils::find_and_execute_with_retry_with_fallback(
                desktop,
                primary_selector,
                alternatives,
                fallback_selectors,
                timeout_ms,
                retries,
                action,
            )
            .await?;
        return Ok(((result, element), selector, None));
    }

    // UI diff requested - we need to capture trees before/after
    tracing::debug!(
        "[ui_diff] UI diff capture enabled for selector: {}",
        primary_selector
    );

    let retry_count = retries.unwrap_or(0);
    let mut last_error: Option<anyhow::Error> = None;

    for attempt in 0..=retry_count {
        // Find the element first to get PID
        match find_element_with_fallbacks(
            desktop,
            primary_selector,
            alternatives,
            fallback_selectors,
            timeout_ms,
        )
        .await
        {
            Ok((element, successful_selector)) => {
                // Get PID for tree capture
                let pid = element.process_id().unwrap_or(0);
                if pid == 0 {
                    tracing::warn!(
                        "[ui_diff] Could not get PID from element, skipping diff capture"
                    );
                    // Fall back to executing without diff
                    match action(element.clone()).await {
                        Ok(result) => return Ok(((result, element), successful_selector, None)),
                        Err(e) => {
                            last_error = Some(e.into());
                            if attempt < retry_count {
                                tracing::warn!(
                                    "[ui_diff] Action failed on attempt {}/{}. Retrying... Error: {}",
                                    attempt + 1,
                                    retry_count + 1,
                                    last_error.as_ref().unwrap()
                                );
                                tokio::time::sleep(Duration::from_millis(250)).await;
                                continue;
                            }
                        }
                    }
                } else {
                    // Build tree config
                    let detailed = include_detailed_attributes.unwrap_or(true);
                    let tree_config = terminator::platforms::TreeBuildConfig {
                        property_mode: if detailed {
                            terminator::platforms::PropertyLoadingMode::Complete
                        } else {
                            terminator::platforms::PropertyLoadingMode::Fast
                        },
                        timeout_per_operation_ms: Some(100),
                        yield_every_n_elements: Some(25),
                        batch_size: Some(25),
                        max_depth: tree_max_depth,
                        include_all_bounds: false,
                    };

                    // Capture BEFORE tree
                    tracing::debug!("[ui_diff] Capturing UI tree before action (PID: {})", pid);
                    let tree_before = match desktop.get_window_tree(
                        pid,
                        None,
                        Some(tree_config.clone()),
                    ) {
                        Ok(tree) => tree,
                        Err(e) => {
                            tracing::warn!("[ui_diff] Failed to capture tree before action: {}. Continuing without diff.", e);
                            // Execute action without diff
                            match action(element.clone()).await {
                                Ok(result) => {
                                    return Ok(((result, element), successful_selector, None))
                                }
                                Err(e) => {
                                    last_error = Some(e.into());
                                    if attempt < retry_count {
                                        tracing::warn!(
                                            "[ui_diff] Action failed on attempt {}/{}. Retrying... Error: {}",
                                            attempt + 1,
                                            retry_count + 1,
                                            last_error.as_ref().unwrap()
                                        );
                                        tokio::time::sleep(Duration::from_millis(250)).await;
                                        continue;
                                    }
                                }
                            }
                            continue;
                        }
                    };
                    // Clone tree_output_format to avoid move issues in retry loop
                    let before_str = format_tree_string(&tree_before, tree_output_format);

                    // Execute action
                    match action(element.clone()).await {
                        Ok(result) => {
                            // Small delay for UI to settle (1500ms - same delay used in maybe_attach_tree)
                            tokio::time::sleep(Duration::from_millis(1500)).await;

                            // Capture AFTER tree
                            tracing::debug!(
                                "[ui_diff] Capturing UI tree after action (PID: {})",
                                pid
                            );
                            let tree_after = match desktop.get_window_tree(
                                pid,
                                None,
                                Some(tree_config),
                            ) {
                                Ok(tree) => tree,
                                Err(e) => {
                                    tracing::warn!("[ui_diff] Failed to capture tree after action: {}. Returning result without diff.", e);
                                    return Ok(((result, element), successful_selector, None));
                                }
                            };
                            // Clone tree_output_format to avoid move issues in retry loop
                            let after_str = format_tree_string(&tree_after, tree_output_format);

                            // Compute diff using the ui_tree_diff module
                            let diff_result = match crate::ui_tree_diff::simple_ui_tree_diff(
                                &before_str,
                                &after_str,
                            ) {
                                Ok(Some(diff)) => {
                                    tracing::info!(
                                        "[ui_diff] UI changes detected: {} characters in diff",
                                        diff.len()
                                    );
                                    Some(UiDiffResult {
                                        diff,
                                        tree_before: before_str,
                                        tree_after: after_str,
                                        has_changes: true,
                                    })
                                }
                                Ok(None) => {
                                    tracing::debug!("[ui_diff] No UI changes detected");
                                    Some(UiDiffResult {
                                        diff: "No UI changes detected".to_string(),
                                        tree_before: before_str,
                                        tree_after: after_str,
                                        has_changes: false,
                                    })
                                }
                                Err(e) => {
                                    tracing::warn!("[ui_diff] Failed to compute UI diff: {}. Returning result without diff.", e);
                                    None
                                }
                            };

                            return Ok(((result, element), successful_selector, diff_result));
                        }
                        Err(e) => {
                            last_error = Some(e.into());
                            if attempt < retry_count {
                                tracing::warn!(
                                    "[ui_diff] Action failed on attempt {}/{}. Retrying... Error: {}",
                                    attempt + 1,
                                    retry_count + 1,
                                    last_error.as_ref().unwrap()
                                );
                                tokio::time::sleep(Duration::from_millis(250)).await;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                last_error = Some(e.into());
                if attempt < retry_count {
                    tracing::warn!(
                        "[ui_diff] Find element failed on attempt {}/{}. Retrying... Error: {}",
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

pub fn should_add_focus_check(tool_calls: &[ToolCall], current_index: usize) -> bool {
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
            | "get_applications_and_windows_list"
            | "activate_element"
    )
}

pub fn is_state_changing_action(tool_name: &str) -> bool {
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

pub fn should_capture_tree(tool_name: &str, index: usize, total_steps: usize) -> bool {
    // Capture tree at key points:
    // 1. After major navigation
    // 2. Before complex sequences
    // 3. At regular intervals (every 5 steps)
    // 4. Before the final action

    matches!(tool_name, "navigate_browser" | "open_application")
        || index.is_multiple_of(5)
        || index == total_steps - 1
}

/// Result of post-action verification
#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub passed: bool,
    pub method: String,
    pub details: String,
    pub elapsed_ms: u64,
}

/// Perform post-action verification using the new general-purpose verification system.
///
/// This function handles:
/// 1. Fast path: Try reading the element directly (no tree walking)
/// 2. Window-scoped search: Re-locate element within its window
/// 3. Fallback: Desktop-wide search if window search fails
///
/// # Arguments
/// * `desktop` - The Desktop instance
/// * `element` - The element the action was performed on
/// * `verify_exists_selector` - Optional selector that should exist after action
/// * `verify_not_exists_selector` - Optional selector that should NOT exist after action
/// * `verify_timeout_ms` - Timeout for verification polling
/// * `successful_selector` - The selector used to find the original element
///
/// # Returns
/// Ok(VerificationResult) if verification passed, Err if failed
pub async fn verify_post_action(
    desktop: &Desktop,
    element: &UIElement,
    verify_exists_selector: Option<&str>,
    verify_not_exists_selector: Option<&str>,
    verify_timeout_ms: u64,
    _successful_selector: &str,
) -> Result<VerificationResult, anyhow::Error> {
    use terminator::Selector;

    let start = tokio::time::Instant::now();

    // Handle verify_element_exists
    if let Some(exists_selector) = verify_exists_selector {
        tracing::debug!("[verify] Checking element exists: {}", exists_selector);

        let mut verification_element = None;
        let mut method = "unknown";

        // OPTIMIZATION: Try window-scoped search using PID-based window lookup
        // This is the standard pattern used throughout the codebase (tree capture, ui_diff, etc.)
        match element.application() {
            Ok(Some(app_window)) => {
                tracing::debug!(
                    "[verify] Got application window via PID (name={}, role={})",
                    app_window.name().unwrap_or_default(),
                    app_window.role()
                );

                // Create locator scoped to the application window using .within()
                let locator = desktop
                    .locator(Selector::from(exists_selector))
                    .within(app_window);

                match locator
                    .wait(Some(Duration::from_millis(verify_timeout_ms)))
                    .await
                {
                    Ok(found_element) => {
                        tracing::debug!("[verify] Window-scoped search SUCCESS");
                        method = "window_scoped_search";
                        verification_element = Some(found_element);
                    }
                    Err(e) => {
                        tracing::warn!(
                            "[verify] Window-scoped search failed: {}, will try desktop-wide",
                            e
                        );
                    }
                }
            }
            Ok(None) => {
                tracing::debug!(
                    "[verify] element.application() returned None, will use desktop-wide search"
                );
            }
            Err(e) => {
                tracing::warn!(
                    "[verify] Failed to get application by PID: {}, will use desktop-wide search",
                    e
                );
            }
        }

        // Fallback: Desktop-wide search if window search failed or window not available
        if verification_element.is_none() {
            tracing::info!("[verify] Trying desktop-wide search as fallback");
            method = "desktop_wide_search";

            let locator = desktop.locator(Selector::from(exists_selector));
            if let Ok(found_element) = locator
                .wait(Some(Duration::from_millis(verify_timeout_ms)))
                .await
            {
                verification_element = Some(found_element);
            }
        }

        if verification_element.is_none() {
            return Err(anyhow::anyhow!(
                "Verification failed: expected element '{}' not found after {}ms",
                exists_selector,
                start.elapsed().as_millis()
            ));
        }

        return Ok(VerificationResult {
            passed: true,
            method: method.to_string(),
            details: format!("Element '{exists_selector}' found"),
            elapsed_ms: start.elapsed().as_millis() as u64,
        });
    }

    // Handle verify_element_not_exists
    if let Some(not_exists_selector) = verify_not_exists_selector {
        tracing::debug!(
            "[verify] Checking element does NOT exist: {}",
            not_exists_selector
        );

        let mut method = "desktop_wide_search";

        // Try window-scoped search using PID-based window lookup
        let search_result = match element.application() {
            Ok(Some(app_window)) => {
                tracing::debug!("[verify] Got application window via PID for NOT_EXISTS check");
                method = "window_scoped_search";

                let locator = desktop
                    .locator(Selector::from(not_exists_selector))
                    .within(app_window);

                locator
                    .wait(Some(Duration::from_millis(verify_timeout_ms)))
                    .await
            }
            Ok(None) | Err(_) => {
                tracing::debug!(
                    "[verify] PID-based window unavailable, using desktop-wide NOT_EXISTS check"
                );

                let locator = desktop.locator(Selector::from(not_exists_selector));
                locator
                    .wait(Some(Duration::from_millis(verify_timeout_ms)))
                    .await
            }
        };

        // Check the result - we WANT this to fail (element should NOT exist)
        match search_result {
            Ok(_) => {
                // Element found - this is a verification failure!
                return Err(anyhow::anyhow!(
                    "Verification failed: element '{}' should not exist but was found",
                    not_exists_selector
                ));
            }
            Err(_) => {
                // Element not found - this is what we wanted!
                return Ok(VerificationResult {
                    passed: true,
                    method: method.to_string(),
                    details: format!("Element '{not_exists_selector}' correctly not present"),
                    elapsed_ms: start.elapsed().as_millis() as u64,
                });
            }
        }
    }

    // No verification requested
    Ok(VerificationResult {
        passed: true,
        method: "none".to_string(),
        details: "No verification requested".to_string(),
        elapsed_ms: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_substitute_simple_string_variable() {
        let mut args = json!({"url": "{{url}}"});
        let vars = json!({"url": "http://example.com"});
        substitute_variables(&mut args, &vars);
        assert_eq!(args["url"], "http://example.com");
    }

    #[test]
    fn test_substitute_nested_variable() {
        let mut args = json!({"selector": "{{selectors.my_button}}"});
        let vars = json!({"selectors": {"my_button": "role:Button|name:Click Me"}});
        substitute_variables(&mut args, &vars);
        assert_eq!(args["selector"], "role:Button|name:Click Me");
    }

    #[test]
    fn test_substitute_variable_in_string() {
        let mut args = json!({"selector": "role:RadioButton|name:{{gender}}"});
        let vars = json!({"gender": "Male"});
        substitute_variables(&mut args, &vars);
        assert_eq!(args["selector"], "role:RadioButton|name:Male");
    }

    #[test]
    fn test_substitute_non_existent_variable() {
        let mut args = json!({"selector": "{{non_existent}}"});
        let vars = json!({});
        substitute_variables(&mut args, &vars);
        assert_eq!(args["selector"], "{{non_existent}}");
    }

    #[test]
    fn test_substitute_variable_with_hyphen() {
        let mut args = json!({"value": "{{a-b-c}}"});
        let vars = json!({"a-b-c": "test-value"});
        substitute_variables(&mut args, &vars);
        assert_eq!(args["value"], "test-value");
    }

    #[test]
    fn test_substitute_partial_with_number() {
        let mut args = json!({"value": "timeout_{{timeout_ms}}"});
        let vars = json!({"timeout_ms": 5000});
        substitute_variables(&mut args, &vars);
        assert_eq!(args["value"], "timeout_5000");
    }

    #[test]
    fn test_substitute_simple_variable() {
        let mut args = json!({"state": "{{desired_state}}"});
        let vars = json!({"desired_state": true});
        substitute_variables(&mut args, &vars);
        assert_eq!(args["state"], true);
    }

    #[test]
    fn test_substitute_expression_true() {
        let mut args = json!({"state": "{{contains(product_types, 'FEX')}}"});
        let vars = json!({"product_types": ["FEX", "Term"]});
        substitute_variables(&mut args, &vars);
        assert_eq!(args["state"], true);
    }

    #[test]
    fn test_substitute_expression_false() {
        let mut args = json!({"state": "{{startsWith(name, 'Jane')}}"});
        let vars = json!({"name": "John Doe"});
        substitute_variables(&mut args, &vars);
        assert_eq!(args["state"], false);
    }

    #[test]
    fn test_substitute_equality_expression() {
        let mut args = json!({"enabled": "{{quote_type == 'Face Amount'}}"});
        let vars = json!({"quote_type": "Face Amount"});
        substitute_variables(&mut args, &vars);
        assert_eq!(args["enabled"], true);
    }

    #[test]
    fn test_substitute_github_actions_style_variable() {
        let mut args = json!({"url": "${{target_url}}"});
        let vars = json!({"target_url": "https://github.com"});
        substitute_variables(&mut args, &vars);
        assert_eq!(args["url"], "https://github.com");
    }

    #[test]
    fn test_substitute_github_actions_style_partial() {
        let mut args = json!({"selector": "role:Button|name:${{button_name}}"});
        let vars = json!({"button_name": "Submit"});
        substitute_variables(&mut args, &vars);
        assert_eq!(args["selector"], "role:Button|name:Submit");
    }

    #[test]
    fn test_substitute_github_actions_style_expression() {
        let mut args = json!({"enabled": "${{quote_type == 'Face Amount'}}"});
        let vars = json!({"quote_type": "Face Amount"});
        substitute_variables(&mut args, &vars);
        assert_eq!(args["enabled"], true);
    }

    #[test]
    fn test_substitute_equality_expression_false() {
        let mut args = json!({"enabled": "{{quote_type == 'Monthly'}}"});
        let vars = json!({"quote_type": "Face Amount"});
        substitute_variables(&mut args, &vars);
        assert_eq!(args["enabled"], false);
    }

    #[test]
    fn test_substitute_in_complex_workflow() {
        let mut args = json!({
            "steps": [
                {
                    "tool_name": "navigate_browser",
                    "arguments": {
                        "url": "{{url}}"
                    }
                },
                {
                    "tool_name": "maximize_window",
                    "arguments": {
                        "selector": "{{selectors.browser_window}}"
                    }
                },
                {
                    "tool_name": "set_selected",
                    "arguments": {
                        "selector": "role:RadioButton|name:{{applicant_gender}}",
                        "state": true
                    }
                },
                {
                    "tool_name": "set_toggled",
                    "arguments": {
                        "selector": "{{selectors.fex_checkbox_checked}}",
                        "state": "{{contains(product_types, 'FEX')}}"
                    },
                    "continue_on_error": true
                },
                {
                    "tool_name": "set_toggled",
                    "arguments": {
                        "selector": "{{selectors.medsup_checkbox_checked}}",
                        "state": "{{contains(product_types, 'MedSup')}}"
                    },
                    "continue_on_error": true
                },
                {
                    "group_name": "Enter Quote Value (Face Amount)",
                    "if": "quote_type == 'Face Amount'",
                    "skippable": false,
                    "steps": [
                        {
                            "tool_name": "click_element",
                            "arguments": {
                                "selector": "{{selectors.face_value_toggle}}",
                                "timeout_ms": 1000
                            }
                        }
                    ]
                },
                {
                    "tool_name": "unfindable_test",
                    "arguments": {
                        "selector": "{{selectors.not_real}}"
                    }
                }
            ]
        });

        // Use anonymized data for the test
        let vars = json!({
            "url": "https://example-insurance-quote.com/",
            "applicant_gender": "Female",
            "product_types": [
                "FEX",
                "Term"
            ],
            "quote_type": "Face Amount",
            "selectors": {
                "browser_window": "role:Window|name:Insurance Quoting App",
                "fex_checkbox_checked": "role:CheckBox|name:FEX",
                "medsup_checkbox_checked": "role:CheckBox|name:MedSup",
                "face_value_toggle": "role:Text|name:Face Value"
            }
        });

        substitute_variables(&mut args, &vars);

        // Step 0: navigate_browser
        assert_eq!(
            args["steps"][0]["arguments"]["url"],
            "https://example-insurance-quote.com/"
        );

        // Step 1: maximize_window
        assert_eq!(
            args["steps"][1]["arguments"]["selector"],
            "role:Window|name:Insurance Quoting App"
        );

        // Step 2: set_selected
        assert_eq!(
            args["steps"][2]["arguments"]["selector"],
            "role:RadioButton|name:Female"
        );

        // Step 3: set_toggled for FEX (expression -> true)
        assert_eq!(
            args["steps"][3]["arguments"]["selector"],
            "role:CheckBox|name:FEX"
        );
        assert_eq!(args["steps"][3]["arguments"]["state"], true);

        // Step 4: set_toggled for MedSup (expression -> false)
        assert_eq!(
            args["steps"][4]["arguments"]["selector"],
            "role:CheckBox|name:MedSup"
        );
        assert_eq!(args["steps"][4]["arguments"]["state"], false);

        // Step 5: group arguments should be substituted
        assert_eq!(
            args["steps"][5]["steps"][0]["arguments"]["selector"],
            "role:Text|name:Face Value"
        );
        // The 'if' condition itself is not a {{...}} placeholder, so it should not be changed.
        assert_eq!(args["steps"][5]["if"], "quote_type == 'Face Amount'");

        // Step 6: non-existent variable should be left as-is
        assert_eq!(
            args["steps"][6]["arguments"]["selector"],
            "{{selectors.not_real}}"
        );
    }

    #[test]
    fn test_do_not_evaluate_free_text_as_expression() {
        let mut args = json!({"text": "{{some text}}"});
        let vars = json!({});
        substitute_variables(&mut args, &vars);
        assert_eq!(
            args["text"], "{{some text}}",
            "Should not evaluate placeholder as a boolean expression"
        );
    }

    #[test]
    fn test_substitute_in_full_user_workflow() {
        let mut args = json!({
            "steps": [
                {
                    "tool_name": "navigate_browser",
                    "arguments": {
                        "url": "{{url}}"
                    }
                },
                {
                    "tool_name": "set_value",
                    "arguments": {
                        "selector": "{{selectors.dob_field}}",
                        "value": "{{applicant_dob}}"
                    }
                },
                {
                    "tool_name": "set_selected",
                    "arguments": {
                        "selector": "role:RadioButton|name:{{applicant_gender}}",
                        "state": true
                    }
                },
                {
                    "if": "contains(product_types, 'FEX')",
                    "tool_name": "set_toggled",
                    "arguments": {
                        "selector": "{{selectors.fex_checkbox_checked}}",
                        "state": "{{contains(product_types, 'FEX')}}"
                    }
                },
                {
                    "group_name": "Enter Quote Value (Face Amount)",
                    "if": "quote_type == 'Face Amount'",
                    "steps": [
                        {
                            "tool_name": "click_element",
                            "arguments": {
                                "selector": "{{selectors.face_value_toggle}}"
                            }
                        }
                    ]
                }
            ]
        });

        let vars = json!({
            "url": "https://example.com",
            "applicant_dob": "01/01/2000",
            "applicant_gender": "Male",
            "product_types": ["FEX"],
            "quote_type": "Face Amount",
            "selectors": {
                "dob_field": "id:dob",
                "fex_checkbox_checked": "id:fex",
                "face_value_toggle": "id:face_value"
            }
        });

        substitute_variables(&mut args, &vars);

        // Check URL substitution
        assert_eq!(args["steps"][0]["arguments"]["url"], "https://example.com");

        // Check nested variable substitution
        assert_eq!(args["steps"][1]["arguments"]["selector"], "id:dob");
        assert_eq!(args["steps"][1]["arguments"]["value"], "01/01/2000");

        // Check partial substitution
        assert_eq!(
            args["steps"][2]["arguments"]["selector"],
            "role:RadioButton|name:Male"
        );

        // Check substitution inside a step that also has an `if` condition
        // The `if` condition itself should NOT be substituted as it's not a {{}} placeholder
        assert_eq!(args["steps"][3]["if"], "contains(product_types, 'FEX')");
        assert_eq!(args["steps"][3]["arguments"]["selector"], "id:fex");
        assert_eq!(args["steps"][3]["arguments"]["state"], true);

        // Check substitution within a nested step
        assert_eq!(args["steps"][4]["if"], "quote_type == 'Face Amount'");
        assert_eq!(
            args["steps"][4]["steps"][0]["arguments"]["selector"],
            "id:face_value"
        );
    }

    #[test]
    fn test_substitute_workflow_navigate_browser() {
        let mut args = json!({
            "tool_name": "navigate_browser",
            "arguments": {
                "url": "{{url}}"
            }
        });
        let vars = json!({
            "url": "https://bob.com/"
        });
        substitute_variables(&mut args, &vars);
        assert_eq!(args["arguments"]["url"], "https://bob.com/");
    }

    #[test]
    fn test_substitute_with_inputs_wrapper() {
        // Test the exact structure from the workflow
        let mut args = json!({
            "arguments": {
                "url": "{{url}}"
            }
        });
        let vars = json!({
            "url": "https://bob.com/"
        });
        substitute_variables(&mut args, &vars);
        assert_eq!(args["arguments"]["url"], "https://bob.com/");
    }

    #[test]
    fn test_exact_execution_context_structure() {
        // Test the exact structure that the server creates
        let mut tool_args = json!({
            "url": "{{url}}"
        });

        // This is how the server builds the execution context
        let inputs = json!({
            "url": "https://bob.com/"
        });
        let execution_context_map = inputs.as_object().cloned().unwrap_or_default();
        let execution_context = serde_json::Value::Object(execution_context_map);

        substitute_variables(&mut tool_args, &execution_context);
        assert_eq!(tool_args["url"], "https://bob.com/");
    }

    #[test]
    fn test_negation_preserves_original_functionality() {
        let vars = json!({
            "product_types": ["FEX", "Term"],
            "quote_type": "Face Amount",
            "enabled": true
        });

        // Ensure original functionality still works
        assert!(expression_eval::evaluate(
            "contains(product_types, 'FEX')",
            &vars
        ));
        assert!(!expression_eval::evaluate(
            "contains(product_types, 'MedSup')",
            &vars
        ));
        assert!(expression_eval::evaluate(
            "quote_type == 'Face Amount'",
            &vars
        ));
        assert!(!expression_eval::evaluate(
            "quote_type == 'Monthly Amount'",
            &vars
        ));
        assert!(expression_eval::evaluate("enabled == true", &vars));
        assert!(!expression_eval::evaluate("enabled == false", &vars));

        // And that negation works correctly
        assert!(!expression_eval::evaluate(
            "!contains(product_types, 'FEX')",
            &vars
        ));
        assert!(expression_eval::evaluate(
            "!contains(product_types, 'MedSup')",
            &vars
        ));
        assert!(!expression_eval::evaluate(
            "!quote_type == 'Face Amount'",
            &vars
        ));
        assert!(expression_eval::evaluate(
            "!quote_type == 'Monthly Amount'",
            &vars
        ));
        assert!(!expression_eval::evaluate("!enabled == true", &vars));
        assert!(expression_eval::evaluate("!enabled == false", &vars));
    }

    // Tests for negation operator in substitute_variables
    #[test]
    fn test_substitute_negation_expressions() {
        let mut args = json!({"enabled": "{{!contains(product_types, 'FEX')}}"});
        let vars = json!({"product_types": ["FEX", "Term"]});
        substitute_variables(&mut args, &vars);
        assert_eq!(args["enabled"], false); // FEX is in the array, so !contains is false

        let mut args = json!({"enabled": "{{!contains(product_types, 'MedSup')}}"});
        let vars = json!({"product_types": ["FEX", "Term"]});
        substitute_variables(&mut args, &vars);
        assert_eq!(args["enabled"], true); // MedSup is not in the array, so !contains is true
    }

    #[test]
    fn test_substitute_negation_binary_expressions() {
        let mut args = json!({"skip_step": "{{!quote_type == 'Face Amount'}}"});
        let vars = json!({"quote_type": "Face Amount"});
        substitute_variables(&mut args, &vars);
        assert_eq!(args["skip_step"], false); // Quote type is Face Amount, so !== is false

        let mut args = json!({"skip_step": "{{!quote_type == 'Monthly Amount'}}"});
        let vars = json!({"quote_type": "Face Amount"});
        substitute_variables(&mut args, &vars);
        assert_eq!(args["skip_step"], true); // Quote type is not Monthly Amount, so !== is true
    }

    #[test]
    fn test_substitute_double_negation() {
        let mut args = json!({"enabled": "{{!!contains(product_types, 'FEX')}}"});
        let vars = json!({"product_types": ["FEX", "Term"]});
        substitute_variables(&mut args, &vars);
        assert_eq!(args["enabled"], true); // !!true = true

        let mut args = json!({"enabled": "{{!!contains(product_types, 'MedSup')}}"});
        let vars = json!({"product_types": ["FEX", "Term"]});
        substitute_variables(&mut args, &vars);
        assert_eq!(args["enabled"], false); // !!false = false
    }

    #[test]
    fn test_substitute_negation_with_whitespace() {
        let mut args = json!({"enabled": "{{! contains(product_types, 'MedSup')}}"});
        let vars = json!({"product_types": ["FEX", "Term"]});
        substitute_variables(&mut args, &vars);
        assert_eq!(args["enabled"], true);

        let mut args = json!({"enabled": "{{  !  contains(product_types, 'MedSup')  }}"});
        let vars = json!({"product_types": ["FEX", "Term"]});
        substitute_variables(&mut args, &vars);
        assert_eq!(args["enabled"], true);
    }

    #[test]
    fn test_substitute_negation_in_workflow_context() {
        let mut args = json!({
            "steps": [
                {
                    "group_name": "Uncheck FEX if not needed",
                    "if": "!contains(product_types, 'FEX')",
                    "steps": [
                        {
                            "tool_name": "set_toggled",
                            "arguments": {
                                "selector": "{{selectors.fex_checkbox}}",
                                "state": false
                            }
                        }
                    ]
                },
                {
                    "tool_name": "set_toggled",
                    "arguments": {
                        "selector": "{{selectors.medsup_checkbox}}",
                        "state": "{{!contains(product_types, 'MedSup')}}"
                    }
                }
            ]
        });

        let vars = json!({
            "product_types": ["FEX", "Term"],
            "selectors": {
                "fex_checkbox": "role:CheckBox|name:FEX",
                "medsup_checkbox": "role:CheckBox|name:MedSup"
            }
        });

        substitute_variables(&mut args, &vars);

        // The if condition should remain as text (not substituted)
        assert_eq!(args["steps"][0]["if"], "!contains(product_types, 'FEX')");

        // The selector should be substituted
        assert_eq!(
            args["steps"][0]["steps"][0]["arguments"]["selector"],
            "role:CheckBox|name:FEX"
        );

        // The negation expression should be evaluated to true (MedSup not in product_types)
        assert_eq!(args["steps"][1]["arguments"]["state"], true);
        assert_eq!(
            args["steps"][1]["arguments"]["selector"],
            "role:CheckBox|name:MedSup"
        );
    }

    #[test]
    fn test_substitute_complex_negation_scenarios() {
        let mut args = json!({
            "conditional_steps": [
                {
                    "enabled": "{{!startsWith(user_name, 'Admin')}}"
                },
                {
                    "enabled": "{{!endsWith(email, '@test.com')}}"
                },
                {
                    "enabled": "{{!contains(roles, 'SuperUser')}}"
                }
            ]
        });

        let vars = json!({
            "user_name": "John Doe",
            "email": "john@example.com",
            "roles": ["User", "Editor"]
        });

        substitute_variables(&mut args, &vars);

        assert_eq!(args["conditional_steps"][0]["enabled"], true); // Doesn't start with Admin
        assert_eq!(args["conditional_steps"][1]["enabled"], true); // Doesn't end with @test.com
        assert_eq!(args["conditional_steps"][2]["enabled"], true); // Doesn't contain SuperUser
    }

    #[test]
    fn test_substitute_negation_edge_cases() {
        // Test with missing variables
        let mut args = json!({"enabled": "{{!contains(missing_var, 'value')}}"});
        let vars = json!({});
        substitute_variables(&mut args, &vars);
        assert_eq!(args["enabled"], true); // Missing variable defaults to false, !false = true

        // Test with empty arrays
        let mut args = json!({"enabled": "{{!contains(empty_array, 'anything')}}"});
        let vars = json!({"empty_array": []});
        substitute_variables(&mut args, &vars);
        assert_eq!(args["enabled"], true); // Empty array doesn't contain anything, !false = true

        // Test with null values
        let mut args = json!({"enabled": "{{!null_value == 'test'}}"});
        let vars = json!({"null_value": null});
        substitute_variables(&mut args, &vars);
        assert_eq!(args["enabled"], true); // Null comparison fails, !false = true
    }

    #[test]
    fn test_substitute_realistic_workflow_with_negation() {
        // Test a realistic workflow scenario using negation
        let mut args = json!({
            "steps": [
                {
                    "group_name": "Uncheck unwanted product types",
                    "steps": [
                        {
                            "tool_name": "set_toggled",
                            "arguments": {
                                "selector": "{{selectors.fex_checkbox}}",
                                "state": "{{!contains(unwanted_products, 'FEX')}}"
                            }
                        },
                        {
                            "tool_name": "set_toggled",
                            "arguments": {
                                "selector": "{{selectors.medsup_checkbox}}",
                                "state": "{{!contains(unwanted_products, 'MedSup')}}"
                            }
                        }
                    ]
                },
                {
                    "group_name": "Skip premium users",
                    "if": "!contains(user_roles, 'Premium')",
                    "steps": [
                        {
                            "tool_name": "click_element",
                            "arguments": {
                                "selector": "{{selectors.basic_plan_button}}"
                            }
                        }
                    ]
                }
            ]
        });

        let vars = json!({
            "unwanted_products": ["MedSup", "Preneed"],
            "user_roles": ["Basic", "User"],
            "selectors": {
                "fex_checkbox": "role:CheckBox|name:FEX",
                "medsup_checkbox": "role:CheckBox|name:MedSup",
                "basic_plan_button": "role:Button|name:Basic Plan"
            }
        });

        substitute_variables(&mut args, &vars);

        // FEX should be enabled (not in unwanted_products)
        assert_eq!(args["steps"][0]["steps"][0]["arguments"]["state"], true);

        // MedSup should be disabled (in unwanted_products)
        assert_eq!(args["steps"][0]["steps"][1]["arguments"]["state"], false);

        // Selectors should be substituted correctly
        assert_eq!(
            args["steps"][0]["steps"][0]["arguments"]["selector"],
            "role:CheckBox|name:FEX"
        );
        assert_eq!(
            args["steps"][0]["steps"][1]["arguments"]["selector"],
            "role:CheckBox|name:MedSup"
        );
        assert_eq!(
            args["steps"][1]["steps"][0]["arguments"]["selector"],
            "role:Button|name:Basic Plan"
        );

        // The if condition should remain as text
        assert_eq!(args["steps"][1]["if"], "!contains(user_roles, 'Premium')");
    }
}
