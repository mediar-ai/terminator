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

use crate::cli::{Cli, Commands};
use crate::workflow_exec::handle_mcp_command;
use crate::version_control::{
    ensure_project_root,
    full_release,
    sync_all_versions,
    bump_version,
    tag_and_push,
    show_status,
};

mod cli;
mod mcp_client;
mod utils;
mod workflow_exec;
mod version_control;

fn main() {
    use clap::Parser;
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

