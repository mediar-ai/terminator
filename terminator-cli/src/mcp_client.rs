use tracing::info;
use anyhow::Result;
use serde_json::json;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use anthropic_sdk::{
    Client as AnthropicClient,
    ToolChoice
};
use rmcp::{
    object,
    ServiceExt,
    model::{
        ClientInfo,
        Implementation,
        CallToolRequestParam,
        ClientCapabilities,
    },
    transport::{
        TokioChildProcess,
        StreamableHttpClientTransport,
    },
};
use crate::utils::{
    find_executable,
    init_logging,
    create_command
};

pub enum Transport {
    Http(String),
    Stdio(Vec<String>),
}

pub async fn interactive_chat(transport: Transport) -> Result<()> {
    println!("🤖 Terminator MCP Chat Client");
    println!("=============================");

    match transport {
        Transport::Http(url) => {
            println!("Connecting to: {url}");
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

            // Get server info
            let server_info = service.peer_info();
            if let Some(info) = server_info {
                println!("✅ Connected to server: {}", info.server_info.name);
                println!("   Version: {}", info.server_info.version);
            }

            // List available tools
            let tools = service.list_all_tools().await?;
            println!("\n📋 Available tools ({}):", tools.len());
            for (i, tool) in tools.iter().enumerate() {
                if i < 10 {
                    println!(
                        "   🔧 {} - {}",
                        tool.name,
                        tool.description.as_deref().unwrap_or("No description")
                    );
                } else if i == 10 {
                    println!("   ... and {} more tools", tools.len() - 10);
                    break;
                }
            }

            println!("\n💡 Examples:");
            println!("  - get_desktop_info");
            println!("  - list_applications");
            println!("  - open_application notepad");
            println!("  - type_text 'Hello from Terminator!'");
            println!("  - take_screenshot");
            println!("\nType 'help' to see all tools, 'exit' to quit");
            println!("=====================================\n");

            let stdin = io::stdin();
            let mut stdout = io::stdout();

            loop {
                print!("🔧 Tool (or command): ");
                stdout.flush()?;

                let mut input = String::new();
                stdin.read_line(&mut input)?;
                let input = input.trim();

                if input.is_empty() {
                    continue;
                }

                if input == "exit" || input == "quit" {
                    println!("👋 Goodbye!");
                    break;
                }

                if input == "help" {
                    println!("\n📚 All available tools:");
                    for tool in &tools {
                        println!(
                            "   {} - {}",
                            tool.name,
                            tool.description.as_deref().unwrap_or("No description")
                        );
                        if let Some(props) = tool.input_schema.get("properties") {
                            println!("      Parameters: {}", serde_json::to_string(props)?);
                        }
                    }
                    println!();
                    continue;
                }

                // Parse tool call
                let parts: Vec<&str> = input.splitn(2, ' ').collect();
                let tool_name = parts[0].to_string();

                // Build arguments
                let arguments = if parts.len() > 1 {
                    let args_part = parts[1];
                    // Try to parse as JSON first
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(args_part) {
                        json.as_object().cloned()
                    } else {
                        // Otherwise, try to build simple arguments
                        match tool_name.as_str() {
                            "open_application" => Some(object!({ "name": args_part.to_string() })),
                            "type_text" => Some(object!({ "text": args_part.to_string() })),
                            _ => None,
                        }
                    }
                } else {
                    None
                };

                println!(
                    "\n⚡ Calling {} with args: {}",
                    tool_name,
                    arguments
                        .as_ref()
                        .map(|a| serde_json::to_string(a).unwrap_or_default())
                        .unwrap_or_else(|| "{}".to_string())
                );

                match service
                    .call_tool(CallToolRequestParam {
                        name: tool_name.into(),
                        arguments,
                    })
                    .await
                {
                    Ok(result) => {
                        println!("✅ Result:");
                        for content in &result.content {
                            if let Some(text) = content.as_text() {
                                println!("{}", text.text);
                            } else if let Some(image) = content.as_image() {
                                println!("[Image: {}]", image.mime_type);
                            } else if let Some(resource) = content.as_resource() {
                                println!("[Resource: {:?}]", resource.resource);
                            }
                        }
                        println!();
                    }
                    Err(e) => {
                        println!("❌ Error: {e}\n");
                    }
                }
            }

            // Cancel the service connection
            service.cancel().await?;
        }
        Transport::Stdio(command) => {
            println!("Starting: {}", command.join(" "));
            let executable = find_executable(&command[0]).unwrap_or_else(|| command[0].clone());
            let command_args: Vec<String> = if command.len() > 1 {
                command[1..].to_vec()
            } else {
                vec![]
            };
            let cmd = create_command(&executable, &command_args);
            let transport = TokioChildProcess::new(cmd)?;
            let service = ().serve(transport).await?;
            // Get server info
            let server_info = service.peer_info();
            if let Some(info) = server_info {
                println!("✅ Connected to server: {}", info.server_info.name);
                println!("   Version: {}", info.server_info.version);
            }

            // List available tools
            let tools = service.list_all_tools().await?;
            println!("\n📋 Available tools ({}):", tools.len());
            for (i, tool) in tools.iter().enumerate() {
                if i < 10 {
                    println!(
                        "   🔧 {} - {}",
                        tool.name,
                        tool.description.as_deref().unwrap_or("No description")
                    );
                } else if i == 10 {
                    println!("   ... and {} more tools", tools.len() - 10);
                    break;
                }
            }

            println!("\n💡 Examples:");
            println!("  - get_desktop_info");
            println!("  - list_applications");
            println!("  - open_application notepad");
            println!("  - type_text 'Hello from Terminator!'");
            println!("  - take_screenshot");
            println!("\nType 'help' to see all tools, 'exit' to quit");
            println!("=====================================\n");

            let stdin = io::stdin();
            let mut stdout = io::stdout();

            loop {
                print!("🔧 Tool (or command): ");
                stdout.flush()?;

                let mut input = String::new();
                stdin.read_line(&mut input)?;
                let input = input.trim();

                if input.is_empty() {
                    continue;
                }

                if input == "exit" || input == "quit" {
                    println!("👋 Goodbye!");
                    break;
                }

                if input == "help" {
                    println!("\n📚 All available tools:");
                    for tool in &tools {
                        println!(
                            "   {} - {}",
                            tool.name,
                            tool.description.as_deref().unwrap_or("No description")
                        );
                        if let Some(props) = tool.input_schema.get("properties") {
                            println!("      Parameters: {}", serde_json::to_string(props)?);
                        }
                    }
                    println!();
                    continue;
                }

                // Parse tool call
                let parts: Vec<&str> = input.splitn(2, ' ').collect();
                let tool_name = parts[0].to_string();

                // Build arguments
                let arguments = if parts.len() > 1 {
                    let args_part = parts[1];
                    // Try to parse as JSON first
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(args_part) {
                        json.as_object().cloned()
                    } else {
                        // Otherwise, try to build simple arguments
                        match tool_name.as_str() {
                            "open_application" => Some(object!({ "name": args_part.to_string() })),
                            "type_text" => Some(object!({ "text": args_part.to_string() })),
                            _ => None,
                        }
                    }
                } else {
                    None
                };

                println!(
                    "\n⚡ Calling {} with args: {}",
                    tool_name,
                    arguments
                        .as_ref()
                        .map(|a| serde_json::to_string(a).unwrap_or_default())
                        .unwrap_or_else(|| "{}".to_string())
                );

                match service
                    .call_tool(CallToolRequestParam {
                        name: tool_name.into(),
                        arguments,
                    })
                    .await
                {
                    Ok(result) => {
                        println!("✅ Result:");
                        for content in &result.content {
                            if let Some(text) = content.as_text() {
                                println!("{}", text.text);
                            } else if let Some(image) = content.as_image() {
                                println!("[Image: {}]", image.mime_type);
                            } else if let Some(resource) = content.as_resource() {
                                println!("[Resource: {:?}]", resource.resource);
                            }
                        }
                        println!();
                    }
                    Err(e) => {
                        println!("❌ Error: {e}\n");
                    }
                }
            }

            // Cancel the service connection
            service.cancel().await?;
        }
    }
    Ok(())
}

