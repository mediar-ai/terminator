use anyhow::Result;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, RawContent, TextContent};
use rmcp::transport::stdio;
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use terminator_mcp_agent::observability_tools::{create_annotated_handler, ObservabilityTools};
use terminator_mcp_agent::tool_annotations::{
    ContentAnnotator, ObservabilityCollector, ObservabilityConfig, OperationType,
};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

/// Example tool arguments
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReadFileArgs {
    path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WriteFileArgs {
    path: String,
    content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProcessDataArgs {
    data: Vec<String>,
    operation: String,
}

/// Example service with annotated tools
struct AnnotatedDemoService {
    collector: Arc<ObservabilityCollector>,
    observability_tools: Arc<ObservabilityTools>,
}

#[tool_router]
impl AnnotatedDemoService {
    /// Read a file with automatic observability tracking
    #[tool(description = "Read a file from the filesystem with observability")]
    async fn read_file(&self, params: Parameters<ReadFileArgs>) -> Result<CallToolResult, McpError> {
        info!("Reading file: {}", params.arguments.path);
        
        // Simulate file reading
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        let content = format!("Contents of file: {}", params.arguments.path);
        
        Ok(CallToolResult {
            content: vec![Content {
                raw: RawContent::Text(TextContent { text: content }),
                annotations: None,
            }
            .with_audience(vec!["user".to_string()])],
            is_error: Some(false),
            error: None,
        })
    }

    /// Write a file with automatic observability tracking
    #[tool(description = "Write a file to the filesystem with observability")]
    async fn write_file(&self, params: Parameters<WriteFileArgs>) -> Result<CallToolResult, McpError> {
        info!("Writing to file: {}", params.arguments.path);
        
        // Simulate file writing
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        
        let message = format!(
            "Successfully wrote {} bytes to {}",
            params.arguments.content.len(),
            params.arguments.path
        );
        
        Ok(CallToolResult {
            content: vec![Content {
                raw: RawContent::Text(TextContent { text: message }),
                annotations: None,
            }
            .with_priority(0.8)],
            is_error: Some(false),
            error: None,
        })
    }

    /// Process data with automatic observability tracking
    #[tool(description = "Process data with various operations")]
    async fn process_data(&self, params: Parameters<ProcessDataArgs>) -> Result<CallToolResult, McpError> {
        info!("Processing {} items with operation: {}", 
            params.arguments.data.len(), 
            params.arguments.operation
        );
        
        // Simulate data processing
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        let result = match params.arguments.operation.as_str() {
            "count" => format!("Counted {} items", params.arguments.data.len()),
            "join" => format!("Joined: {}", params.arguments.data.join(", ")),
            "reverse" => {
                let mut reversed = params.arguments.data.clone();
                reversed.reverse();
                format!("Reversed: {:?}", reversed)
            }
            _ => "Unknown operation".to_string(),
        };
        
        Ok(CallToolResult {
            content: vec![Content {
                raw: RawContent::Text(TextContent { text: result }),
                annotations: None,
            }],
            is_error: Some(false),
            error: None,
        })
    }

    /// Simulate a failing operation for testing error tracking
    #[tool(description = "Simulate a failing operation")]
    async fn failing_operation(&self) -> Result<CallToolResult, McpError> {
        info!("Starting failing operation");
        
        // Simulate some work before failing
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        
        Err(McpError {
            code: -32603,
            message: "Simulated failure for testing".to_string(),
            data: None,
        })
    }
}

impl AnnotatedDemoService {
    fn new() -> Self {
        let config = ObservabilityConfig {
            max_stored_executions: 100,
            enable_performance_warnings: true,
            slow_operation_threshold_ms: 300,
            large_payload_threshold_bytes: 1024,
        };
        
        let collector = Arc::new(ObservabilityCollector::new(config));
        let observability_tools = Arc::new(ObservabilityTools::new(collector.clone()));
        
        Self {
            collector,
            observability_tools,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .with_target(false)
        .finish();
    
    tracing::subscriber::set_global_default(subscriber)?;
    
    info!("Starting Annotated Tools Demo MCP Server");
    
    // Create service with annotated tools
    let service = AnnotatedDemoService::new();
    let collector = service.collector.clone();
    
    // Create the server handler with wrapped tools
    let mut handler = ServerHandler::new(
        rmcp::model::ServerInfo {
            name: "annotated-tools-demo".to_string(),
            version: "1.0.0".to_string(),
        },
        rmcp::model::ServerCapabilities {
            tools: Some(rmcp::model::ToolsCapability {
                list_changed: Some(false),
            }),
            resources: None,
            prompts: None,
            sampling: None,
            logging: None,
            completion: None,
            experimental: None,
        },
    );
    
    // Register annotated tools with automatic observability
    handler.tools_mut().register_handler(
        "read_file",
        rmcp::model::Tool {
            name: "read_file".to_string(),
            title: Some("Read File".to_string()),
            description: Some("Read a file with observability tracking".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file"
                    }
                },
                "required": ["path"]
            }),
            output_schema: None,
        },
        create_annotated_handler(
            collector.clone(),
            "read_file".to_string(),
            OperationType::Read,
            |params: Parameters<serde_json::Value>| {
                let args: ReadFileArgs = serde_json::from_value(params.arguments).unwrap();
                async move {
                    info!("Reading file: {}", args.path);
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    
                    let content = format!("Contents of file: {}", args.path);
                    
                    Ok(CallToolResult {
                        content: vec![Content {
                            raw: RawContent::Text(TextContent { text: content }),
                            annotations: None,
                        }],
                        is_error: Some(false),
                        error: None,
                    })
                }
            },
        ),
    );
    
    handler.tools_mut().register_handler(
        "write_file",
        rmcp::model::Tool {
            name: "write_file".to_string(),
            title: Some("Write File".to_string()),
            description: Some("Write a file with observability tracking".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write"
                    }
                },
                "required": ["path", "content"]
            }),
            output_schema: None,
        },
        create_annotated_handler(
            collector.clone(),
            "write_file".to_string(),
            OperationType::Write,
            |params: Parameters<serde_json::Value>| {
                let args: WriteFileArgs = serde_json::from_value(params.arguments).unwrap();
                async move {
                    info!("Writing to file: {}", args.path);
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                    
                    let message = format!(
                        "Successfully wrote {} bytes to {}",
                        args.content.len(),
                        args.path
                    );
                    
                    Ok(CallToolResult {
                        content: vec![Content {
                            raw: RawContent::Text(TextContent { text: message }),
                            annotations: None,
                        }],
                        is_error: Some(false),
                        error: None,
                    })
                }
            },
        ),
    );
    
    // Register observability tools
    handler.tools_mut().register_handler(
        "get_execution_history",
        rmcp::model::Tool {
            name: "get_execution_history".to_string(),
            title: Some("Get Execution History".to_string()),
            description: Some("Get historical data about tool executions".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "tool_name": {
                        "type": "string",
                        "description": "Filter by tool name"
                    },
                    "limit": {
                        "type": "number",
                        "description": "Maximum results"
                    }
                }
            }),
            output_schema: None,
        },
        |params: Parameters<serde_json::Value>| {
            let observability_tools = service.observability_tools.clone();
            async move {
                let args = terminator_mcp_agent::observability_tools::GetExecutionHistoryArgs {
                    tool_name: params.arguments.get("tool_name")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    operation_type: None,
                    limit: params.arguments.get("limit")
                        .and_then(|v| v.as_u64())
                        .map(|n| n as usize),
                };
                
                observability_tools.get_execution_history(Parameters { arguments: args }).await
            }
        },
    );
    
    handler.tools_mut().register_handler(
        "export_telemetry",
        rmcp::model::Tool {
            name: "export_telemetry".to_string(),
            title: Some("Export Telemetry".to_string()),
            description: Some("Export complete telemetry data".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "format": {
                        "type": "string",
                        "description": "Export format (json or csv)"
                    }
                }
            }),
            output_schema: None,
        },
        |params: Parameters<serde_json::Value>| {
            let observability_tools = service.observability_tools.clone();
            async move {
                let args = terminator_mcp_agent::observability_tools::ExportTelemetryArgs {
                    format: params.arguments.get("format")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                };
                
                observability_tools.export_telemetry(Parameters { arguments: args }).await
            }
        },
    );
    
    // Run the server with stdio transport
    info!("Server initialized, starting stdio transport");
    let transport = stdio::Stdio::default();
    let service = handler.service();
    let service = service.with_protocol_version(rmcp::model::ProtocolVersion {
        protocol: "mcp".to_string(),
        supported_versions: vec!["1.0.0".to_string()],
    });
    
    transport.run(service).await?;
    
    info!("Server shutdown");
    Ok(())
}