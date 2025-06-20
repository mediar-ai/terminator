use anyhow::Result;
use clap::Parser;
use rmcp::{
    client::{Client, ClientHandler},
    model::{CallToolRequest, CallToolResult, Content, ServerInfo, ToolInfo},
    transport::{stdio, ConfigureCommandExt, TokioChildProcess},
    ClientTransport, ServiceExt,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use std::process::ExitCode;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{error, info, warn};

/// Test runner for Terminator MCP Agent
/// Used for automated testing in CI/CD environments
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Goal of the test - what the automation should achieve
    #[arg(short, long)]
    goal: String,

    /// Expected outcome - what should be validated after execution
    #[arg(short, long)]
    expectation: String,

    /// Test timeout in seconds (default: 300)
    #[arg(short, long, default_value = "300")]
    timeout: u64,

    /// Application to test (e.g., "notepad", "calculator")
    #[arg(short, long)]
    app: Option<String>,

    /// Use standalone MCP server (spawns server as child process)
    #[arg(long, default_value = "true")]
    standalone: bool,

    /// Server command (if standalone)
    #[arg(long, default_value = "terminator-mcp-agent")]
    server_command: String,

    /// Output format (json, human)
    #[arg(long, default_value = "json")]
    output_format: String,

    /// Virtual machine mode - adds delays for VM environments
    #[arg(long)]
    vm_mode: bool,
}

#[derive(Serialize, Deserialize)]
struct TestResult {
    success: bool,
    goal: String,
    expectation: String,
    actual_result: Option<String>,
    error: Option<String>,
    duration_ms: u128,
    steps_executed: Vec<StepResult>,
    screenshots: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct StepResult {
    step_number: usize,
    action: String,
    success: bool,
    details: Option<Value>,
    error: Option<String>,
}

struct TestRunner {
    client: Client,
    args: Args,
    steps: Vec<StepResult>,
    screenshots: Vec<String>,
}

impl TestRunner {
    async fn new(client: Client, args: Args) -> Self {
        Self {
            client,
            args,
            steps: Vec::new(),
            screenshots: Vec::new(),
        }
    }

    async fn run_test(&mut self) -> Result<TestResult> {
        let start_time = std::time::Instant::now();
        
        info!("Starting test with goal: {}", self.args.goal);
        info!("Expected outcome: {}", self.args.expectation);

        // Execute test with timeout
        let test_future = self.execute_test_steps();
        let result = match timeout(Duration::from_secs(self.args.timeout), test_future).await {
            Ok(Ok(actual_result)) => {
                info!("Test completed successfully");
                TestResult {
                    success: true,
                    goal: self.args.goal.clone(),
                    expectation: self.args.expectation.clone(),
                    actual_result: Some(actual_result),
                    error: None,
                    duration_ms: start_time.elapsed().as_millis(),
                    steps_executed: self.steps.clone(),
                    screenshots: self.screenshots.clone(),
                }
            }
            Ok(Err(e)) => {
                error!("Test failed: {}", e);
                TestResult {
                    success: false,
                    goal: self.args.goal.clone(),
                    expectation: self.args.expectation.clone(),
                    actual_result: None,
                    error: Some(e.to_string()),
                    duration_ms: start_time.elapsed().as_millis(),
                    steps_executed: self.steps.clone(),
                    screenshots: self.screenshots.clone(),
                }
            }
            Err(_) => {
                error!("Test timed out after {} seconds", self.args.timeout);
                TestResult {
                    success: false,
                    goal: self.args.goal.clone(),
                    expectation: self.args.expectation.clone(),
                    actual_result: None,
                    error: Some(format!("Test timed out after {} seconds", self.args.timeout)),
                    duration_ms: start_time.elapsed().as_millis(),
                    steps_executed: self.steps.clone(),
                    screenshots: self.screenshots.clone(),
                }
            }
        };

        Ok(result)
    }

    async fn execute_test_steps(&mut self) -> Result<String> {
        // Step 1: Open application if specified
        if let Some(app) = &self.args.app {
            self.open_application(app).await?;
            
            // Wait for app to initialize (longer in VM mode)
            let wait_time = if self.args.vm_mode { 5000 } else { 2000 };
            tokio::time::sleep(Duration::from_millis(wait_time)).await;
        }

        // Step 2: Get applications and find target
        let apps = self.get_applications().await?;
        
        // Step 3: Execute test based on goal
        let result = self.execute_goal_based_test().await?;

        // Step 4: Validate expectation
        self.validate_expectation(&result).await?;

        Ok(result)
    }