pub async fn execute_command(
    transport: Transport,
    tool: String,
    args: Option<String>,
) -> Result<()> {
    // Initialize logging for non-interactive mode
    init_logging();

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
            for content in &result.content {
                if let Some(text) = content.as_text() {
                    println!("{}", text.text);
                } else if let Some(image) = content.as_image() {
                    println!("[Image: {}]", image.mime_type);
                } else if let Some(resource) = content.as_resource() {
                    println!("[Resource: {:?}]", resource.resource);
                }
            }

            // Cancel the service connection
            service.cancel().await?;
        }
        Transport::Stdio(command) => {
            info!("Starting MCP server: {}", command.join(" "));
            let executable = find_executable(&command[0]).unwrap_or_else(|| command[0].clone());
            let command_args: Vec<String> = if command.len() > 1 {
                command[1..].to_vec()
            } else {
                vec![]
            };
            let cmd = create_command(&executable, &command_args);
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
            for content in &result.content {
                if let Some(text) = content.as_text() {
                    println!("{}", text.text);
                } else if let Some(image) = content.as_image() {
                    println!("[Image: {}]", image.mime_type);
                } else if let Some(resource) = content.as_resource() {
                    println!("[Resource: {:?}]", resource.resource);
                }
            }

            // Cancel the service connection
            service.cancel().await?;
        }
    }
    Ok(())
}

