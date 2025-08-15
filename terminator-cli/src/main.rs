#!/usr/bin/env cargo

//! Terminator CLI
//!
//! A cross-platform Rust tool to manage the Terminator project, including version management,
//! releases, and development workflows.
//!
//! Usage from workspace root:
//!   cargo run --bin terminator -- patch      # Bump patch version
//!   cargo run --bin terminator -- minor      # Bump minor version  
//!   cargo run --bin terminator -- major      # Bump major version
//!   cargo run --bin terminator -- sync       # Sync all versions
//!   cargo run --bin terminator -- status     # Show current status
//!   cargo run --bin terminator -- tag        # Tag and push current version
//!   cargo run --bin terminator -- release    # Full release: bump patch + tag + push
//!   cargo run --bin terminator -- release minor # Full release: bump minor + tag + push

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use std::env;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

mod mcp_client;

#[derive(Parser)]
#[command(name = "terminator")]
#[command(about = "🤖 Terminator CLI - AI-native GUI automation")]
#[command(
    long_about = "Terminator CLI provides tools for managing the Terminator project, including version management, releases, and development workflows."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(ValueEnum, Clone, Copy, Debug, Default)]
#[clap(rename_all = "lower")]
enum BumpLevel {
    #[default]
    Patch,
    Minor,
    Major,
}

impl std::fmt::Display for BumpLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format!("{self:?}").to_lowercase())
    }
}

#[derive(Parser, Debug)]
struct ReleaseArgs {
    /// The part of the version to bump: patch, minor, or major.
    #[clap(value_enum, default_value_t = BumpLevel::Patch)]
    level: BumpLevel,
}

#[derive(Parser, Debug)]
struct McpChatArgs {
    /// MCP server URL (e.g., http://localhost:3000)
    #[clap(long, short = 'u', conflicts_with = "command")]
    url: Option<String>,

    /// Command to start MCP server via stdio (e.g., "npx -y terminator-mcp-agent")
    #[clap(long, short = 'c', conflicts_with = "url")]
    command: Option<String>,
}

#[derive(Parser, Debug)]
struct McpExecArgs {
    /// MCP server URL
    #[clap(long, short = 'u', conflicts_with = "command")]
    url: Option<String>,

    /// Command to start MCP server via stdio
    #[clap(long, short = 'c', conflicts_with = "url")]
    command: Option<String>,

    /// Tool name to execute
    tool: String,

    /// Arguments for the tool (as JSON or simple string)
    args: Option<String>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[clap(rename_all = "lower")]
enum InputType {
    Auto,
    Gist,
    Raw,
    File,
}

#[derive(Parser, Debug)]
struct McpRunArgs {
    /// MCP server URL (e.g., http://localhost:3000)
    #[clap(long, short = 'u', conflicts_with = "command")]
    url: Option<String>,

    /// Command to start MCP server via stdio (e.g., "npx -y terminator-mcp-agent")
    #[clap(long, short = 'c', conflicts_with = "url")]
    command: Option<String>,

    /// Input source - can be a GitHub gist URL, raw gist URL, or local file path (JSON/YAML)
    input: String,

    /// Input type (auto-detected by default)
    #[clap(long, value_enum, default_value = "auto")]
    input_type: InputType,

    /// Dry run - parse and validate the workflow without executing
    #[clap(long)]
    dry_run: bool,

    /// Verbose output
    #[clap(long, short)]
    verbose: bool,

    /// Stop on first error (default: true)
    #[clap(long)]
    no_stop_on_error: bool,

