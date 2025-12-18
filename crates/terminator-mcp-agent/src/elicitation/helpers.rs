//! Helper functions for elicitation with graceful fallback
//!
//! These functions make it easy to request user input during tool execution
//! while gracefully handling clients that don't support elicitation.

use rmcp::service::{ElicitationSafe, Peer, RoleServer};

/// Elicit structured data from the user with graceful fallback
///
/// Attempts to elicit information from the user through the MCP client.
/// If the client doesn't support elicitation, or the user declines/cancels,
/// returns the provided default value.
///
/// # Example
/// ```ignore
/// use terminator_mcp_agent::elicitation::{elicit_with_fallback, WorkflowContext};
///
/// async fn my_tool(peer: &Peer<RoleServer>) {
///     let ctx = elicit_with_fallback(
///         peer,
///         "Please provide workflow context",
///         WorkflowContext::default(),
///     ).await;
///
///     println!("Business purpose: {}", ctx.business_purpose);
/// }
/// ```
pub async fn elicit_with_fallback<T>(peer: &Peer<RoleServer>, message: &str, default: T) -> T
where
    T: ElicitationSafe + serde::de::DeserializeOwned + Send + 'static,
{
    if !peer.supports_elicitation() {
        tracing::debug!("[elicitation] Client does not support elicitation: {}", message);
        return default;
    }

    match peer.elicit::<T>(message).await {
        Ok(Some(data)) => {
            tracing::info!("[elicitation] User provided data for: {}", message);
            data
        }
        Ok(None) => {
            tracing::debug!("[elicitation] User declined/cancelled: {}", message);
            default
        }
        Err(e) => {
            tracing::debug!("[elicitation] Error ({}): {}", e, message);
            default
        }
    }
}

/// Try to elicit data, returning None if not supported or declined
///
/// Similar to `elicit_with_fallback` but returns `Option<T>` instead of
/// requiring a default value. Useful when you want to know whether the
/// user actually provided input.
///
/// # Example
/// ```ignore
/// use terminator_mcp_agent::elicitation::{try_elicit, ActionConfirmation};
///
/// async fn dangerous_operation(peer: &Peer<RoleServer>) -> Result<(), Error> {
///     if let Some(confirm) = try_elicit::<ActionConfirmation>(
///         peer,
///         "This will delete all files. Are you sure?",
///     ).await {
///         if confirm.confirmed {
///             // Proceed with deletion
///         }
///     }
///     Ok(())
/// }
/// ```
pub async fn try_elicit<T>(peer: &Peer<RoleServer>, message: &str) -> Option<T>
where
    T: ElicitationSafe + serde::de::DeserializeOwned + Send + 'static,
{
    if !peer.supports_elicitation() {
        tracing::debug!("[elicitation] Client does not support elicitation: {}", message);
        return None;
    }

    match peer.elicit::<T>(message).await {
        Ok(Some(data)) => {
            tracing::info!("[elicitation] User provided data for: {}", message);
            Some(data)
        }
        Ok(None) => {
            tracing::debug!("[elicitation] User declined/cancelled: {}", message);
            None
        }
        Err(e) => {
            tracing::debug!("[elicitation] Error: {} ({})", message, e);
            None
        }
    }
}

/// Check if the client supports elicitation
///
/// Returns `true` if the connected MCP client declared elicitation support
/// during initialization. Note that even if this returns `true`, elicitation
/// calls may still fail at runtime.
pub fn supports_elicitation(peer: &Peer<RoleServer>) -> bool {
    peer.supports_elicitation()
}
