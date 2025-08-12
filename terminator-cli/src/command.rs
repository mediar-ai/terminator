use anyhow::Result;
use crate::mcp_client;
use crate::utils::is_batch_file;
use std::process::{Command, Stdio};
use crate::cli::{McpCommands, VersionCommands};
use crate::workflow_exec::workflow::run_workflow;
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

pub fn parse_transport(url: Option<String>, command: Option<String>) -> mcp_client::Transport {
    if let Some(url) = url {
        mcp_client::Transport::Http(url)
    } else if let Some(command) = command {
        let parts = parse_command(&command);
        mcp_client::Transport::Stdio(parts)
    } else {
        // Default to spawning local MCP agent via npx for convenience
        let default_cmd = "npx -y terminator-mcp-agent@latest";
        println!("ℹ️  No --url or --command specified. Falling back to '{default_cmd}'");
        let parts = parse_command(default_cmd);
        mcp_client::Transport::Stdio(parts)
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
                mcp_client::interactive_chat(transport).await
            }
            McpCommands::AiChat(args) => {
                mcp_client::natural_language_chat(transport, args.aiprovider).await
            }
            McpCommands::Exec(args) => {
                mcp_client::execute_command(transport, args.tool, args.args).await
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
