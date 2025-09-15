use anyhow::Result;
use std::io::{self, Write};
use rmcp::{
    object, ServiceExt,
    model::{CallToolRequestParam, ClientCapabilities, ClientInfo, Implementation},
    transport::{StreamableHttpClientTransport, TokioChildProcess},
};
use crate::{utils, workflow_exec::workflow::Transport};

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
            let executable = utils::find_executable(&command[0]).unwrap_or_else(|| command[0].clone());
            let command_args: Vec<String> = if command.len() > 1 {
                command[1..].to_vec()
            } else {
                vec![]
            };
            let mut cmd = utils::create_command(&executable, &command_args);
            // Ensure server prints useful logs if not set by user
            if std::env::var("LOG_LEVEL").is_err() && std::env::var("RUST_LOG").is_err() {
                cmd.env("LOG_LEVEL", "info");
            }
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
