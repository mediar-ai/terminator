# Terminator MCP Test Runner

A binary for automated UI testing using the Terminator MCP Agent, designed for CI/CD environments like GitHub Actions.

## Overview

The `terminator-mcp-test-runner` is a command-line tool that:
- Connects to the Terminator MCP Agent as a client
- Executes UI automation tests based on natural language goals
- Validates expected outcomes
- Reports results in machine-readable (JSON) or human-readable formats
- Works in virtual machine environments with appropriate delays

## Features

- **Goal-Based Testing**: Define tests using natural language goals like "Open Notepad and type Hello World"
- **Expectation Validation**: Verify outcomes match expectations
- **Cross-Platform**: Works on Windows, Linux, and macOS (with platform-specific limitations)
- **CI/CD Ready**: JSON output format for easy parsing in automation pipelines
- **VM Mode**: Special mode with increased delays for virtual machine environments
- **Screenshot Capture**: Automatically captures screenshots during test execution
- **Timeout Protection**: Configurable timeout to prevent hanging tests

## Installation

Build from source:
```bash
cargo build --release --bin terminator-mcp-test-runner
```

## Usage

### Basic Command
```bash
terminator-mcp-test-runner \
  --goal "Open Notepad and type Hello World" \
  --expectation "Text successfully typed" \
  --app notepad
```

### All Options
```bash
terminator-mcp-test-runner [OPTIONS]

OPTIONS:
  -g, --goal <GOAL>                    Goal of the test - what the automation should achieve
  -e, --expectation <EXPECTATION>      Expected outcome - what should be validated after execution
  -t, --timeout <TIMEOUT>              Test timeout in seconds [default: 300]
  -a, --app <APP>                      Application to test (e.g., "notepad", "calculator")
      --standalone <STANDALONE>        Use standalone MCP server [default: true]
      --server-command <SERVER_COMMAND> Server command (if standalone) [default: terminator-mcp-agent]
      --output-format <OUTPUT_FORMAT>  Output format (json, human) [default: json]
      --vm-mode                        Virtual machine mode - adds delays for VM environments
  -h, --help                          Print help
  -V, --version                       Print version
```

## GitHub Actions Integration

### Simple Workflow Example

```yaml
name: UI Test

on: [push, pull_request]

jobs:
  test:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Build Test Runner
        run: |
          cargo build --release --bin terminator-mcp-agent
          cargo build --release --bin terminator-mcp-test-runner
      
      - name: Run UI Test
        run: |
          ./target/release/terminator-mcp-test-runner \
            --goal "Open Notepad and type test message" \
            --expectation "Text typed successfully" \
            --app notepad \
            --output-format json > results.json
      
      - name: Upload Results
        uses: actions/upload-artifact@v4
        with:
          name: test-results
          path: results.json
```

### Advanced Matrix Testing

See `.github/workflows/mcp-test-runner-example.yml` for a complete example with:
- Multi-OS testing (Windows, Linux)
- Virtual display setup for Linux
- Matrix strategy for multiple test cases
- Result parsing and reporting

## Output Formats

### JSON Format
```json
{
  "success": true,
  "goal": "Open Notepad and type Hello World",
  "expectation": "Text successfully typed",
  "actual_result": "Hello World",
  "error": null,
  "duration_ms": 5234,
  "steps_executed": [
    {
      "step_number": 1,
      "action": "Open application: notepad",
      "success": true,
      "details": {...},
      "error": null
    }
  ],
  "screenshots": ["screenshot_after_typing_0.png"]
}
```

### Human Format
```
=== Test Results ===
Goal: Open Notepad and type Hello World
Expectation: Text successfully typed
Success: true
Duration: 5234ms
Actual Result: Hello World

Steps Executed:
  1. Open application: notepad - ✓
  2. Type text: Hello World - ✓
  3. Capture screenshot: after_typing - ✓

Screenshots:
  - screenshot_after_typing_0.png
```

## Test Goal Examples

### Text Input Tests
- "Open Notepad and type Hello World"
- "Type a test message in the text editor"
- "Enter user credentials in the login form"

### Click Tests
- "Click the calculate button"
- "Press the submit button"
- "Click on File menu and select Save"

### Navigation Tests
- "Navigate to settings"
- "Open the preferences dialog"
- "Go to the help section"

## Virtual Machine Considerations

When running in VM environments (GitHub Actions, Docker, etc.), use `--vm-mode`:
- Increases wait times for application startup
- Adds delays between actions for slower environments
- Ensures UI elements have time to render

## Exit Codes

- `0`: Test passed successfully
- `1`: Test failed or error occurred
- Other: System-specific error codes

## Troubleshooting

### Test Timeouts
Increase the timeout value:
```bash
--timeout 600  # 10 minutes
```

### VM Performance Issues
Always use `--vm-mode` in virtualized environments:
```bash
--vm-mode --timeout 300
```

### Application Not Found
Ensure the application is installed and accessible:
- Windows: Application should be in PATH or provide full path
- Linux: Application should be installed via package manager
- Use `which <app>` or `where <app>` to verify

## Development

### Adding New Goal Types

Edit `src/test_runner.rs` and modify the `execute_goal_based_test` method:

```rust
if goal_lower.contains("your_pattern") {
    self.execute_your_custom_test().await
}
```

### Custom Validation

Implement custom validation logic in the `validate_expectation` method.

## Security Considerations

- The test runner spawns the MCP server as a child process by default
- Use `--standalone false` to connect to an existing server
- Be cautious with goal strings that might execute system commands
- Review test goals before running in production environments

## Contributing

When contributing new features:
1. Add appropriate test goal patterns
2. Document new goal types in this README
3. Include example GitHub Actions workflows
4. Test in VM environments

## License

Same as the Terminator project.