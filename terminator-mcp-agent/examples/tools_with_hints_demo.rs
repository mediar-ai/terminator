use anyhow::Result;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, RawContent, TextContent};
use rmcp::transport::stdio;
use rmcp::{ErrorData as McpError, ServerHandler, ServiceExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use terminator_mcp_agent::mcp_tool_hints::{tool_categories, ToolHints, ToolWithHints};
use terminator_mcp_agent::observability_tools::create_annotated_handler;
use terminator_mcp_agent::tool_annotations::{ObservabilityCollector, ObservabilityConfig, OperationType};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

/// Example arguments for various tool types
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
struct DeleteFileArgs {
    path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QueryDatabaseArgs {
    query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UpdateDatabaseArgs {
    table: String,
    id: u64,
    data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExecuteCommandArgs {
    command: String,
    args: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .with_target(false)
        .finish();
    
    tracing::subscriber::set_global_default(subscriber)?;
    
    info!("Starting MCP Tools with Hints Demo Server");
    
    // Create observability collector for tracking
    let config = ObservabilityConfig {
        max_stored_executions: 100,
        enable_performance_warnings: true,
        slow_operation_threshold_ms: 300,
        large_payload_threshold_bytes: 1024,
    };
    let collector = Arc::new(ObservabilityCollector::new(config));
    
    // Create the server handler
    let mut handler = ServerHandler::new(
        rmcp::model::ServerInfo {
            name: "tools-with-hints-demo".to_string(),
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
    
    // Register READ-ONLY tool: read_file
    handler.tools_mut().register_handler(
        "read_file",
        rmcp::model::Tool {
            name: "read_file".to_string(),
            title: Some("Read File".to_string()),
            description: Some("Read file contents (read-only operation)".to_string()),
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
        }.mark_read_only(), // Apply read-only hints
        create_annotated_handler(
            collector.clone(),
            "read_file".to_string(),
            OperationType::Read,
            |params: Parameters<serde_json::Value>| {
                async move {
                    let args: ReadFileArgs = serde_json::from_value(params.arguments)?;
                    info!("Reading file (read-only): {}", args.path);
                    
                    Ok(CallToolResult {
                        content: vec![Content {
                            raw: RawContent::Text(TextContent {
                                text: format!("File contents of {}: [simulated content]", args.path),
                            }),
                            annotations: None,
                        }],
                        is_error: Some(false),
                        error: None,
                    })
                }
            },
        ),
    );
    
    // Register SAFE WRITE tool: write_file
    handler.tools_mut().register_handler(
        "write_file",
        rmcp::model::Tool {
            name: "write_file".to_string(),
            title: Some("Write File".to_string()),
            description: Some("Write to file (non-destructive write)".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "content": {"type": "string"}
                },
                "required": ["path", "content"]
            }),
            output_schema: None,
        }.with_hints(ToolHints::safe_write().with_open_world(true)),
        create_annotated_handler(
            collector.clone(),
            "write_file".to_string(),
            OperationType::Write,
            |params: Parameters<serde_json::Value>| {
                async move {
                    let args: WriteFileArgs = serde_json::from_value(params.arguments)?;
                    info!("Writing to file (safe write): {}", args.path);
                    
                    Ok(CallToolResult {
                        content: vec![Content {
                            raw: RawContent::Text(TextContent {
                                text: format!("Wrote {} bytes to {}", args.content.len(), args.path),
                            }),
                            annotations: None,
                        }],
                        is_error: Some(false),
                        error: None,
                    })
                }
            },
        ),
    );
    
    // Register DESTRUCTIVE tool: delete_file
    handler.tools_mut().register_handler(
        "delete_file",
        rmcp::model::Tool {
            name: "delete_file".to_string(),
            title: Some("Delete File".to_string()),
            description: Some("Delete a file (DESTRUCTIVE operation)".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"]
            }),
            output_schema: None,
        }.with_hints(tool_categories::file_delete_hints()),
        create_annotated_handler(
            collector.clone(),
            "delete_file".to_string(),
            OperationType::Write,
            |params: Parameters<serde_json::Value>| {
                async move {
                    let args: DeleteFileArgs = serde_json::from_value(params.arguments)?;
                    info!("Deleting file (DESTRUCTIVE): {}", args.path);
                    
                    Ok(CallToolResult {
                        content: vec![Content {
                            raw: RawContent::Text(TextContent {
                                text: format!("⚠️ DELETED file: {}", args.path),
                            }),
                            annotations: None,
                        }],
                        is_error: Some(false),
                        error: None,
                    })
                }
            },
        ),
    );
    
    // Register IDEMPOTENT READ tool: query_database
    handler.tools_mut().register_handler(
        "query_database",
        rmcp::model::Tool {
            name: "query_database".to_string(),
            title: Some("Query Database".to_string()),
            description: Some("Execute a database query (read-only, idempotent)".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                },
                "required": ["query"]
            }),
            output_schema: None,
        }.with_hints(tool_categories::db_query_hints()),
        create_annotated_handler(
            collector.clone(),
            "query_database".to_string(),
            OperationType::Query,
            |params: Parameters<serde_json::Value>| {
                async move {
                    let args: QueryDatabaseArgs = serde_json::from_value(params.arguments)?;
                    info!("Executing DB query (read-only, idempotent): {}", args.query);
                    
                    Ok(CallToolResult {
                        content: vec![Content {
                            raw: RawContent::Text(TextContent {
                                text: format!("Query results: [10 rows returned]"),
                            }),
                            annotations: None,
                        }],
                        is_error: Some(false),
                        error: None,
                    })
                }
            },
        ),
    );
    
    // Register NON-IDEMPOTENT WRITE tool: update_database
    handler.tools_mut().register_handler(
        "update_database",
        rmcp::model::Tool {
            name: "update_database".to_string(),
            title: Some("Update Database".to_string()),
            description: Some("Update database records (non-idempotent write)".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "table": {"type": "string"},
                    "id": {"type": "number"},
                    "data": {"type": "object"}
                },
                "required": ["table", "id", "data"]
            }),
            output_schema: None,
        }.with_hints(tool_categories::db_modify_hints()),
        create_annotated_handler(
            collector.clone(),
            "update_database".to_string(),
            OperationType::Write,
            |params: Parameters<serde_json::Value>| {
                async move {
                    let args: UpdateDatabaseArgs = serde_json::from_value(params.arguments)?;
                    info!("Updating DB record (non-idempotent): {} #{}", args.table, args.id);
                    
                    Ok(CallToolResult {
                        content: vec![Content {
                            raw: RawContent::Text(TextContent {
                                text: format!("Updated {} record #{}", args.table, args.id),
                            }),
                            annotations: None,
                        }],
                        is_error: Some(false),
                        error: None,
                    })
                }
            },
        ),
    );
    
    // Register OPEN-WORLD tool: execute_command
    handler.tools_mut().register_handler(
        "execute_command",
        rmcp::model::Tool {
            name: "execute_command".to_string(),
            title: Some("Execute Command".to_string()),
            description: Some("Execute system command (open-world interaction, requires confirmation)".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string"},
                    "args": {
                        "type": "array",
                        "items": {"type": "string"}
                    }
                },
                "required": ["command"]
            }),
            output_schema: None,
        }.with_hints(tool_categories::process_exec_hints()),
        create_annotated_handler(
            collector.clone(),
            "execute_command".to_string(),
            OperationType::Execute,
            |params: Parameters<serde_json::Value>| {
                async move {
                    let args: ExecuteCommandArgs = serde_json::from_value(params.arguments)?;
                    info!("Executing command (open-world, requires confirmation): {}", args.command);
                    
                    Ok(CallToolResult {
                        content: vec![Content {
                            raw: RawContent::Text(TextContent {
                                text: format!("⚡ Executed: {} {:?}", args.command, args.args),
                            }),
                            annotations: None,
                        }],
                        is_error: Some(false),
                        error: None,
                    })
                }
            },
        ),
    );
    
    // Register tool to show all hints
    handler.tools_mut().register_handler(
        "show_tool_categories",
        rmcp::model::Tool {
            name: "show_tool_categories".to_string(),
            title: Some("Show Tool Categories".to_string()),
            description: Some("Display all tool categories and their hints".to_string()),
            input_schema: serde_json::json!({"type": "object"}),
            output_schema: None,
        }.mark_read_only(),
        |_params: Parameters<serde_json::Value>| {
            async move {
                let categories = vec![
                    ("File Read", tool_categories::file_read_hints()),
                    ("File Write", tool_categories::file_write_hints()),
                    ("File Delete", tool_categories::file_delete_hints()),
                    ("DB Query", tool_categories::db_query_hints()),
                    ("DB Modify", tool_categories::db_modify_hints()),
                    ("HTTP GET", tool_categories::http_get_hints()),
                    ("HTTP Mutate", tool_categories::http_mutate_hints()),
                    ("Process Exec", tool_categories::process_exec_hints()),
                    ("System Config", tool_categories::system_config_hints()),
                ];
                
                let mut output = String::from("Tool Categories and Hints:\n\n");
                for (name, hints) in categories {
                    output.push_str(&format!("{}: {:?}\n", name, hints));
                }
                
                Ok(CallToolResult {
                    content: vec![Content {
                        raw: RawContent::Text(TextContent { text: output }),
                        annotations: None,
                    }],
                    is_error: Some(false),
                    error: None,
                })
            }
        },
    );
    
    // Run the server
    info!("Server initialized with tool hints, starting stdio transport");
    info!("Available tools:");
    info!("  - read_file (read-only)");
    info!("  - write_file (safe write)");
    info!("  - delete_file (DESTRUCTIVE)");
    info!("  - query_database (idempotent read)");
    info!("  - update_database (non-idempotent write)");
    info!("  - execute_command (open-world, requires confirmation)");
    info!("  - show_tool_categories (display all hint categories)");
    
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