use crate::tool_annotations::{
    ContentAnnotator, ObservabilityCollector, ObservabilityConfig, OperationType,
    ToolExecutionMetadata,
};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, RawContent, TextContent};
use rmcp::{tool, ErrorData as McpError};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info};

/// Arguments for getting execution history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetExecutionHistoryArgs {
    /// Filter by tool name (optional)
    pub tool_name: Option<String>,
    /// Filter by operation type (optional)
    pub operation_type: Option<String>,
    /// Maximum number of results to return
    pub limit: Option<usize>,
}

/// Arguments for getting active executions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetActiveExecutionsArgs {}

/// Arguments for exporting telemetry data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportTelemetryArgs {
    /// Format for export (json, csv, etc.)
    pub format: Option<String>,
}

/// Arguments for configuring observability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigureObservabilityArgs {
    pub max_stored_executions: Option<usize>,
    pub enable_performance_warnings: Option<bool>,
    pub slow_operation_threshold_ms: Option<u64>,
    pub large_payload_threshold_bytes: Option<usize>,
}

/// Observability tools wrapper
pub struct ObservabilityTools {
    collector: Arc<ObservabilityCollector>,
}

impl ObservabilityTools {
    pub fn new(collector: Arc<ObservabilityCollector>) -> Self {
        Self { collector }
    }

    /// Get execution history with optional filters
    #[tool(
        description = "Get historical data about tool executions for debugging and performance analysis"
    )]
    pub async fn get_execution_history(
        &self,
        params: Parameters<GetExecutionHistoryArgs>,
    ) -> Result<CallToolResult, McpError> {
        info!("Getting execution history with filters: {:?}", params.arguments);
        
        let mut history = self.collector.get_execution_history().await;
        
        // Apply filters
        if let Some(tool_name) = &params.arguments.tool_name {
            history.retain(|e| e.tool_name == *tool_name);
        }
        
        if let Some(op_type) = &params.arguments.operation_type {
            history.retain(|e| format!("{:?}", e.operation_type).to_lowercase() == op_type.to_lowercase());
        }
        
        // Apply limit
        if let Some(limit) = params.arguments.limit {
            history.truncate(limit);
        }
        
        let content = Content {
            raw: RawContent::Text(TextContent {
                text: serde_json::to_string_pretty(&history)
                    .unwrap_or_else(|_| "Failed to serialize history".to_string()),
            }),
            annotations: None,
        }.with_audience(vec!["assistant".to_string()])
         .with_priority(0.5);
        
        Ok(CallToolResult {
            content: vec![content],
            is_error: Some(false),
            error: None,
        })
    }

    /// Get currently active tool executions
    #[tool(
        description = "Get information about currently running tool executions for monitoring"
    )]
    pub async fn get_active_executions(
        &self,
        _params: Parameters<GetActiveExecutionsArgs>,
    ) -> Result<CallToolResult, McpError> {
        info!("Getting active executions");
        
        let active = self.collector.get_active_executions().await;
        
        let content = Content {
            raw: RawContent::Text(TextContent {
                text: serde_json::to_string_pretty(&active)
                    .unwrap_or_else(|_| "Failed to serialize active executions".to_string()),
            }),
            annotations: None,
        }.with_audience(vec!["assistant".to_string()])
         .with_priority(0.8);
        
        Ok(CallToolResult {
            content: vec![content],
            is_error: Some(false),
            error: None,
        })
    }

    /// Export complete telemetry data
    #[tool(
        description = "Export complete telemetry data including statistics and execution traces"
    )]
    pub async fn export_telemetry(
        &self,
        params: Parameters<ExportTelemetryArgs>,
    ) -> Result<CallToolResult, McpError> {
        info!("Exporting telemetry in format: {:?}", params.arguments.format);
        
        let telemetry = self.collector.export_telemetry().await;
        
        let text = match params.arguments.format.as_deref() {
            Some("csv") => {
                // Simple CSV export of execution history
                let history = self.collector.get_execution_history().await;
                let mut csv = String::from("tool_name,operation_type,status,duration_ms,input_size,output_size,trace_id\n");
                for exec in history {
                    csv.push_str(&format!(
                        "{},{:?},{:?},{},{},{},{}\n",
                        exec.tool_name,
                        exec.operation_type,
                        exec.status,
                        exec.duration_ms.unwrap_or(0),
                        exec.input_size_bytes,
                        exec.output_size_bytes.unwrap_or(0),
                        exec.trace_id
                    ));
                }
                csv
            }
            _ => {
                // Default to JSON
                serde_json::to_string_pretty(&telemetry)
                    .unwrap_or_else(|_| "Failed to serialize telemetry".to_string())
            }
        };
        
        let content = Content {
            raw: RawContent::Text(TextContent { text }),
            annotations: None,
        }.with_audience(vec!["assistant".to_string()])
         .with_priority(0.3);
        
        Ok(CallToolResult {
            content: vec![content],
            is_error: Some(false),
            error: None,
        })
    }

    /// Get workflow traces showing parent-child relationships
    #[tool(
        description = "Get workflow traces showing the relationship between parent and child tool executions"
    )]
    pub async fn get_workflow_traces(&self) -> Result<CallToolResult, McpError> {
        info!("Getting workflow traces");
        
        let history = self.collector.get_execution_history().await;
        
        // Build workflow trees
        let mut workflows: HashMap<String, Vec<ToolExecutionMetadata>> = HashMap::new();
        
        for exec in history {
            if let Some(parent_id) = &exec.parent_trace_id {
                workflows.entry(parent_id.clone())
                    .or_default()
                    .push(exec);
            } else {
                workflows.entry(exec.trace_id.clone())
                    .or_default();
            }
        }
        
        let content = Content {
            raw: RawContent::Text(TextContent {
                text: serde_json::to_string_pretty(&workflows)
                    .unwrap_or_else(|_| "Failed to serialize workflows".to_string()),
            }),
            annotations: None,
        }.with_audience(vec!["assistant".to_string()])
         .with_priority(0.4);
        
        Ok(CallToolResult {
            content: vec![content],
            is_error: Some(false),
            error: None,
        })
    }
}