    /// Include detailed results (default: true)
    #[clap(long)]
    no_detailed_results: bool,
}

#[derive(Subcommand)]
enum McpCommands {
    /// Interactive chat with MCP server
    Chat(McpChatArgs),
    /// Interactive AI-powered chat with MCP server
    AiChat(McpChatArgs),
    /// Execute a single MCP tool
    Exec(McpExecArgs),
    /// Execute a workflow sequence from a local file or GitHub gist
    Run(McpRunArgs),
}

#[derive(Subcommand)]
enum Commands {
    /// Bump patch version (x.y.Z+1)
    Patch,
    /// Bump minor version (x.Y+1.0)
    Minor,
    /// Bump major version (X+1.0.0)
    Major,
    /// Sync all package versions without bumping
    Sync,
    /// Show current version status
    Status,
    /// Tag current version and push (triggers CI)
    Tag,
    /// Full release: bump version + tag + push
    Release(ReleaseArgs),
    /// MCP client commands
    #[command(subcommand)]
    Mcp(McpCommands),
}

fn main() {
    let cli = Cli::parse();

    // Only ensure we're in the project root for development commands
    match cli.command {
        Commands::Patch => {
            ensure_project_root();
            bump_version("patch");
        }
        Commands::Minor => {
            ensure_project_root();
            bump_version("minor");
        }
        Commands::Major => {
            ensure_project_root();
            bump_version("major");
        }
        Commands::Sync => {
            ensure_project_root();
            sync_all_versions();
        }
        Commands::Status => {
            ensure_project_root();
            show_status();
        }
        Commands::Tag => {
            ensure_project_root();
            tag_and_push();
        }
        Commands::Release(args) => {
            ensure_project_root();
            full_release(&args.level.to_string());
        }
        Commands::Mcp(mcp_cmd) => handle_mcp_command(mcp_cmd),
    }
}

fn ensure_project_root() {
    // Check if we're already in the project root
    if Path::new("Cargo.toml").exists() && Path::new("terminator").exists() {
        return;
    }

    // If we're in terminator-cli, go up one level
    if env::current_dir()
        .map(|p| {
            p.file_name()
                .map(|n| n == "terminator-cli")
                .unwrap_or(false)
        })
        .unwrap_or(false)
        && env::set_current_dir("..").is_err()
    {
        eprintln!("❌ Failed to change to project root directory");
        std::process::exit(1);
    }

    // Final check
    if !Path::new("Cargo.toml").exists() || !Path::new("terminator").exists() {
        eprintln!("❌ Not in Terminator project root. Please run from workspace root.");
        eprintln!("💡 Usage: terminator <command>");
        std::process::exit(1);
    }
}

fn get_workspace_version() -> Result<String, Box<dyn std::error::Error>> {
    let cargo_toml = fs::read_to_string("Cargo.toml")?;
    let mut in_workspace_package = false;

    for line in cargo_toml.lines() {
        let trimmed_line = line.trim();
        if trimmed_line == "[workspace.package]" {
            in_workspace_package = true;
            continue;
        }

        if in_workspace_package {
            if trimmed_line.starts_with('[') {
                // We've left the workspace.package section
                break;
            }
            if trimmed_line.starts_with("version") {
                if let Some(version_part) = trimmed_line.split('=').nth(1) {
                    if let Some(version) = version_part.trim().split('"').nth(1) {
                        return Ok(version.to_string());
                    }
                }
            }
        }
    }

    Err("Version not found in [workspace.package] in Cargo.toml".into())
}

fn sync_cargo_versions() -> Result<(), Box<dyn std::error::Error>> {
    println!("📦 Syncing Cargo.toml dependency versions...");
    let workspace_version = get_workspace_version()?;

    let cargo_toml = fs::read_to_string("Cargo.toml")?;
    let mut lines: Vec<String> = cargo_toml.lines().map(|s| s.to_string()).collect();
    let mut in_workspace_deps = false;
    let mut deps_version_updated = false;

    let tmp = 0..lines.len();
    for i in tmp {
        let line = &lines[i];
        let trimmed_line = line.trim();

        if trimmed_line.starts_with('[') {
            in_workspace_deps = trimmed_line == "[workspace.dependencies]";
            continue;
        }

        if in_workspace_deps && trimmed_line.starts_with("terminator =") {
            let line_clone = line.clone();
            if let Some(start) = line_clone.find("version = \"") {
                let version_start = start + "version = \"".len();
                if let Some(end_quote_offset) = line_clone[version_start..].find('"') {
                    let range = version_start..(version_start + end_quote_offset);
                    if &line_clone[range.clone()] != workspace_version.as_str() {
                        lines[i].replace_range(range, &workspace_version);
                        println!(
                            "✅ Updated 'terminator' dependency version to {workspace_version}."
                        );
                        deps_version_updated = true;
                    } else {
                        println!("✅ 'terminator' dependency version is already up to date.");
                        deps_version_updated = true; // Mark as done
                    }
                }
            }
            break; // Assume only one terminator dependency to update
        }
    }

    if deps_version_updated {
        fs::write("Cargo.toml", lines.join("\n") + "\n")?;
    } else {
        eprintln!(
            "⚠️  Warning: Could not find 'terminator' in [workspace.dependencies] to sync version."
        );
    }
    Ok(())
}

fn set_workspace_version(new_version: &str) -> Result<(), Box<dyn std::error::Error>> {
    let cargo_toml = fs::read_to_string("Cargo.toml")?;
    let mut lines: Vec<String> = cargo_toml.lines().map(|s| s.to_string()).collect();
    let mut in_workspace_package = false;
    let mut package_version_updated = false;

    let tmp = 0..lines.len();
    for i in tmp {
        let line = &lines[i];
        let trimmed_line = line.trim();

        if trimmed_line.starts_with('[') {
            in_workspace_package = trimmed_line == "[workspace.package]";
            continue;
        }

        if in_workspace_package && trimmed_line.starts_with("version =") {
            let indentation = line.len() - line.trim_start().len();
            lines[i] = format!("{}version = \"{}\"", " ".repeat(indentation), new_version);
            package_version_updated = true;
            break; // Exit after finding and updating the version
        }
    }

    if !package_version_updated {
        return Err("version key not found in [workspace.package] in Cargo.toml".into());
    }

    fs::write("Cargo.toml", lines.join("\n") + "\n")?;
    Ok(())
}

fn parse_version(version: &str) -> Result<(u32, u32, u32), Box<dyn std::error::Error>> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return Err("Invalid version format".into());
    }

    let major = parts[0].parse::<u32>()?;
    let minor = parts[1].parse::<u32>()?;
    let patch = parts[2].parse::<u32>()?;

    Ok((major, minor, patch))
}