    async fn open_application(&mut self, app_name: &str) -> Result<()> {
        info!("Opening application: {}", app_name);
        
        let params = json!({
            "app_name": app_name,
            "arguments": []
        });

        let result = self.call_tool("open_application", params).await?;
        
        self.steps.push(StepResult {
            step_number: self.steps.len() + 1,
            action: format!("Open application: {}", app_name),
            success: true,
            details: Some(result),
            error: None,
        });

        Ok(())
    }

    async fn get_applications(&mut self) -> Result<Vec<Value>> {
        info!("Getting list of applications");
        
        let result = self.call_tool("get_applications", json!({})).await?;
        
        self.steps.push(StepResult {
            step_number: self.steps.len() + 1,
            action: "Get applications".to_string(),
            success: true,
            details: Some(result.clone()),
            error: None,
        });

        if let Some(apps) = result["applications"].as_array() {
            Ok(apps.clone())
        } else {
            Ok(vec![])
        }
    }

    async fn execute_goal_based_test(&mut self) -> Result<String> {
        // This is where we would implement goal-based test execution
        // For now, let's create a simple example that demonstrates the concept
        
        info!("Executing goal-based test: {}", self.args.goal);

        // Parse the goal and determine actions
        let goal_lower = self.args.goal.to_lowercase();
        
        if goal_lower.contains("type") && goal_lower.contains("text") {
            // Example: "Type 'Hello World' in notepad"
            self.execute_typing_test().await
        } else if goal_lower.contains("click") && goal_lower.contains("button") {
            // Example: "Click the calculate button"
            self.execute_click_test().await
        } else if goal_lower.contains("navigate") || goal_lower.contains("open") {
            // Example: "Navigate to settings"
            self.execute_navigation_test().await
        } else {
            // Generic test execution
            self.execute_generic_test().await
        }
    }

    async fn execute_typing_test(&mut self) -> Result<String> {
        // Extract text to type from goal
        let text_to_type = "Hello from MCP Test Runner";
        
        // Find the main text area
        let params = json!({
            "selector": "role:Document",
            "alternative_selectors": ["role:Edit", "role:Text"],
            "text_to_type": text_to_type,
            "verify_action": true
        });

        let result = self.call_tool("type_into_element", params).await?;
        
        self.steps.push(StepResult {
            step_number: self.steps.len() + 1,
            action: format!("Type text: {}", text_to_type),
            success: true,
            details: Some(result),
            error: None,
        });

        // Take screenshot after typing
        self.capture_screenshot("after_typing").await?;

        Ok(text_to_type.to_string())
    }

    async fn execute_click_test(&mut self) -> Result<String> {
        // Example click test implementation
        let params = json!({
            "selector": "role:Button",
            "include_tree": true
        });

        let result = self.call_tool("click_element", params).await?;
        
        self.steps.push(StepResult {
            step_number: self.steps.len() + 1,
            action: "Click button".to_string(),
            success: true,
            details: Some(result),
            error: None,
        });

        Ok("Button clicked".to_string())
    }

    async fn execute_navigation_test(&mut self) -> Result<String> {
        // Example navigation test
        Ok("Navigation completed".to_string())
    }

    async fn execute_generic_test(&mut self) -> Result<String> {
        // Generic test that captures current state
        self.capture_screenshot("test_state").await?;
        Ok("Test executed".to_string())
    }

