use tracing::info;
use anyhow::Result;
use crate::mcp_client;
use crate::utils::is_batch_file;
use std::process::{Command, Stdio};
use crate::cli::{McpCommands, VersionCommands, Transport};
use crate::workflow_exec::workflow::run_workflow;
use crate::utils::{
    find_executable,
    init_logging,
};
use rmcp::{
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
use crate::version_control::{
    ensure_project_root,
    full_release,
    sync_all_versions,
    bump_version,
    tag_and_push,
    show_status,
};

/// Create command with proper handling for batch files on Windows
pub fn create_command(executable: &str, args: &[String]) -> tokio::process::Command {
    let mut cmd = if cfg!(windows) && is_batch_file(executable) {
        // For batch files on Windows, use cmd.exe /c
        let mut cmd = tokio::process::Command::new("cmd");
        cmd.arg("/c");
        cmd.arg(executable);
        cmd
    } else {
        tokio::process::Command::new(executable)
    };

    if !args.is_empty() {
        cmd.args(args);
    }

    cmd
}

pub fn run_command(program: &str, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Command failed: {} {}\nError: {}",
            program,
            args.join(" "),
            stderr
        )
        .into());
    }

    Ok(())
}

pub fn parse_command(command: &str) -> Vec<String> {
    // Simple command parsing - splits by spaces but respects quotes
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for c in command.chars() {
        match c {
            '"' => in_quotes = !in_quotes,
            ' ' if !in_quotes => {
                if !current.is_empty() {
                    parts.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(c),
        }
    }

    if !current.is_empty() {
        parts.push(current);
    }

    parts
}

pub fn parse_transport(url: Option<String>, command: Option<String>) -> Transport {
    if let Some(url) = url {
        Transport::Http(url)
    } else if let Some(command) = command {
        let parts = parse_command(&command);
        Transport::Stdio(parts)
    } else {
        // Default to spawning local MCP agent via npx for convenience
        let default_cmd = "npx -y terminator-mcp-agent@latest";
        println!("ℹ️  No --url or --command specified. Falling back to '{default_cmd}'");
        let parts = parse_command(default_cmd);
        Transport::Stdio(parts)
    }
}

pub fn handle_version_command(version_cmd: VersionCommands) {
    ensure_project_root();
    match version_cmd {
        VersionCommands::Patch => bump_version("patch"),
        VersionCommands::Minor => bump_version("minor"),
        VersionCommands::Major => bump_version("major"),
        VersionCommands::Sync => sync_all_versions(),
        VersionCommands::Status => show_status(),
        VersionCommands::Tag => tag_and_push(),
        VersionCommands::Release(args) => { full_release(&args.level.to_string()) }
    }
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
            if let Some(content_vec) = &result.content {
                for content in content_vec {
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
                    }
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
            if let Some(content_vec) = &result.content {
                for content in content_vec {
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
                    }
                }
            }

            // Cancel the service connection
            service.cancel().await?;
        }
    }
    Ok(())
}

pub fn handle_mcp_command(cmd: McpCommands) {
    let transport = match cmd {
        McpCommands::Chat(ref args) => parse_transport(args.url.clone(), args.command.clone()),
        McpCommands::AiChat(ref args) => parse_transport(args.url.clone(), args.command.clone()),
        McpCommands::Exec(ref args) => parse_transport(args.url.clone(), args.command.clone()),
        McpCommands::Run(ref args) => parse_transport(args.url.clone(), args.command.clone()),
    };

    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

    let result = rt.block_on(async {
        match cmd {
            McpCommands::Chat(_) => {
                mcp_client::interactive_chat::interactive_chat(transport).await
            }
            McpCommands::AiChat(args) => {
                mcp_client::natural_lang::aichat(transport, args.aiprovider).await
            }
            McpCommands::Exec(args) => {
                execute_command(transport, args.tool, args.args).await
            }
            McpCommands::Run(args) => {
                run_workflow(transport, args).await
            }
        }
    });

    if let Err(e) = result {
        eprintln!("❌ MCP command error: {e}");
        std::process::exit(1);
    }
}

