use rmcp::model::Tool;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// MCP Tool Hints for describing tool behavior
/// These are advisory hints that help clients understand tool behavior
/// but should NOT be relied upon for security decisions
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolHints {
    /// If true, the tool does not modify its environment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only_hint: Option<bool>,
    
    /// If true, the tool may perform destructive updates
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destructive_hint: Option<bool>,
    
    /// If true, repeated calls with same args have no additional effect
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotent_hint: Option<bool>,
    
    /// If true, tool interacts with external entities (network, filesystem, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_world_hint: Option<bool>,
    
    /// Additional custom hints for specific use cases
    #[serde(flatten)]
    pub custom_hints: HashMap<String, serde_json::Value>,
}

impl ToolHints {
    /// Create hints for a read-only tool
    pub fn read_only() -> Self {
        Self {
            read_only_hint: Some(true),
            destructive_hint: Some(false),
            idempotent_hint: Some(true),
            open_world_hint: None,
            custom_hints: HashMap::new(),
        }
    }
    
    /// Create hints for a destructive tool
    pub fn destructive() -> Self {
        Self {
            read_only_hint: Some(false),
            destructive_hint: Some(true),
            idempotent_hint: Some(false),
            open_world_hint: None,
            custom_hints: HashMap::new(),
        }
    }
    
    /// Create hints for a safe write operation (non-destructive modification)
    pub fn safe_write() -> Self {
        Self {
            read_only_hint: Some(false),
            destructive_hint: Some(false),
            idempotent_hint: None,
            open_world_hint: None,
            custom_hints: HashMap::new(),
        }
    }
    
    /// Create hints for an idempotent operation
    pub fn idempotent() -> Self {
        Self {
            read_only_hint: None,
            destructive_hint: None,
            idempotent_hint: Some(true),
            open_world_hint: None,
            custom_hints: HashMap::new(),
        }
    }
    
    /// Create hints for a tool that interacts with external systems
    pub fn external_interaction() -> Self {
        Self {
            read_only_hint: None,
            destructive_hint: None,
            idempotent_hint: None,
            open_world_hint: Some(true),
            custom_hints: HashMap::new(),
        }
    }
    
    /// Builder pattern for chaining hint configurations
    pub fn with_read_only(mut self, value: bool) -> Self {
        self.read_only_hint = Some(value);
        self
    }
    
    pub fn with_destructive(mut self, value: bool) -> Self {
        self.destructive_hint = Some(value);
        self
    }
    
    pub fn with_idempotent(mut self, value: bool) -> Self {
        self.idempotent_hint = Some(value);
        self
    }
    
    pub fn with_open_world(mut self, value: bool) -> Self {
        self.open_world_hint = Some(value);
        self
    }
    
    pub fn with_custom_hint(mut self, key: String, value: serde_json::Value) -> Self {
        self.custom_hints.insert(key, value);
        self
    }
}

/// Extension trait to add hints to MCP Tool definitions
pub trait ToolWithHints {
    fn with_hints(self, hints: ToolHints) -> Self;
    fn mark_read_only(self) -> Self;
    fn mark_destructive(self) -> Self;
    fn mark_idempotent(self) -> Self;
    fn mark_external(self) -> Self;
}

impl ToolWithHints for Tool {
    fn with_hints(mut self, hints: ToolHints) -> Self {
        // Convert hints to JSON and merge into tool definition
        // Note: rmcp crate may need updates to directly support these fields
        // For now, we can add them to the input_schema or as custom metadata
        
        // Add hints as a special field in the tool's metadata
        // This approach depends on rmcp crate's support for custom fields
        // If not directly supported, consider storing in description or custom field
        
        // Temporary approach: encode hints in description
        if let Ok(hints_json) = serde_json::to_string(&hints) {
            let hint_description = format!(
                "{}\n[Hints: {}]",
                self.description.as_deref().unwrap_or(""),
                hints_json
            );
            self.description = Some(hint_description);
        }
        
        self
    }
    
    fn mark_read_only(self) -> Self {
        self.with_hints(ToolHints::read_only())
    }
    
    fn mark_destructive(self) -> Self {
        self.with_hints(ToolHints::destructive())
    }
    
    fn mark_idempotent(self) -> Self {
        self.with_hints(ToolHints::idempotent())
    }
    
    fn mark_external(self) -> Self {
        self.with_hints(ToolHints::external_interaction())
    }
}

