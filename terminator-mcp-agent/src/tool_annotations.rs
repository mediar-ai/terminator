use rmcp::model::{Annotations, Content, RawContent, TextContent};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Metadata for tool execution observability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionMetadata {
    pub tool_name: String,
    pub operation_type: OperationType,
    pub start_time: u64,
    pub end_time: Option<u64>,
    pub duration_ms: Option<u64>,
    pub status: ExecutionStatus,
    pub input_size_bytes: usize,
    pub output_size_bytes: Option<usize>,
    pub error: Option<String>,
    pub annotations: HashMap<String, serde_json::Value>,
    pub trace_id: String,
    pub parent_trace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationType {
    Read,
    Write,
    Execute,
    Query,
    Transform,
    Validate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Pending,
    Running,
    Success,
    Failed,
    Timeout,
    Cancelled,
}

/// Observability collector for tracking tool executions
pub struct ObservabilityCollector {
    executions: Arc<RwLock<Vec<ToolExecutionMetadata>>>,
    active_traces: Arc<RwLock<HashMap<String, ToolExecutionMetadata>>>,
    config: ObservabilityConfig,
}

#[derive(Debug, Clone)]
pub struct ObservabilityConfig {
    pub max_stored_executions: usize,
    pub enable_performance_warnings: bool,
    pub slow_operation_threshold_ms: u64,
    pub large_payload_threshold_bytes: usize,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            max_stored_executions: 1000,
            enable_performance_warnings: true,
            slow_operation_threshold_ms: 5000,
            large_payload_threshold_bytes: 1024 * 1024, // 1MB
        }
    }
}

