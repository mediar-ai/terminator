//! Element interaction tools: click, type, press_key, scroll, etc.
//!
//! This module contains helpers for UI element interaction tools.

use crate::telemetry::StepSpan;
use rmcp::ErrorData as McpError;
use serde_json::{json, Value};
use terminator::{Desktop, UIElement};

/// Build common element info JSON from a UIElement
pub fn build_element_info(element: &UIElement) -> Value {
    json!({
        "role": element.role(),
        "name": element.name(),
        "process_id": element.process_id().unwrap_or(0),
        "window_title": element.window_title(),
    })
}

/// Build base result JSON for action tools
pub fn build_action_result(action: &str, selector_used: &str, element: &UIElement) -> Value {
    json!({
        "action": action,
        "status": "success",
        "selector_used": selector_used,
        "element": build_element_info(element),
        "timestamp": chrono::Utc::now().to_rfc3339()
    })
}

/// Options for post-action verification
pub struct VerificationOptions<'a> {
    pub verify_element_exists: &'a str,
    pub verify_element_not_exists: &'a str,
    pub verify_timeout_ms: Option<u64>,
}

/// Attach UI diff result to result_json if present
pub fn attach_ui_diff_to_result(
    ui_diff: Option<crate::helpers::UiDiffResult>,
    include_full_trees: bool,
    tool_name: &str,
    result_json: &mut Value,
    span: &mut StepSpan,
) {
    if let Some(diff_result) = ui_diff {
        tracing::debug!(
            "[{}] Attaching UI diff to result (has_changes: {})",
            tool_name,
            diff_result.has_changes
        );
        span.set_attribute("ui_diff.has_changes", diff_result.has_changes.to_string());

        result_json["ui_diff"] = json!(diff_result.diff);
        result_json["has_ui_changes"] = json!(diff_result.has_changes);
        if include_full_trees {
            result_json["tree_before"] = json!(diff_result.tree_before);
            result_json["tree_after"] = json!(diff_result.tree_after);
        }
    }
}

/// Perform post-action verification and update result_json and span
///
/// Returns Ok(()) if verification passed or was skipped, Err if verification failed
pub async fn perform_post_action_verification(
    desktop: &Desktop,
    element: &UIElement,
    opts: &VerificationOptions<'_>,
    successful_selector: &str,
    tool_name: &str,
    result_json: &mut Value,
    span: &mut StepSpan,
) -> Result<(), McpError> {
    // Skip if no verification requested
    if opts.verify_element_exists.is_empty() && opts.verify_element_not_exists.is_empty() {
        return Ok(());
    }

    let verify_timeout_ms = opts.verify_timeout_ms.unwrap_or(2000);

    let verify_exists_opt = if opts.verify_element_exists.is_empty() {
        None
    } else {
        Some(opts.verify_element_exists)
    };
    let verify_not_exists_opt = if opts.verify_element_not_exists.is_empty() {
        None
    } else {
        Some(opts.verify_element_not_exists)
    };

    match crate::helpers::verify_post_action(
        desktop,
        element,
        verify_exists_opt,
        verify_not_exists_opt,
        verify_timeout_ms,
        successful_selector,
    )
    .await
    {
        Ok(verification_result) => {
            tracing::info!(
                "[{}] Verification passed: method={}, details={}",
                tool_name,
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
            Ok(())
        }
        Err(e) => {
            tracing::error!("[{}] Verification failed: {}", tool_name, e);
            span.set_attribute("verification.passed", "false".to_string());
            span.set_status(false, Some("Verification failed"));
            // Note: caller must call span.end() after receiving this error
            Err(McpError::internal_error(
                format!("Post-action verification failed: {e}"),
                Some(json!({
                    "selector_used": successful_selector,
                    "verify_exists": opts.verify_element_exists,
                    "verify_not_exists": opts.verify_element_not_exists,
                    "timeout_ms": verify_timeout_ms,
                })),
            ))
        }
    }
}
