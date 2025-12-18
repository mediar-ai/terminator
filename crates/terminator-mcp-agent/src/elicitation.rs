//! Elicitation schemas for MCP server

use rmcp::service::{ElicitationSafe, Peer, RoleServer};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use rmcp::elicit_safe;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[schemars(description = "Business context for workflow execution")]
pub struct WorkflowContext {
    #[schemars(description = "What is the business purpose?")]
    pub business_purpose: String,
    #[serde(default)]
    pub target_app: Option<String>,
    #[serde(default)]
    pub expected_outcome: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ElementDisambiguation {
    pub selected_index: usize,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ErrorRecoveryChoice {
    pub action: ErrorRecoveryAction,
    #[serde(default)]
    pub additional_context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum ErrorRecoveryAction {
    Retry,
    WaitLonger,
    TryAlternativeSelector,
    Skip,
    Abort,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActionConfirmation {
    pub confirmed: bool,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SelectorRefinement {
    pub element_description: String,
    #[serde(default)]
    pub element_type: Option<ElementTypeHint>,
    #[serde(default)]
    pub visible_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum ElementTypeHint {
    Button,
    TextField,
    Checkbox,
    Dropdown,
    Link,
    Menu,
    Tab,
    ListItem,
    Other,
}

elicit_safe!(WorkflowContext);
elicit_safe!(ElementDisambiguation);
elicit_safe!(ErrorRecoveryChoice);
elicit_safe!(ActionConfirmation);
elicit_safe!(SelectorRefinement);

pub async fn elicit_with_fallback<T>(peer: &Peer<RoleServer>, message: &str, default: T) -> T
where
    T: ElicitationSafe + serde::de::DeserializeOwned + Send + 'static,
{
    if !peer.supports_elicitation() {
        tracing::debug!("[elicitation] Not supported: {}", message);
        return default;
    }
    match peer.elicit::<T>(message).await {
        Ok(Some(data)) => data,
        Ok(None) => default,
        Err(e) => {
            tracing::debug!("[elicitation] Error: {}", e);
            default
        }
    }
}

pub async fn try_elicit<T>(peer: &Peer<RoleServer>, message: &str) -> Option<T>
where
    T: ElicitationSafe + serde::de::DeserializeOwned + Send + 'static,
{
    if !peer.supports_elicitation() {
        return None;
    }
    match peer.elicit::<T>(message).await {
        Ok(Some(data)) => Some(data),
        _ => None,
    }
}

pub fn supports_elicitation(peer: &Peer<RoleServer>) -> bool {
    peer.supports_elicitation()
}
