//! Example demonstrating how to use the Terminator MCP Test Runner
//!
//! This example shows how to run automated UI tests using the MCP test runner
//! in different scenarios.

use std::io::{BufRead, BufReader};
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Terminator MCP Test Runner Demo\n");

    // Example 1: Basic text input test
    println!("Example 1: Basic Text Input Test");
    println!("================================");
    run_test(
        "Open Notepad and type Hello World",
        "Text successfully typed in Notepad",
        Some("notepad"),
        false,
    )?;

    // Example 2: Calculator test
    println!("\nExample 2: Calculator Test");
    println!("==========================");
    run_test(
        "Open Calculator and click buttons to calculate 2+2",
        "Result shows 4",
        Some("calc"),
        false,
    )?;

    // Example 3: Generic application test
    println!("\nExample 3: Generic Application Test");
    println!("===================================");
    run_test(
        "Take a screenshot of the desktop",
        "Screenshot captured successfully",
        None,
        false,
    )?;

    Ok(())
}

fn run_test(
    goal: &str,
    expectation: &str,
    app: Option<&str>,
    vm_mode: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Goal: {}", goal);
    println!("Expectation: {}", expectation);

    // Build the command
    let mut cmd = Command::new("cargo");
    cmd.arg("run")
        .arg("--release")
        .arg("--bin")
        .arg("terminator-mcp-test-runner")
        .arg("--")
        .arg("--goal")
        .arg(goal)
        .arg("--expectation")
        .arg(expectation)
        .arg("--output-format")
        .arg("human")
        .arg("--timeout")
        .arg("60");

    if let Some(app_name) = app {
        cmd.arg("--app").arg(app_name);
    }

    if vm_mode {
        cmd.arg("--vm-mode");
    }

    println!("Running test...\n");

    // Execute the command
    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    // Read stdout
    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            println!("{}", line?);
        }
    }

    // Wait for the process to complete
    let status = child.wait()?;

    if status.success() {
        println!("\n✅ Test passed!");
    } else {
        println!("\n❌ Test failed!");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Ignore by default as it requires UI environment
    fn test_notepad_automation() {
        let result = run_test(
            "Open Notepad and type test message",
            "Text typed successfully",
            Some("notepad"),
            false,
        );
        assert!(result.is_ok());
    }

    #[test]
    #[ignore] // Ignore by default as it requires UI environment
    fn test_screenshot_capture() {
        let result = run_test(
            "Capture a screenshot of the current state",
            "Screenshot captured",
            None,
            false,
        );
        assert!(result.is_ok());
    }
}

// Example output format:
/*
Terminator MCP Test Runner Demo

Example 1: Basic Text Input Test
================================
Goal: Open Notepad and type Hello World
Expectation: Text successfully typed in Notepad
Running test...

=== Test Results ===
Goal: Open Notepad and type Hello World
Expectation: Text successfully typed in Notepad
Success: true
Duration: 5234ms
Actual Result: Hello World

Steps Executed:
  1. Open application: notepad - ✓
  2. Get applications - ✓
  3. Type text: Hello from MCP Test Runner - ✓
  4. Capture screenshot: after_typing - ✓

Screenshots:
  - screenshot_after_typing_0.png

✅ Test passed!
*/