/// Macro to define a tool with hints
#[macro_export]
macro_rules! tool_with_hints {
    (
        name: $name:expr,
        description: $desc:expr,
        hints: { $($hint_field:ident: $hint_value:expr),* $(,)? },
        input_schema: $schema:expr
    ) => {{
        use $crate::mcp_tool_hints::{ToolHints, ToolWithHints};
        
        let mut hints = ToolHints::default();
        $(
            hints.$hint_field = Some($hint_value);
        )*
        
        rmcp::model::Tool {
            name: $name.to_string(),
            title: None,
            description: Some($desc.to_string()),
            input_schema: $schema,
            output_schema: None,
        }.with_hints(hints)
    }};
}

/// Common tool categories with appropriate hints
pub mod tool_categories {
    use super::ToolHints;
    
    /// File system read operations
    pub fn file_read_hints() -> ToolHints {
        ToolHints::default()
            .with_read_only(true)
            .with_destructive(false)
            .with_idempotent(true)
            .with_open_world(true)
    }
    
    /// File system write operations
    pub fn file_write_hints() -> ToolHints {
        ToolHints::default()
            .with_read_only(false)
            .with_destructive(false)
            .with_idempotent(false)
            .with_open_world(true)
    }
    
    /// File system delete operations
    pub fn file_delete_hints() -> ToolHints {
        ToolHints::default()
            .with_read_only(false)
            .with_destructive(true)
            .with_idempotent(true)
            .with_open_world(true)
    }
    
    /// Database query operations
    pub fn db_query_hints() -> ToolHints {
        ToolHints::default()
            .with_read_only(true)
            .with_destructive(false)
            .with_idempotent(true)
            .with_open_world(true)
    }
    
    /// Database modification operations
    pub fn db_modify_hints() -> ToolHints {
        ToolHints::default()
            .with_read_only(false)
            .with_destructive(false)
            .with_idempotent(false)
            .with_open_world(true)
    }
    
    /// Network GET requests
    pub fn http_get_hints() -> ToolHints {
        ToolHints::default()
            .with_read_only(true)
            .with_destructive(false)
            .with_idempotent(true)
            .with_open_world(true)
    }
    
    /// Network POST/PUT requests
    pub fn http_mutate_hints() -> ToolHints {
        ToolHints::default()
            .with_read_only(false)
            .with_destructive(false)
            .with_idempotent(false)
            .with_open_world(true)
    }
    
    /// Process execution
    pub fn process_exec_hints() -> ToolHints {
        ToolHints::default()
            .with_read_only(false)
            .with_destructive(false)
            .with_idempotent(false)
            .with_open_world(true)
            .with_custom_hint("requires_confirmation".to_string(), serde_json::json!(true))
    }
    
    /// System configuration changes
    pub fn system_config_hints() -> ToolHints {
        ToolHints::default()
            .with_read_only(false)
            .with_destructive(true)
            .with_idempotent(false)
            .with_open_world(true)
            .with_custom_hint("requires_confirmation".to_string(), serde_json::json!(true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tool_hints_creation() {
        let read_only = ToolHints::read_only();
        assert_eq!(read_only.read_only_hint, Some(true));
        assert_eq!(read_only.destructive_hint, Some(false));
        
        let destructive = ToolHints::destructive();
        assert_eq!(destructive.read_only_hint, Some(false));
        assert_eq!(destructive.destructive_hint, Some(true));
    }
    
    #[test]
    fn test_builder_pattern() {
        let hints = ToolHints::default()
            .with_read_only(true)
            .with_idempotent(true)
            .with_custom_hint("test_hint".to_string(), serde_json::json!("test_value"));
        
        assert_eq!(hints.read_only_hint, Some(true));
        assert_eq!(hints.idempotent_hint, Some(true));
        assert_eq!(hints.custom_hints.get("test_hint"), Some(&serde_json::json!("test_value")));
    }
    
    #[test]
    fn test_serialization() {
        let hints = ToolHints {
            read_only_hint: Some(true),
            destructive_hint: Some(false),
            idempotent_hint: None,
            open_world_hint: Some(true),
            custom_hints: HashMap::new(),
        };
        
        let json = serde_json::to_string(&hints).unwrap();
        assert!(json.contains("\"read_only_hint\":true"));
        assert!(json.contains("\"destructive_hint\":false"));
        assert!(json.contains("\"open_world_hint\":true"));
        assert!(!json.contains("idempotent_hint")); // Should be skipped when None
    }
}