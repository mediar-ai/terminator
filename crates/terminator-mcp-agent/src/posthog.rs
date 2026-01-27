// PostHog analytics for product telemetry
// Tracks usage to help improve terminator
// Users can opt-out by setting POSTHOG_DISABLED=true

use serde_json::{json, Value};
use std::sync::OnceLock;
use tracing::{debug, warn};

/// PostHog API key (Mediar's project)
const POSTHOG_API_KEY: &str = "phc_NFSaZUao49XckpqaeyB3lIEKrFXhhXbKaI81jqZ8yn9";

/// PostHog EU endpoint
const POSTHOG_HOST: &str = "https://eu.i.posthog.com";

/// Cached distinct ID (machine-based, anonymous)
static DISTINCT_ID: OnceLock<String> = OnceLock::new();

/// Check if PostHog is disabled
fn is_disabled() -> bool {
    std::env::var("POSTHOG_DISABLED")
        .or_else(|_| std::env::var("TERMINATOR_ANALYTICS_DISABLED"))
        .unwrap_or_default()
        .eq_ignore_ascii_case("true")
}

/// Get or create a distinct ID for this machine (anonymous)
fn get_distinct_id() -> &'static str {
    DISTINCT_ID.get_or_init(|| {
        // Try to get a stable machine ID
        let hostname = hostname::get()
            .ok()
            .and_then(|h| h.to_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "unknown".to_string());

        // Hash the hostname for privacy
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        hostname.hash(&mut hasher);
        format!("terminator-{:x}", hasher.finish())
    })
}

/// Get deployment type
fn get_deployment_type() -> String {
    std::env::var("SENTRY_DEPLOYMENT_TYPE")
        .or_else(|_| std::env::var("DEPLOYMENT_TYPE"))
        .unwrap_or_else(|_| "oss".to_string())
}

/// Capture an event to PostHog (non-blocking)
pub fn capture(event: &str, properties: Value) {
    if is_disabled() {
        return;
    }

    let event = event.to_string();
    let distinct_id = get_distinct_id().to_string();
    let deployment_type = get_deployment_type();

    // Merge properties with default properties
    let mut props = match properties {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };

    // Add default properties
    props.insert("version".to_string(), json!(env!("CARGO_PKG_VERSION")));
    props.insert("os".to_string(), json!(std::env::consts::OS));
    props.insert("arch".to_string(), json!(std::env::consts::ARCH));
    props.insert("deployment_type".to_string(), json!(deployment_type));

    // Build the capture payload
    let payload = json!({
        "api_key": POSTHOG_API_KEY,
        "event": event,
        "distinct_id": distinct_id,
        "properties": props,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    // Send async to avoid blocking
    tokio::spawn(async move {
        if let Err(e) = send_event(payload).await {
            debug!("PostHog capture failed (non-critical): {}", e);
        }
    });
}

/// Send event to PostHog
async fn send_event(payload: Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    let response = client
        .post(format!("{}/capture/", POSTHOG_HOST))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await?;

    if !response.status().is_success() {
        warn!("PostHog returned status: {}", response.status());
    }

    Ok(())
}

/// Track MCP agent startup
pub fn track_startup() {
    capture(
        "mcp_agent_started",
        json!({
            "transport": std::env::var("MCP_TRANSPORT").unwrap_or_else(|_| "stdio".to_string()),
        }),
    );
}

/// Track tool execution
pub fn track_tool_execution(tool_name: &str, success: bool, duration_ms: u64, error: Option<&str>) {
    let mut props = json!({
        "tool": tool_name,
        "success": success,
        "duration_ms": duration_ms,
    });

    if let Some(err) = error {
        // Classify error type (don't send full error message for privacy)
        let error_type = classify_error(err);
        props["error_type"] = json!(error_type);
    }

    capture("tool_executed", props);
}

/// Classify error into a category (privacy-preserving)
fn classify_error(error: &str) -> &'static str {
    let lower = error.to_lowercase();
    if lower.contains("not found") || lower.contains("unable to find") {
        "element_not_found"
    } else if lower.contains("timeout") {
        "timeout"
    } else if lower.contains("permission") || lower.contains("access") {
        "permission_denied"
    } else if lower.contains("network") || lower.contains("connection") {
        "network_error"
    } else {
        "other"
    }
}

/// Track workflow/sequence execution
pub fn track_workflow_execution(total_steps: usize, success: bool, duration_ms: u64) {
    capture(
        "workflow_executed",
        json!({
            "total_steps": total_steps,
            "success": success,
            "duration_ms": duration_ms,
        }),
    );
}