fn bump_version(bump_type: &str) {
    println!("🔄 Bumping {bump_type} version...");

    let current_version = match get_workspace_version() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("❌ Failed to get current version: {e}");
            return;
        }
    };

    let (major, minor, patch) = match parse_version(&current_version) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("❌ Failed to parse version {current_version}: {e}");
            return;
        }
    };

    let new_version = match bump_type {
        "patch" => format!("{}.{}.{}", major, minor, patch + 1),
        "minor" => format!("{}.{}.0", major, minor + 1),
        "major" => format!("{}.0.0", major + 1),
        _ => {
            eprintln!("❌ Invalid bump type: {bump_type}");
            return;
        }
    };

    println!("📝 {current_version} → {new_version}");

    if let Err(e) = set_workspace_version(&new_version) {
        eprintln!("❌ Failed to update workspace version: {e}");
        return;
    }

    println!("✅ Updated workspace version to {new_version}");
    sync_all_versions();
}

fn sync_all_versions() {
    println!("🔄 Syncing all package versions...");

    // First, sync versions within Cargo.toml
    if let Err(e) = sync_cargo_versions() {
        eprintln!("❌ Failed to sync versions in Cargo.toml: {e}");
        return;
    }

    let workspace_version = match get_workspace_version() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("❌ Failed to get workspace version: {e}");
            return;
        }
    };

    println!("📦 Workspace version: {workspace_version}");

    // Sync Node.js bindings
    sync_nodejs_bindings(&workspace_version);

    // Sync MCP agent
    sync_mcp_agent(&workspace_version);

    // Sync Browser Extension
    sync_browser_extension(&workspace_version);

    // Update Cargo.lock
    println!("🔒 Updating Cargo.lock...");
    if let Err(e) = run_command("cargo", &["check", "--quiet"]) {
        eprintln!("⚠️  Warning: Failed to update Cargo.lock: {e}");
    }

    println!("✅ All versions synchronized!");
}