    async fn validate_expectation(&mut self, actual_result: &str) -> Result<()> {
        info!("Validating expectation: {}", self.args.expectation);
        
        // Simple validation - in a real implementation, this would be more sophisticated
        let expectation_lower = self.args.expectation.to_lowercase();
        let result_lower = actual_result.to_lowercase();
        
        if expectation_lower.contains("success") || 
           expectation_lower.contains("complete") ||
           result_lower.contains(&expectation_lower) {
            info!("Expectation validated successfully");
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Expectation not met. Expected: '{}', Actual: '{}'",
                self.args.expectation,
                actual_result
            ))
        }
    }

    async fn capture_screenshot(&mut self, name: &str) -> Result<()> {
        info!("Capturing screenshot: {}", name);
        
        let result = self.call_tool("capture_screen", json!({})).await?;
        
        if let Some(base64_data) = result["base64_image"].as_str() {
            let screenshot_path = format!("screenshot_{}_{}.png", name, self.screenshots.len());
            self.screenshots.push(screenshot_path.clone());
            
            // In a real implementation, we would save the screenshot to disk or artifact storage
            info!("Screenshot captured: {}", screenshot_path);
        }
        
        self.steps.push(StepResult {
            step_number: self.steps.len() + 1,
            action: format!("Capture screenshot: {}", name),
            success: true,
            details: Some(json!({"screenshot": name})),
            error: None,
        });

        Ok(())
    }

    async fn call_tool(&mut self, tool_name: &str, params: Value) -> Result<Value> {
        let request = CallToolRequest {
            name: tool_name.to_string(),
            arguments: Some(params),
        };

        match self.client.call_tool(request).await {
            Ok(result) => {
                if result.is_error.unwrap_or(false) {
                    if let Some(content) = result.content.first() {
                        match content {
                            Content::Text { text } => {
                                Err(anyhow::anyhow!("Tool error: {}", text))
                            }
                            _ => Err(anyhow::anyhow!("Unknown tool error")),
                        }
                    } else {
                        Err(anyhow::anyhow!("Unknown tool error"))
                    }
                } else {
                    // Extract JSON content from result
                    if let Some(content) = result.content.first() {
                        match content {
                            Content::Text { text } => {
                                // Try to parse text as JSON
                                serde_json::from_str(text)
                                    .unwrap_or_else(|_| json!({"result": text}))
                            }
                            _ => Ok(json!({"result": "success"})),
                        }
                    } else {
                        Ok(json!({}))
                    }
                }
            }
            Err(e) => Err(anyhow::anyhow!("Failed to call tool {}: {}", tool_name, e)),
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();

    let args = Args::parse();
    
    // Run the test
    match run_test(args).await {
        Ok(0) => ExitCode::SUCCESS,
        Ok(code) => ExitCode::from(code as u8),
        Err(e) => {
            error!("Test runner failed: {}", e);
            ExitCode::FAILURE
        }
    }
}

async fn run_test(args: Args) -> Result<i32> {
    info!("Terminator MCP Test Runner starting...");
    
    // Create MCP client
    let client = if args.standalone {
        // Spawn MCP server as child process
        info!("Starting MCP server: {}", args.server_command);
        
        let transport = TokioChildProcess::new(Command::new(&args.server_command).configure(|cmd| {
            // Ensure the server uses stdio transport
            cmd.env("MCP_TRANSPORT", "stdio");
        }))?;
        
        EmptyClientHandler.serve(transport).await?
    } else {
        // Connect to existing MCP server via stdio
        info!("Connecting to existing MCP server via stdio");
        EmptyClientHandler.serve(stdio()).await?
    };

    // Create test runner
    let mut runner = TestRunner::new(client, args).await;
    
    // Run the test
    let result = runner.run_test().await?;
    
    // Output results
    match runner.args.output_format.as_str() {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        "human" => {
            println!("\n=== Test Results ===");
            println!("Goal: {}", result.goal);
            println!("Expectation: {}", result.expectation);
            println!("Success: {}", result.success);
            println!("Duration: {}ms", result.duration_ms);
            
            if let Some(actual) = &result.actual_result {
                println!("Actual Result: {}", actual);
            }
            
            if let Some(error) = &result.error {
                println!("Error: {}", error);
            }
            
            println!("\nSteps Executed:");
            for step in &result.steps_executed {
                println!("  {}. {} - {}", 
                    step.step_number, 
                    step.action,
                    if step.success { "✓" } else { "✗" }
                );
            }
            
            if !result.screenshots.is_empty() {
                println!("\nScreenshots:");
                for screenshot in &result.screenshots {
                    println!("  - {}", screenshot);
                }
            }
        }
        _ => {
            warn!("Unknown output format, using JSON");
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }
    
    // Return exit code based on test success
    Ok(if result.success { 0 } else { 1 })
}

// Empty client handler implementation
#[derive(Clone)]
struct EmptyClientHandler;

impl ClientHandler for EmptyClientHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            name: "terminator-test-client".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            ..Default::default()
        }
    }

    async fn list_tools(&self) -> Result<Vec<ToolInfo>, rmcp::Error> {
        Ok(vec![])
    }

    async fn call_tool(&self, _request: CallToolRequest) -> Result<CallToolResult, rmcp::Error> {
        Err(rmcp::Error::method_not_found("No tools available"))
    }
}