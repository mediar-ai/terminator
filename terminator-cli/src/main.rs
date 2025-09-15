#!/usr/bin/env cargo

//! Terminator CLI
//!
//! A cross-platform Rust tool to manage the Terminator project, including version management,
//! releases, and development workflows.
//!
//! Usage from workspace root:
//!   cargo run --bin terminator -- version patch      # Bump patch version
//!   cargo run --bin terminator -- version minor      # Bump minor version  
//!   cargo run --bin terminator -- version major      # Bump major version
//!   cargo run --bin terminator -- version sync       # Sync all versions
//!   cargo run --bin terminator -- version status     # Show current status
//!   cargo run --bin terminator -- version tag        # Tag and push current version
//!   cargo run --bin terminator -- version release    # Full release: bump patch + tag + push
//!   cargo run --bin terminator -- version release minor # Full release: bump minor + tag + push


use crate::cli::{Cli, Commands};
use crate::command::{
    handle_mcp_command, handle_version_command
};

mod cli;
mod utils;
mod command;
mod telemetry;
mod mpc_client;
mod workflow_exec;
mod version_control;

fn main() {
    use clap::Parser;
    let cli = Cli::parse();

    // Only ensure we're in the project root for development commands
    match cli.command {
        Commands::Version(version_cmd) => handle_version_command(version_cmd),
        Commands::Mcp(mcp_cmd) => handle_mcp_command(mcp_cmd),
    }
}
