use anyhow::Result;
use serde_json::json;
use rmcp::{
    ServiceExt,
    transport::{StreamableHttpClientTransport, TokioChildProcess},
    model::{CallToolRequestParam,
        ClientCapabilities, ClientInfo, Implementation},
};
use tracing::info;
use tokio::time::sleep;
use std::time::Duration;
use super::workflow::Transport;
use crate::{utils, telemetry::process};


#[allow(dead_code)]
pub async fn execute_command_with_result(
    transport: Transport,
    tool: String,
    args: Option<String>,
) -> Result<serde_json::Value> {
    execute_command_with_progress(transport, tool, args, false).await
}

pub async fn execute_command_with_progress(
    transport: Transport,
    tool: String,
    args: Option<String>,
    show_progress: bool,
) -> Result<serde_json::Value> {
    execute_command_with_progress_and_retry(transport, tool, args, show_progress, false).await
}

pub async fn execute_command_with_progress_and_retry(
    transport: Transport,
    tool: String,
    args: Option<String>,
    show_progress: bool,
    no_retry: bool,
) -> Result<serde_json::Value> {
    use colored::Colorize;
    use tracing::debug;

    // Start telemetry receiver if showing progress for workflows
    let telemetry_handle = if show_progress && tool == "execute_sequence" {
        match process::start_telemetry_receiver().await {
            Ok(handle) => {
                debug!("Started telemetry receiver on port 4318");
                Some(handle)
            }
            Err(e) => {
                debug!("Failed to start telemetry receiver: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Special handling for execute_sequence to capture full result
    if tool == "execute_sequence" {
        match transport {
            Transport::Http(url) => {
                debug!("Connecting to server: {}", url);
                let transport = StreamableHttpClientTransport::from_uri(url.as_str());
                let client_info = ClientInfo {
                    protocol_version: Default::default(),
                    capabilities: ClientCapabilities::default(),
                    client_info: Implementation {
                        name: "terminator-cli".to_string(),
                        version: env!("CARGO_PKG_VERSION").to_string(),
                    },
                };

                // Connection setup - no retry here as StreamableHttpClientTransport doesn't support cloning
                // Retries will be handled at the tool call level
                let service = client_info.serve(transport).await?;

                let arguments = if let Some(args_str) = args {
                    serde_json::from_str::<serde_json::Value>(&args_str)
                        .ok()
                        .and_then(|v| v.as_object().cloned())
                } else {
                    None
                };

                // Parse workflow to get step count if showing progress
                if show_progress {
                    if let Some(args_obj) = &arguments {
                        if let Some(steps) = args_obj.get("steps").and_then(|v| v.as_array()) {
                            let total_steps = steps.len();
                            println!(
                                "\n{} {} {}",
                                "🎯".cyan(),
                                "WORKFLOW START:".bold().cyan(),
                                format!("{total_steps} steps").dimmed()
                            );

                            // List the steps that will be executed
                            for (i, step) in steps.iter().enumerate() {
                                let tool_name = step
                                    .get("tool_name")
                                    .and_then(|v| v.as_str())
                                    .or_else(|| step.get("group_name").and_then(|v| v.as_str()))
                                    .unwrap_or("unknown");
                                let step_id = step.get("id").and_then(|v| v.as_str()).unwrap_or("");

                                println!(
                                    "  {} Step {}/{}: {} {}",
                                    "📋".dimmed(),
                                    i + 1,
                                    total_steps,
                                    tool_name.yellow(),
                                    if !step_id.is_empty() {
                                        format!("[{step_id}]").dimmed().to_string()
                                    } else {
                                        String::new()
                                    }
                                );
                            }
                            println!("\n{} Executing workflow...\n", "⚡".cyan());
                        }
                    }
                }

                // Retry logic for tool execution
                let mut retry_count = 0;
                let max_retries = if no_retry { 0 } else { 3 };
                let mut _last_error = None;

                let result = loop {
                    match service
                        .call_tool(CallToolRequestParam {
                            name: tool.clone().into(),
                            arguments: arguments.clone(),
                        })
                        .await
                    {
                        Ok(res) => break res,
                        Err(e) => {
                            let error_str = e.to_string();
                            let is_retryable = error_str.contains("401")
                                || error_str.contains("Unauthorized")
                                || error_str.contains("500")
                                || error_str.contains("502")
                                || error_str.contains("503")
                                || error_str.contains("504")
                                || error_str.contains("timeout");

                            if is_retryable && retry_count < max_retries {
                                retry_count += 1;
                                let delay = Duration::from_secs(2u64.pow(retry_count));
                                eprintln!("⚠️  Tool execution failed: {}. Retrying in {} seconds... (attempt {}/{})", 
                                         error_str, delay.as_secs(), retry_count, max_retries);
                                sleep(delay).await;
                                _last_error = Some(e);
                            } else {
                                return Err(e.into());
                            }
                        }
                    }
                };

                // Parse the result content as JSON
                if !result.content.is_empty() {
                    for content in &result.content {
                        if let rmcp::model::RawContent::Text(text) = &content.raw {
                            // Try to parse as JSON
                            if let Ok(json_result) =
                                serde_json::from_str::<serde_json::Value>(&text.text)
                            {
                                service.cancel().await?;

                                // Stop telemetry receiver if it was started
                                if let Some(handle) = telemetry_handle {
                                    handle.abort();
                                }

                                return Ok(json_result);
                            }
                        }
                    }
                }

                service.cancel().await?;

                // Stop telemetry receiver if it was started
                if let Some(handle) = telemetry_handle {
                    handle.abort();
                }

                Ok(json!({"status": "unknown", "message": "No parseable result from workflow"}))
            }
            Transport::Stdio(command) => {
                debug!("Starting MCP server: {}", command.join(" "));
                let executable = utils::find_executable(&command[0]).unwrap_or_else(|| command[0].clone());
                let command_args: Vec<String> = if command.len() > 1 {
                    command[1..].to_vec()
                } else {
                    vec![]
                };
                let mut cmd = utils::create_command(&executable, &command_args);

                // Set up logging for the server to capture step progress
                if std::env::var("LOG_LEVEL").is_err() && std::env::var("RUST_LOG").is_err() {
                    if show_progress {
                        // Enable info level logging to see step progress
                        cmd.env("RUST_LOG", "terminator_mcp_agent=info");
                    } else {
                        cmd.env("LOG_LEVEL", "info");
                    }
                }

                // Enable telemetry if showing progress
                if show_progress {
                    cmd.env("OTEL_EXPORTER_OTLP_ENDPOINT", "http://localhost:4318");
                    cmd.env("OTEL_SERVICE_NAME", "terminator-mcp");
                    cmd.env("ENABLE_TELEMETRY", "true");
                }

                // For now, just use the standard transport without stderr parsing
                // TODO: Add proper step streaming once MCP protocol supports it
                let transport = TokioChildProcess::new(cmd)?;
                let service = ().serve(transport).await?;

                let arguments = if let Some(args_str) = args {
                    // Parse workflow to show initial progress
                    if show_progress {
                        if let Ok(workflow) = serde_json::from_str::<serde_json::Value>(&args_str) {
                            if let Some(steps) = workflow.get("steps").and_then(|v| v.as_array()) {
                                let total_steps = steps.len();
                                println!(
                                    "\n{} {} {}",
                                    "🎯".cyan(),
                                    "WORKFLOW START:".bold().cyan(),
                                    format!("{total_steps} steps").dimmed()
                                );

                                // List the steps that will be executed
                                for (i, step) in steps.iter().enumerate() {
                                    let tool_name = step
                                        .get("tool_name")
                                        .and_then(|v| v.as_str())
                                        .or_else(|| step.get("group_name").and_then(|v| v.as_str()))
                                        .unwrap_or("unknown");
                                    let step_id =
                                        step.get("id").and_then(|v| v.as_str()).unwrap_or("");

                                    println!(
                                        "  {} Step {}/{}: {} {}",
                                        "📋".dimmed(),
                                        i + 1,
                                        total_steps,
                                        tool_name.yellow(),
                                        if !step_id.is_empty() {
                                            format!("[{step_id}]").dimmed().to_string()
                                        } else {
                                            String::new()
                                        }
                                    );
                                }
                                println!("\n{} Executing workflow...\n", "⚡".cyan());
                            }
                        }
                    }

                    serde_json::from_str::<serde_json::Value>(&args_str)
                        .ok()
                        .and_then(|v| v.as_object().cloned())
                } else {
                    None
                };

                // Retry logic for tool execution (stdio)
                let mut retry_count = 0;
                let max_retries = if no_retry { 0 } else { 3 };
                let mut _last_error = None;

                let result = loop {
                    match service
                        .call_tool(CallToolRequestParam {
                            name: tool.clone().into(),
                            arguments: arguments.clone(),
                        })
                        .await
                    {
                        Ok(res) => break res,
                        Err(e) => {
                            let error_str = e.to_string();
                            let is_retryable = error_str.contains("401")
                                || error_str.contains("Unauthorized")
                                || error_str.contains("500")
                                || error_str.contains("502")
                                || error_str.contains("503")
                                || error_str.contains("504")
                                || error_str.contains("timeout");

                            if is_retryable && retry_count < max_retries {
                                retry_count += 1;
                                let delay = Duration::from_secs(2u64.pow(retry_count));
                                eprintln!("⚠️  Tool execution failed: {}. Retrying in {} seconds... (attempt {}/{})", 
                                         error_str, delay.as_secs(), retry_count, max_retries);
                                sleep(delay).await;
                                _last_error = Some(e);
                            } else {
                                return Err(e.into());
                            }
                        }
                    }
                };

                // Parse the result content as JSON
                if !result.content.is_empty() {
                    for content in &result.content {
                        if let rmcp::model::RawContent::Text(text) = &content.raw {
                            // Try to parse as JSON
                            if let Ok(json_result) =
                                serde_json::from_str::<serde_json::Value>(&text.text)
                            {
                                service.cancel().await?;

                                // Stop telemetry receiver if it was started
                                if let Some(handle) = telemetry_handle {
                                    handle.abort();
                                }

                                return Ok(json_result);
                            }
                        }
                    }
                }

                service.cancel().await?;

                // Stop telemetry receiver if it was started
                if let Some(handle) = telemetry_handle {
                    handle.abort();
                }

                Ok(json!({"status": "unknown", "message": "No parseable result from workflow"}))
            }
        }
    } else {
        // For other tools, just execute normally
        execute_command(transport, tool.clone(), args).await?;
        Ok(json!({"status": "success", "message": format!("Tool {} executed", tool)}))
    }
}

pub async fn execute_command(
    transport: Transport,
    tool: String,
    args: Option<String>,
) -> Result<()> {
    // Initialize logging for non-interactive mode
    utils::init_logging();

    match transport {
        Transport::Http(url) => {
            info!("Connecting to server: {}", url);
            let transport = StreamableHttpClientTransport::from_uri(url.as_str());
            let client_info = ClientInfo {
                protocol_version: Default::default(),
                capabilities: ClientCapabilities::default(),
                client_info: Implementation {
                    name: "terminator-cli".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
            };
            let service = client_info.serve(transport).await?;

            let arguments = if let Some(args_str) = args {
                serde_json::from_str::<serde_json::Value>(&args_str)
                    .ok()
                    .and_then(|v| v.as_object().cloned())
            } else {
                None
            };

            println!(
                "⚡ Calling {} with args: {}",
                tool,
                arguments
                    .as_ref()
                    .map(|a| serde_json::to_string(a).unwrap_or_default())
                    .unwrap_or_else(|| "{}".to_string())
            );

            let result = service
                .call_tool(CallToolRequestParam {
                    name: tool.into(),
                    arguments,
                })
                .await?;

            println!("✅ Result:");
            if !result.content.is_empty() {
                for content in &result.content {
                    match &content.raw {
                        rmcp::model::RawContent::Text(text) => {
                            println!("{}", text.text);
                        }
                        rmcp::model::RawContent::Image(image) => {
                            println!("[Image: {}]", image.mime_type);
                        }
                        rmcp::model::RawContent::Resource(resource) => {
                            println!("[Resource: {:?}]", resource.resource);
                        }
                        rmcp::model::RawContent::Audio(audio) => {
                            println!("[Audio: {}]", audio.mime_type);
                        }
                        rmcp::model::RawContent::ResourceLink(resource) => {
                            println!("[ResourceLink: {resource:?}]");
                        }
                    }
                }
            }

            // Cancel the service connection
            service.cancel().await?;
        }
        Transport::Stdio(command) => {
            info!("Starting MCP server: {}", command.join(" "));
            let executable = utils::find_executable(&command[0]).unwrap_or_else(|| command[0].clone());
            let command_args: Vec<String> = if command.len() > 1 {
                command[1..].to_vec()
            } else {
                vec![]
            };
            let mut cmd = utils::create_command(&executable, &command_args);
            // Default server log level to info if not provided by the user
            if std::env::var("LOG_LEVEL").is_err() && std::env::var("RUST_LOG").is_err() {
                cmd.env("LOG_LEVEL", "info");
            }
            let transport = TokioChildProcess::new(cmd)?;
            let service = ().serve(transport).await?;

            let arguments = if let Some(args_str) = args {
                serde_json::from_str::<serde_json::Value>(&args_str)
                    .ok()
                    .and_then(|v| v.as_object().cloned())
            } else {
                None
            };

            println!(
                "⚡ Calling {} with args: {}",
                tool,
                arguments
                    .as_ref()
                    .map(|a| serde_json::to_string(a).unwrap_or_default())
                    .unwrap_or_else(|| "{}".to_string())
            );

            let result = service
                .call_tool(CallToolRequestParam {
                    name: tool.into(),
                    arguments,
                })
                .await?;

            println!("✅ Result:");
            if !result.content.is_empty() {
                for content in &result.content {
                    match &content.raw {
                        rmcp::model::RawContent::Text(text) => {
                            println!("{}", text.text);
                        }
                        rmcp::model::RawContent::Image(image) => {
                            println!("[Image: {}]", image.mime_type);
                        }
                        rmcp::model::RawContent::Resource(resource) => {
                            println!("[Resource: {:?}]", resource.resource);
                        }
                        rmcp::model::RawContent::Audio(audio) => {
                            println!("[Audio: {}]", audio.mime_type);
                        }
                        rmcp::model::RawContent::ResourceLink(resource) => {
                            println!("[ResourceLink: {resource:?}]");
                        }
                    }
                }
            }

            // Cancel the service connection
            service.cancel().await?;
        }
    }
    Ok(())
}