/// Create an annotated wrapper for an existing tool handler
pub fn create_annotated_handler<F, Fut>(
    collector: Arc<ObservabilityCollector>,
    tool_name: String,
    operation_type: OperationType,
    handler: F,
) -> impl Fn(Parameters<serde_json::Value>) -> Fut + Clone
where
    F: Fn(Parameters<serde_json::Value>) -> Fut + Clone,
    Fut: std::future::Future<Output = Result<CallToolResult, McpError>> + Send,
{
    move |params: Parameters<serde_json::Value>| {
        let collector = collector.clone();
        let tool_name = tool_name.clone();
        let handler = handler.clone();
        
        async move {
            let input_size = serde_json::to_string(&params.arguments)
                .map(|s| s.len())
                .unwrap_or(0);
            
            let trace_id = collector
                .start_execution(tool_name.clone(), operation_type.clone(), input_size, None)
                .await;
            
            match handler(params).await {
                Ok(mut result) => {
                    let output_size = serde_json::to_string(&result)
                        .map(|s| s.len())
                        .unwrap_or(0);
                    
                    let mut annotations = HashMap::new();
                    annotations.insert("success".to_string(), json!(true));
                    annotations.insert("tool".to_string(), json!(tool_name));
                    
                    // Add metadata to result
                    if !result.content.is_empty() {
                        let metadata_content = Content {
                            raw: RawContent::Text(TextContent {
                                text: format!(
                                    "[Trace: {} | Tool: {} | Duration: pending]",
                                    trace_id, tool_name
                                ),
                            }),
                            annotations: Some(rmcp::model::Annotations {
                                audience: Some(vec!["assistant".to_string()]),
                                priority: Some(0.1),
                                cache_control: None,
                            }),
                        };
                        result.content.push(metadata_content);
                    }
                    
                    collector
                        .complete_execution(trace_id, output_size, annotations)
                        .await;
                    
                    Ok(result)
                }
                Err(e) => {
                    collector
                        .fail_execution(trace_id, e.message.clone())
                        .await;
                    Err(e)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_observability_tools() {
        let collector = Arc::new(ObservabilityCollector::new(ObservabilityConfig::default()));
        let tools = ObservabilityTools::new(collector.clone());
        
        // Simulate some executions
        let trace_id = collector
            .start_execution("test_tool".to_string(), OperationType::Read, 100, None)
            .await;
        
        collector
            .complete_execution(trace_id, 200, HashMap::new())
            .await;
        
        // Test getting history
        let params = Parameters {
            arguments: GetExecutionHistoryArgs {
                tool_name: Some("test_tool".to_string()),
                operation_type: None,
                limit: Some(10),
            },
        };
        
        let result = tools.get_execution_history(params).await;
        assert!(result.is_ok());
    }
}