impl ObservabilityCollector {
    pub fn new(config: ObservabilityConfig) -> Self {
        Self {
            executions: Arc::new(RwLock::new(Vec::new())),
            active_traces: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    pub async fn start_execution(
        &self,
        tool_name: String,
        operation_type: OperationType,
        input_size: usize,
        parent_trace_id: Option<String>,
    ) -> String {
        let trace_id = uuid::Uuid::new_v4().to_string();
        let metadata = ToolExecutionMetadata {
            tool_name: tool_name.clone(),
            operation_type,
            start_time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            end_time: None,
            duration_ms: None,
            status: ExecutionStatus::Running,
            input_size_bytes: input_size,
            output_size_bytes: None,
            error: None,
            annotations: HashMap::new(),
            trace_id: trace_id.clone(),
            parent_trace_id,
        };

        let mut active = self.active_traces.write().await;
        active.insert(trace_id.clone(), metadata);

        info!(
            "Started tool execution: {} ({})",
            tool_name, trace_id
        );
        trace_id
    }

    pub async fn complete_execution(
        &self,
        trace_id: String,
        output_size: usize,
        annotations: HashMap<String, serde_json::Value>,
    ) {
        let mut active = self.active_traces.write().await;
        if let Some(mut metadata) = active.remove(&trace_id) {
            let end_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            
            metadata.end_time = Some(end_time);
            metadata.duration_ms = Some(end_time - metadata.start_time);
            metadata.status = ExecutionStatus::Success;
            metadata.output_size_bytes = Some(output_size);
            metadata.annotations = annotations;

            // Check for performance issues
            if self.config.enable_performance_warnings {
                if let Some(duration) = metadata.duration_ms {
                    if duration > self.config.slow_operation_threshold_ms {
                        warn!(
                            "Slow operation detected: {} took {}ms",
                            metadata.tool_name, duration
                        );
                    }
                }

                if output_size > self.config.large_payload_threshold_bytes {
                    warn!(
                        "Large payload detected: {} produced {} bytes",
                        metadata.tool_name, output_size
                    );
                }
            }

            // Store execution history
            let mut executions = self.executions.write().await;
            executions.push(metadata.clone());
            
            // Trim history if needed
            if executions.len() > self.config.max_stored_executions {
                executions.drain(0..executions.len() - self.config.max_stored_executions);
            }

            info!(
                "Completed tool execution: {} ({}) in {}ms",
                metadata.tool_name, trace_id, metadata.duration_ms.unwrap_or(0)
            );
        }
    }

    pub async fn fail_execution(&self, trace_id: String, error: String) {
        let mut active = self.active_traces.write().await;
        if let Some(mut metadata) = active.remove(&trace_id) {
            let end_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            
            metadata.end_time = Some(end_time);
            metadata.duration_ms = Some(end_time - metadata.start_time);
            metadata.status = ExecutionStatus::Failed;
            metadata.error = Some(error.clone());

            let mut executions = self.executions.write().await;
            executions.push(metadata.clone());

            error!(
                "Failed tool execution: {} ({}) - {}",
                metadata.tool_name, trace_id, error
            );
        }
    }

    pub async fn get_execution_history(&self) -> Vec<ToolExecutionMetadata> {
        let executions = self.executions.read().await;
        executions.clone()
    }

    pub async fn get_active_executions(&self) -> Vec<ToolExecutionMetadata> {
        let active = self.active_traces.read().await;
        active.values().cloned().collect()
    }

    pub async fn export_telemetry(&self) -> serde_json::Value {
        let executions = self.executions.read().await;
        let active = self.active_traces.read().await;

        json!({
            "completed_executions": executions.clone(),
            "active_executions": active.values().cloned().collect::<Vec<_>>(),
            "statistics": self.calculate_statistics(&executions),
        })
    }

    fn calculate_statistics(&self, executions: &[ToolExecutionMetadata]) -> serde_json::Value {
        let total = executions.len();
        let successful = executions.iter().filter(|e| matches!(e.status, ExecutionStatus::Success)).count();
        let failed = executions.iter().filter(|e| matches!(e.status, ExecutionStatus::Failed)).count();
        
        let avg_duration = if !executions.is_empty() {
            let sum: u64 = executions.iter()
                .filter_map(|e| e.duration_ms)
                .sum();
            sum / executions.len() as u64
        } else {
            0
        };

        let by_operation: HashMap<String, usize> = executions.iter()
            .fold(HashMap::new(), |mut acc, e| {
                let key = format!("{:?}", e.operation_type);
                *acc.entry(key).or_insert(0) += 1;
                acc
            });

        json!({
            "total_executions": total,
            "successful": successful,
            "failed": failed,
            "average_duration_ms": avg_duration,
            "by_operation_type": by_operation,
        })
    }
}

/// Wrapper for annotating tool results with observability metadata
pub struct AnnotatedToolResult {
    pub content: Vec<Content>,
    pub metadata: ToolExecutionMetadata,
}

impl AnnotatedToolResult {
    pub fn new(content: Vec<Content>, metadata: ToolExecutionMetadata) -> Self {
        Self { content, metadata }
    }

    /// Convert to MCP Content with annotations
    pub fn to_annotated_content(self) -> Vec<Content> {
        let mut annotated_content = self.content;
        
        // Add execution metadata as the last content item
        let metadata_text = format!(
            "\n---\n[Tool Execution Metadata]\nTool: {}\nOperation: {:?}\nDuration: {}ms\nStatus: {:?}\nTrace ID: {}",
            self.metadata.tool_name,
            self.metadata.operation_type,
            self.metadata.duration_ms.unwrap_or(0),
            self.metadata.status,
            self.metadata.trace_id
        );

        annotated_content.push(Content {
            raw: RawContent::Text(TextContent {
                text: metadata_text,
            }),
            annotations: Some(Annotations {
                audience: Some(vec!["assistant".to_string()]),
                priority: Some(0.1),
                cache_control: None,
            }),
        });

        annotated_content
    }
}

/// Macro for wrapping tool handlers with automatic observability
#[macro_export]
macro_rules! annotated_tool {
    ($collector:expr, $tool_name:expr, $op_type:expr, $handler:expr) => {{
        let collector = $collector.clone();
        let tool_name = $tool_name.to_string();
        
        move |params: rmcp::handler::server::wrapper::Parameters| {
            let collector = collector.clone();
            let tool_name = tool_name.clone();
            let handler = $handler.clone();
            
            async move {
                let input_size = serde_json::to_string(&params.arguments)
                    .map(|s| s.len())
                    .unwrap_or(0);
                
                let trace_id = collector
                    .start_execution(tool_name.clone(), $op_type, input_size, None)
                    .await;
                
                match handler(params).await {
                    Ok(result) => {
                        let output_size = serde_json::to_string(&result)
                            .map(|s| s.len())
                            .unwrap_or(0);
                        
                        let mut annotations = std::collections::HashMap::new();
                        annotations.insert("success".to_string(), serde_json::json!(true));
                        
                        collector
                            .complete_execution(trace_id, output_size, annotations)
                            .await;
                        
                        Ok(result)
                    }
                    Err(e) => {
                        collector
                            .fail_execution(trace_id, e.to_string())
                            .await;
                        Err(e)
                    }
                }
            }
        }
    }};
}

/// Helper trait for adding annotations to MCP Content
pub trait ContentAnnotator {
    fn with_annotations(self, annotations: Annotations) -> Self;
    fn with_audience(self, audience: Vec<String>) -> Self;
    fn with_priority(self, priority: f64) -> Self;
}

impl ContentAnnotator for Content {
    fn with_annotations(mut self, annotations: Annotations) -> Self {
        self.annotations = Some(annotations);
        self
    }

    fn with_audience(mut self, audience: Vec<String>) -> Self {
        let mut annotations = self.annotations.unwrap_or_default();
        annotations.audience = Some(audience);
        self.annotations = Some(annotations);
        self
    }

    fn with_priority(mut self, priority: f64) -> Self {
        let mut annotations = self.annotations.unwrap_or_default();
        annotations.priority = Some(priority);
        self.annotations = Some(annotations);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_observability_collector() {
        let collector = ObservabilityCollector::new(ObservabilityConfig::default());
        
        // Start an execution
        let trace_id = collector
            .start_execution(
                "test_tool".to_string(),
                OperationType::Read,
                100,
                None,
            )
            .await;
        
        // Complete it
        let mut annotations = HashMap::new();
        annotations.insert("test_key".to_string(), json!("test_value"));
        
        collector
            .complete_execution(trace_id.clone(), 200, annotations)
            .await;
        
        // Check history
        let history = collector.get_execution_history().await;
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].tool_name, "test_tool");
        assert!(matches!(history[0].status, ExecutionStatus::Success));
    }

    #[tokio::test]
    async fn test_failed_execution() {
        let collector = ObservabilityCollector::new(ObservabilityConfig::default());
        
        let trace_id = collector
            .start_execution(
                "failing_tool".to_string(),
                OperationType::Write,
                50,
                None,
            )
            .await;
        
        collector
            .fail_execution(trace_id, "Test error".to_string())
            .await;
        
        let history = collector.get_execution_history().await;
        assert_eq!(history.len(), 1);
        assert!(matches!(history[0].status, ExecutionStatus::Failed));
        assert_eq!(history[0].error, Some("Test error".to_string()));
    }
}