fn sync_nodejs_bindings(version: &str) {
    println!("📦 Syncing Node.js bindings to version {version}...");

    let nodejs_dir = Path::new("bindings/nodejs");
    if !nodejs_dir.exists() {
        println!("⚠️  Node.js bindings directory not found, skipping");
        return;
    }

    // Update main package.json directly
    if let Err(e) = update_package_json("bindings/nodejs/package.json", version) {
        eprintln!("⚠️  Warning: Failed to update Node.js package.json directly: {e}");
    } else {
        println!("✅ Updated Node.js package.json to {version}");
    }

    // ALSO update CPU/platform-specific packages under bindings/nodejs/npm
    let npm_dir = nodejs_dir.join("npm");
    if npm_dir.exists() {
        if let Ok(entries) = fs::read_dir(&npm_dir) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    let package_json = entry.path().join("package.json");
                    if package_json.exists() {
                        if let Err(e) =
                            update_package_json(&package_json.to_string_lossy(), version)
                        {
                            eprintln!(
                                "⚠️  Warning: Failed to update {}: {}",
                                package_json.display(),
                                e
                            );
                        } else {
                            println!("📦 Updated {}", entry.file_name().to_string_lossy());
                        }
                    }
                }
            }
        }
    }

    // Run sync script if it exists (still useful for additional tasks like N-API metadata)
    let original_dir = match env::current_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("❌ Could not get current directory: {e}");
            return;
        }
    };

    if env::set_current_dir(nodejs_dir).is_ok() {
        println!("🔄 Running npm run sync-version...");
        if run_command("npm", &["run", "sync-version"]).is_ok() {
            println!("✅ Node.js sync script completed");
        } else {
            eprintln!("⚠️  Warning: npm run sync-version failed");
        }
        // Always change back to the original directory
        if let Err(e) = env::set_current_dir(&original_dir) {
            eprintln!("❌ Failed to restore original directory: {e}");
            std::process::exit(1); // Exit if we can't get back, to avoid further errors
        }
    } else {
        eprintln!("⚠️  Warning: Could not switch to Node.js directory");
    }
}

fn sync_mcp_agent(version: &str) {
    println!("📦 Syncing MCP agent...");

    let mcp_dir = Path::new("terminator-mcp-agent");
    if !mcp_dir.exists() {
        return;
    }

    // Update main package.json
    if let Err(e) = update_package_json("terminator-mcp-agent/package.json", version) {
        eprintln!("⚠️  Warning: Failed to update MCP agent package.json: {e}");
        return;
    }

    // Update platform packages
    let npm_dir = mcp_dir.join("npm");
    if npm_dir.exists() {
        if let Ok(entries) = fs::read_dir(npm_dir) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    let package_json = entry.path().join("package.json");
                    if package_json.exists() {
                        if let Err(e) =
                            update_package_json(&package_json.to_string_lossy(), version)
                        {
                            eprintln!(
                                "⚠️  Warning: Failed to update {}: {}",
                                entry.path().display(),
                                e
                            );
                        } else {
                            println!("📦 Updated {}", entry.file_name().to_string_lossy());
                        }
                    }
                }
            }
        }
    }

    // Update package-lock.json
    let original_dir = match env::current_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("❌ Could not get current directory: {e}");
            return;
        }
    };

    if env::set_current_dir(mcp_dir).is_ok() {
        if run_command("npm", &["install", "--package-lock-only", "--silent"]).is_ok() {
            println!("✅ MCP package-lock.json updated.");
        } else {
            eprintln!("⚠️  Warning: Failed to update MCP agent package-lock.json");
        }
        // Always change back to the original directory
        if let Err(e) = env::set_current_dir(&original_dir) {
            eprintln!("❌ Failed to restore original directory: {e}");
            std::process::exit(1);
        }
    }

    println!("✅ MCP agent synced");
}

fn sync_browser_extension(version: &str) {
    println!("📦 Syncing browser extension to version {version}...");

    let ext_dir = Path::new("terminator/browser-extension");
    if !ext_dir.exists() {
        println!("⚠️  Browser extension directory not found, skipping");
        return;
    }

    let manifest_path = ext_dir.join("manifest.json");
    if manifest_path.exists() {
        if let Err(e) = update_json_version(&manifest_path.to_string_lossy(), version) {
            eprintln!(
                "⚠️  Warning: Failed to update {}: {}",
                manifest_path.display(),
                e
            );
        } else {
            println!("✅ Updated manifest.json to {version}");
        }
    }

    let build_check_path = ext_dir.join("build_check.json");
    if build_check_path.exists() {
        if let Err(e) = update_json_version(&build_check_path.to_string_lossy(), version) {
            eprintln!(
                "⚠️  Warning: Failed to update {}: {}",
                build_check_path.display(),
                e
            );
        } else {
            println!("✅ Updated build_check.json to {version}");
        }
    }
}