pub async fn natural_language_chat(transport: Transport) -> Result<()> {
    println!("🤖 Terminator Natural Language Chat Client");
    println!("==========================================");

    // Load Anthropic API Key
    dotenvy::dotenv().ok();
    let api_key = match std::env::var("ANTHROPIC_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            println!("❌ ANTHROPIC_API_KEY environment variable not set.");
            println!("Please set it in a .env file or export it:");
            println!("  export ANTHROPIC_API_KEY='your-api-key-here'");
            return Ok(());
        }
    };

    // Connect to MCP Server
    let service = match transport {
        Transport::Http(url) => {
            println!("Connecting to MCP server: {url}");
            let transport = StreamableHttpClientTransport::from_uri(url.as_str());
            let client_info = ClientInfo {
                protocol_version: Default::default(),
                capabilities: ClientCapabilities::default(),
                client_info: Implementation {
                    name: "terminator-cli-ai".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
            };
            client_info.serve(transport).await?
        }
        Transport::Stdio(command) => {
            println!("Starting MCP server: {}", command.join(" "));
            let executable = find_executable(&command[0]).unwrap_or_else(|| command[0].clone());
            let command_args: Vec<String> = if command.len() > 1 {
                command[1..].to_vec()
            } else {
                vec![]
            };
            let cmd = create_command(&executable, &command_args);
            let transport = TokioChildProcess::new(cmd)?;
            let client_info = ClientInfo {
                protocol_version: Default::default(),
                capabilities: ClientCapabilities::default(),
                client_info: Implementation {
                    name: "terminator-cli-ai".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
            };
            client_info.serve(transport).await?
        }
    };

    if let Some(info) = service.peer_info() {
        println!("✅ Connected to MCP server: {}", info.server_info.name);
    }

    // Get MCP tools and convert to Anthropic format
    let mcp_tools = service.list_all_tools().await?;
    let anthropic_tools: Vec<serde_json::Value> = mcp_tools
        .into_iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description.unwrap_or_default(),
                "input_schema": t.input_schema
            })
        })
        .collect();

    println!("✅ Found {} tools.", anthropic_tools.len());
    println!("\n💡 Type your command in natural language. Examples:");
    println!("  - 'Open notepad and type hello world'");
    println!("  - 'Take a screenshot of the desktop'");
    println!("  - 'Show me all running applications'");
    println!("\nType 'exit' or 'quit' to end the session.");
    println!("========================================================================================\n");

    let mut messages = Vec::new();

    loop {
        print!("💬 You: ");
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.eq_ignore_ascii_case("exit") || input.eq_ignore_ascii_case("quit") {
            println!("👋 Goodbye!");
            break;
        }

        if input.is_empty() {
            continue;
        }

        // Add user message
        messages.push(json!({
            "role": "user",
            "content": input
        }));

        println!("🤔 Thinking...");

        // Process with Claude and handle tool calls in a loop
        loop {
            // Create request
            let mut request_builder = AnthropicClient::new()
                .auth(api_key.as_str())
                .version("2023-06-01")
                .model("claude-3-opus-20240229")
                .messages(&json!(messages))
                .max_tokens(1000)
                .stream(false); // Disable streaming for simplicity

            // Add tools if available
            if !anthropic_tools.is_empty() {
                request_builder = request_builder.tools(&json!(anthropic_tools));
                request_builder = request_builder.tool_choice(ToolChoice::Auto);
            }

            let request = request_builder.build()?;

            // Execute request and collect the response
            let response_text = Arc::new(Mutex::new(String::new()));
            let response_text_clone = response_text.clone();

            let execute_result = request
                .execute(move |response| {
                    let response_text = response_text_clone.clone();
                    async move {
                        // Collect the full response
                        if let Ok(mut text) = response_text.lock() {
                            text.push_str(&response);
                        }
                    }
                })
                .await;

            if let Err(error) = execute_result {
                eprintln!("❌ Error: {error}");
                break; // Break inner loop on error
            }

            // Get the collected response
            let full_response = response_text.lock().unwrap().clone();

            // Try to parse as JSON (the SDK should return JSON when not in streaming mode)
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&full_response) {
                // Extract content from the response
                let mut assistant_content = Vec::new();
                let mut tool_calls = Vec::new();
                let mut text_parts = Vec::new();

                if let Some(content_array) = json.get("content").and_then(|v| v.as_array()) {
                    for content in content_array {
                        if let Some(content_type) = content.get("type").and_then(|v| v.as_str()) {
                            match content_type {
                                "text" => {
                                    if let Some(text) = content.get("text").and_then(|v| v.as_str())
                                    {
                                        text_parts.push(text.to_string());
                                        assistant_content.push(json!({
                                            "type": "text",
                                            "text": text
                                        }));
                                    }
                                }
                                "tool_use" => {
                                    let tool_call = content.clone();
                                    tool_calls.push(tool_call.clone());
                                    assistant_content.push(tool_call);
                                }
                                _ => {}
                            }
                        }
                    }
                }

                // Print the text response
                if !text_parts.is_empty() {
                    println!("{}", text_parts.join("\n"));
                }

                // Add assistant's response to messages
                if !assistant_content.is_empty() {
                    messages.push(json!({
                        "role": "assistant",
                        "content": assistant_content
                    }));
                }

                // If no tool calls, we're done with this query
                if tool_calls.is_empty() {
                    break;
                }

                // Execute tool calls
                println!("\n🔧 Executing {} tool(s)...", tool_calls.len());
                let mut tool_results = Vec::new();

                // Consume `tool_calls` to avoid holding an iterator borrow across the `await` boundary
                for tool_call in tool_calls {
                    let tool_name = tool_call
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let tool_id = tool_call
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let tool_input = tool_call.get("input").cloned().unwrap_or(json!({}));

                    println!("   - Calling `{tool_name}` with args: {tool_input}");

                    let result = service
                        .call_tool(CallToolRequestParam {
                            name: tool_name.into(),
                            arguments: tool_input.as_object().cloned(),
                        })
                        .await;

                    let result_content = match result {
                        Ok(res) => {
                            let text_results: Vec<String> = res
                                .content
                                .iter()
                                .filter_map(|c| c.as_text().map(|t| t.text.clone()))
                                .collect();
                            if text_results.is_empty() {
                                "Tool executed successfully.".to_string()
                            } else {
                                text_results.join("\n")
                            }
                        }
                        Err(e) => format!("Error: {e}"),
                    };

                    let display_result = if result_content.len() > 100 {
                        format!("{}...", &result_content[..100])
                    } else {
                        result_content.clone()
                    };
                    println!("   ✅ Result: {display_result}");

                    tool_results.push(json!({
                        "type": "tool_result",
                        "tool_use_id": tool_id,
                        "content": result_content
                    }));
                }

                // Add tool results to messages
                messages.push(json!({
                    "role": "user",
                    "content": tool_results
                }));

                println!("\n🤔 Processing results...");
                // Continue the loop to get Claude's response about the tool results
            } else {
                // If not JSON, just print the response
                println!("{full_response}");
                break;
            }
        }
    }

    service.cancel().await?;
    Ok(())
}
