//! Cargo subcommand wrapper for the terminator CLI
//!
//! This allows using `cargo terminator <command>` instead of `cargo run --bin terminator -- <command>`

use std::env;
use std::process::Command;

fn main() {
    let args: Vec<String> = env::args().collect();

    // Skip the first argument (cargo-terminator) and pass the rest to the main terminator CLI
    let mut cmd = Command::new("cargo");
    cmd.arg("run").arg("--bin").arg("terminator").arg("--");

    // When called as "cargo terminator", cargo passes:
    // ["cargo-terminator", "terminator", <actual_args>...]
    // So we need to skip the first 2 arguments
    let args_to_pass = if args.len() > 1 && args[1] == "terminator" {
        &args[2..] // Skip "cargo-terminator" and "terminator"
    } else {
        &args[1..] // Skip just "cargo-terminator"
    };

    for arg in args_to_pass {
        cmd.arg(arg);
    }

    let status = cmd
        .status()
        .expect("Failed to execute cargo run --bin terminator");
    std::process::exit(status.code().unwrap_or(1));
}