fn update_package_json(path: &str, version: &str) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let mut pkg: serde_json::Value = serde_json::from_str(&content)?;

    // Update main version
    pkg["version"] = serde_json::Value::String(version.to_string());

    // Update optional dependencies that start with terminator-mcp- or terminator.js-
    if let Some(deps) = pkg
        .get_mut("optionalDependencies")
        .and_then(|v| v.as_object_mut())
    {
        for (key, value) in deps.iter_mut() {
            if key.starts_with("terminator-mcp-") || key.starts_with("terminator.js-") {
                *value = serde_json::Value::String(version.to_string());
            }
        }
    }

    // Write back with pretty formatting
    let formatted = serde_json::to_string_pretty(&pkg)?;
    fs::write(path, formatted + "\n")?;

    Ok(())
}

fn update_json_version(path: &str, version: &str) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let mut json_value: serde_json::Value = serde_json::from_str(&content)?;

    json_value["version"] = serde_json::Value::String(version.to_string());

    let formatted = serde_json::to_string_pretty(&json_value)?;
    fs::write(path, formatted + "\n")?;

    Ok(())
}

fn show_status() {
    println!("📊 Terminator Project Status");
    println!("============================");

    let workspace_version = get_workspace_version().unwrap_or_else(|_| "ERROR".to_string());
    println!("📦 Workspace version: {workspace_version}");

    // Show package versions
    let nodejs_version = get_package_version("bindings/nodejs/package.json");
    let mcp_version = get_package_version("terminator-mcp-agent/package.json");
    let browser_extension_version =
        get_package_version("terminator/browser-extension/manifest.json");

    println!();
    println!("Package versions:");
    println!("  Node.js bindings: {nodejs_version}");
    println!("  MCP agent:        {mcp_version}");
    println!("  Browser extension:{browser_extension_version}");

    // Git status
    println!();
    println!("Git status:");
    if let Ok(output) = Command::new("git").args(["status", "--porcelain"]).output() {
        let status = String::from_utf8_lossy(&output.stdout);
        if status.trim().is_empty() {
            println!("  ✅ Working directory clean");
        } else {
            println!("  ⚠️  Uncommitted changes:");
            for line in status.lines().take(5) {
                println!("     {line}");
            }
        }
    }
}

fn get_package_version(path: &str) -> String {
    match fs::read_to_string(path) {
        Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(pkg) => pkg
                .get("version")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "No version field".to_string()),
            Err(_) => "Parse error".to_string(),
        },
        Err(_) => "Not found".to_string(),
    }
}

fn tag_and_push() {
    let version = match get_workspace_version() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("❌ Failed to get current version: {e}");
            return;
        }
    };

    println!("🏷️  Tagging and pushing version {version}...");

    // Check for uncommitted changes
    if let Ok(output) = Command::new("git").args(["diff", "--name-only"]).output() {
        let diff = String::from_utf8_lossy(&output.stdout);
        if !diff.trim().is_empty() {
            println!("⚠️  Uncommitted changes detected. Committing...");
            if let Err(e) = run_command("git", &["add", "."]) {
                eprintln!("❌ Failed to git add: {e}");
                return;
            }
            if let Err(e) = run_command(
                "git",
                &["commit", "-m", &format!("Bump version to {version}")],
            ) {
                eprintln!("❌ Failed to git commit: {e}");
                return;
            }
        }
    }

    // Create tag
    let tag = format!("v{version}");
    if let Err(e) = run_command(
        "git",
        &[
            "tag",
            "-a",
            &tag,
            "-m",
            &format!("Release version {version}"),
        ],
    ) {
        eprintln!("❌ Failed to create tag: {e}");
        return;
    }

    // Push changes and tag
    if let Err(e) = run_command("git", &["push", "origin", "main"]) {
        eprintln!("❌ Failed to push changes: {e}");
        return;
    }

    if let Err(e) = run_command("git", &["push", "origin", &tag]) {
        eprintln!("❌ Failed to push tag: {e}");
        return;
    }

    println!("✅ Successfully released version {version}!");
    println!("🔗 Check CI: https://github.com/mediar-ai/terminator/actions");
}

fn full_release(bump_type: &str) {
    println!("🚀 Starting full release process with {bump_type} bump...");
    bump_version(bump_type);
    tag_and_push();
}

fn run_command(program: &str, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
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

fn handle_mcp_command(cmd: McpCommands) {
    let transport = match cmd {
        McpCommands::Chat(ref args) => parse_transport(args.url.clone(), args.command.clone()),
        McpCommands::AiChat(ref args) => parse_transport(args.url.clone(), args.command.clone()),
        McpCommands::Exec(ref args) => parse_transport(args.url.clone(), args.command.clone()),
        McpCommands::Run(ref args) => parse_transport(args.url.clone(), args.command.clone()),
    };

    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

    let result = rt.block_on(async {
        match cmd {
            McpCommands::Chat(_) => mcp_client::interactive_chat(transport).await,
            McpCommands::AiChat(_) => mcp_client::natural_language_chat(transport).await,
            McpCommands::Exec(args) => {
                mcp_client::execute_command(transport, args.tool, args.args).await
            }
            McpCommands::Run(args) => run_workflow(transport, args).await,
        }
    });

    if let Err(e) = result {
        eprintln!("❌ MCP command error: {e}");
        std::process::exit(1);
    }
}

fn parse_transport(url: Option<String>, command: Option<String>) -> mcp_client::Transport {
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

fn parse_command(command: &str) -> Vec<String> {
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

async fn run_workflow(transport: mcp_client::Transport, args: McpRunArgs) -> anyhow::Result<()> {
    use tracing::info;

    if args.verbose {
        // Keep rmcp quieter even in verbose mode unless user explicitly overrides
        std::env::set_var("RUST_LOG", "debug,rmcp=warn");
    }

    // Initialize simple logging (only if not already initialized)
    {
        use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
        let _ = tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    // Suppress noisy rmcp info logs by default while keeping our own at info
                    .unwrap_or_else(|_| "info,rmcp=warn".into()),
            )
            .with(tracing_subscriber::fmt::layer())
            .try_init(); // Use try_init instead of init to avoid panics on duplicate initialization
    }

    info!("Starting workflow execution via terminator CLI");
    info!(input = %args.input, ?args.input_type);

    // Resolve actual input type (auto-detect if needed)
    let resolved_type = determine_input_type(&args.input, args.input_type);

    // Fetch workflow content
    let content = match resolved_type {
        InputType::File => {
            info!("Reading local file");
            read_local_file(&args.input).await?
        }
        InputType::Gist => {
            info!("Fetching GitHub gist");
            let raw_url = convert_gist_to_raw_url(&args.input)?;
            fetch_remote_content(&raw_url).await?
        }
        InputType::Raw => {
            info!("Fetching raw URL");
            fetch_remote_content(&args.input).await?
        }
        InputType::Auto => unreachable!(),
    };

    // Parse workflow using the same robust logic as gist_executor
    let mut workflow_val = parse_workflow_content(&content)
        .with_context(|| format!("Failed to parse workflow from {}", args.input))?;

    // Validate workflow structure early to catch issues
    validate_workflow(&workflow_val).with_context(|| "Workflow validation failed")?;

    // Get steps count for logging
    let steps_count = workflow_val
        .get("steps")
        .and_then(|v| v.as_array())
        .map(|arr| arr.len())
        .unwrap_or(0);

    info!(
        "Successfully parsed and validated workflow with {} steps",
        steps_count
    );

    // Apply overrides
    if let Some(obj) = workflow_val.as_object_mut() {
        if args.no_stop_on_error {
            obj.insert("stop_on_error".into(), serde_json::Value::Bool(false));
        }
        if args.no_detailed_results {
            obj.insert(
                "include_detailed_results".into(),
                serde_json::Value::Bool(false),
            );
        }
    }

    if args.dry_run {
        println!("✅ Workflow validation successful!");
        println!("📊 Workflow Summary:");
        println!("   • Steps: {steps_count}");

        if let Some(variables) = workflow_val.get("variables").and_then(|v| v.as_object()) {
            println!("   • Variables: {}", variables.len());
        } else {
            println!("   • Variables: 0");
        }

        if let Some(selectors) = workflow_val.get("selectors").and_then(|v| v.as_object()) {
            println!("   • Selectors: {}", selectors.len());
        } else {
            println!("   • Selectors: 0");
        }

        let stop_on_error = workflow_val
            .get("stop_on_error")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        println!("   • Stop on error: {stop_on_error}");

        return Ok(());
    }

    info!("Executing workflow with {steps_count} steps via MCP");

    // Debug: Print the workflow structure that we're about to send
    if args.verbose {
        let workflow_debug = serde_json::to_string_pretty(&workflow_val)?;
        info!("Workflow structure being sent: {}", workflow_debug);
    }

    // Send the clean workflow JSON directly instead of converting through ExecuteSequenceArgs
    // to avoid adding null fields that might confuse the server
    let workflow_str = serde_json::to_string(&workflow_val)?;

    mcp_client::execute_command(
        transport,
        "execute_sequence".to_string(),
        Some(workflow_str),
    )
    .await?;

    Ok(())
}

fn determine_input_type(input: &str, specified_type: InputType) -> InputType {
    match specified_type {
        InputType::Auto => {
            if input.starts_with("https://gist.github.com/") {
                InputType::Gist
            } else if input.starts_with("https://gist.githubusercontent.com/")
                || input.starts_with("http://")
                || input.starts_with("https://")
            {
                InputType::Raw
            } else {
                InputType::File
            }
        }
        other => other,
    }
}

fn convert_gist_to_raw_url(gist_url: &str) -> anyhow::Result<String> {
    if !gist_url.starts_with("https://gist.github.com/") {
        return Err(anyhow::anyhow!("Invalid GitHub gist URL format"));
    }

    let raw_url = gist_url.replace(
        "https://gist.github.com/",
        "https://gist.githubusercontent.com/",
    );

    Ok(if raw_url.ends_with("/raw") {
        raw_url
    } else {
        format!("{raw_url}/raw")
    })
}

async fn read_local_file(path: &str) -> anyhow::Result<String> {
    use std::path::Path;
    use tokio::fs;

    let p = Path::new(path);
    if !p.exists() {
        return Err(anyhow::anyhow!("File not found: {}", p.display()));
    }
    if !p.is_file() {
        return Err(anyhow::anyhow!("Not a file: {}", p.display()));
    }

    fs::read_to_string(p).await.map_err(|e| e.into())
}

async fn fetch_remote_content(url: &str) -> anyhow::Result<String> {
    let client = reqwest::Client::new();
    let res = client
        .get(url)
        .header("User-Agent", "terminator-cli-workflow/1.0")
        .send()
        .await?;
    if !res.status().is_success() {
        return Err(anyhow::anyhow!(
            "HTTP request failed: {} for {}",
            res.status(),
            url
        ));
    }
    Ok(res.text().await?)
}

/// Parse workflow content using robust parsing strategies from gist_executor.rs
fn parse_workflow_content(content: &str) -> anyhow::Result<serde_json::Value> {
    // Strategy 1: Try direct JSON workflow
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(content) {
        // Check if it's a valid workflow (has steps field)
        if val.get("steps").is_some() {
            return Ok(val);
        }

        // Check if it's a wrapper object
        if let Some(extracted) = extract_workflow_from_wrapper(&val)? {
            return Ok(extracted);
        }
    }

    // Strategy 2: Try direct YAML workflow
    if let Ok(val) = serde_yaml::from_str::<serde_json::Value>(content) {
        // Check if it's a valid workflow (has steps field)
        if val.get("steps").is_some() {
            return Ok(val);
        }

        // Check if it's a wrapper object
        if let Some(extracted) = extract_workflow_from_wrapper(&val)? {
            return Ok(extracted);
        }
    }

    // Strategy 3: Try parsing as JSON wrapper first, then extract
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(content) {
        if let Some(extracted) = extract_workflow_from_wrapper(&val)? {
            return Ok(extracted);
        }
    }

    // Strategy 4: Try parsing as YAML wrapper first, then extract
    if let Ok(val) = serde_yaml::from_str::<serde_json::Value>(content) {
        if let Some(extracted) = extract_workflow_from_wrapper(&val)? {
            return Ok(extracted);
        }
    }

    Err(anyhow::anyhow!(
        "Unable to parse content as JSON or YAML workflow or wrapper object. Content must either be:\n\
        1. A workflow with 'steps' field\n\
        2. A wrapper object with tool_name='execute_sequence' and 'arguments' field\n\
        3. Valid JSON or YAML format"
    ))
}

/// Extract workflow from wrapper object if it has tool_name: execute_sequence
fn extract_workflow_from_wrapper(
    value: &serde_json::Value,
) -> anyhow::Result<Option<serde_json::Value>> {
    if let Some(tool_name) = value.get("tool_name") {
        if tool_name == "execute_sequence" {
            if let Some(arguments) = value.get("arguments") {
                return Ok(Some(arguments.clone()));
            } else {
                return Err(anyhow::anyhow!("Tool call missing 'arguments' field"));
            }
        }
    }
    Ok(None)
}

/// Validate workflow structure to provide early error detection
fn validate_workflow(workflow: &serde_json::Value) -> anyhow::Result<()> {
    // Check that it's an object
    let obj = workflow
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Workflow must be a JSON object"))?;

    // Check that steps exists and is an array
    let steps = obj
        .get("steps")
        .ok_or_else(|| anyhow::anyhow!("Workflow must contain a 'steps' field"))?;

    let steps_array = steps
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("'steps' field must be an array"))?;

    if steps_array.is_empty() {
        return Err(anyhow::anyhow!("Workflow must contain at least one step"));
    }

    // Validate each step
    for (i, step) in steps_array.iter().enumerate() {
        let step_obj = step
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("Step {} must be an object", i))?;

        let has_tool_name = step_obj.contains_key("tool_name");
        let has_group_name = step_obj.contains_key("group_name");

        if !has_tool_name && !has_group_name {
            return Err(anyhow::anyhow!(
                "Step {} must have either 'tool_name' or 'group_name'",
                i
            ));
        }

        if has_tool_name && has_group_name {
            return Err(anyhow::anyhow!(
                "Step {} cannot have both 'tool_name' and 'group_name'",
                i
            ));
        }
    }

    // Validate variables if present
    if let Some(variables) = obj.get("variables") {
        if let Some(vars_obj) = variables.as_object() {
            for (name, def) in vars_obj {
                if name.is_empty() {
                    return Err(anyhow::anyhow!("Variable name cannot be empty"));
                }

                if let Some(def_obj) = def.as_object() {
                    // Ensure label exists and is non-empty
                    if let Some(label) = def_obj.get("label") {
                        if let Some(label_str) = label.as_str() {
                            if label_str.is_empty() {
                                return Err(anyhow::anyhow!(
                                    "Variable '{}' must have a non-empty label",
                                    name
                                ));
                            }
                        }
                    } else {
                        return Err(anyhow::anyhow!(
                            "Variable '{}' must have a 'label' field",
                            name
                        ));
                    }

                    // --------------------- NEW VALIDATION ---------------------
                    // Enforce `required` property logic
                    let is_required = def_obj
                        .get("required")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);

                    if is_required {
                        // Check for default value in definition
                        let has_default = def_obj.contains_key("default");

                        // Check if inputs provide a value for this variable
                        let input_has_value = obj
                            .get("inputs")
                            .and_then(|v| v.as_object())
                            .map(|inputs_obj| inputs_obj.contains_key(name))
                            .unwrap_or(false);

                        if !has_default && !input_has_value {
                            return Err(anyhow::anyhow!(
                                "Required variable '{}' is missing and has no default value",
                                name
                            ));
                        }
                    }
                    // ----------------------------------------------------------------
                }
            }
        }
    }

    Ok(())
}
