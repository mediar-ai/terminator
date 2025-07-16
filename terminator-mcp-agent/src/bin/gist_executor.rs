use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use rmcp::handler::server::tool::Parameters;
use std::path::Path;
use terminator_mcp_agent::server::DesktopWrapper;
use terminator_mcp_agent::utils::{init_logging, ExecuteSequenceArgs};
use tracing::{error, info};

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Execute Terminator MCP workflow sequences from GitHub gists or local files",
    long_about = "Execute Terminator MCP workflow sequences from various sources:
  • GitHub gist URLs (https://gist.github.com/...)
  • Raw gist URLs (https://gist.githubusercontent.com/...)  
  • Local JSON/YAML files (relative or absolute paths)

Examples:
  gist_executor workflow.json
  gist_executor --verbose --output results.json workflow.yaml
  gist_executor --output workflow_results.json --no-open /path/to/workflow.json
  gist_executor -o results.json https://gist.github.com/user/abc123"
)]
struct Args {
    /// Input source - can be a GitHub gist URL, raw gist URL, or local file path (JSON/YAML)
    #[arg(help = "GitHub gist URL, raw gist URL, or path to local JSON/YAML file")]
    input: String,

    /// Input type
    #[arg(short, long, value_enum, default_value = "auto")]
    input_type: InputType,

    /// Dry run - parse and validate the workflow without executing
    #[arg(short, long)]
    dry_run: bool,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Stop on first error (default: true)
    #[arg(long)]
    no_stop_on_error: bool,

    /// Include detailed results (default: true)
    #[arg(long)]
    no_detailed_results: bool,

    /// Output results to JSON file (and open it)
    #[arg(short, long)]
    output: Option<String>,

    /// Don't automatically open the output file
    #[arg(long)]
    no_open: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum InputType {
    /// Auto-detect based on input string
    Auto,
    /// GitHub gist URL (https://gist.github.com/...)
    Gist,
    /// Raw gist URL (https://gist.githubusercontent.com/...)
    Raw,
    /// Local JSON/YAML file path
    File,
}

/// Filter out base64 content from JSON values to make terminal output readable
fn filter_base64_content(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            // Remove or summarize known base64 fields
            if let Some(serde_json::Value::Array(content_array)) = map.get_mut("content") {
                for item in content_array.iter_mut() {
                    if let serde_json::Value::Object(content_obj) = item {
                        // Check if this looks like base64 image content
                        if let Some(data) = content_obj.get("data") {
                            if let Some(data_str) = data.as_str() {
                                if data_str.len() > 1000
                                    && (data_str.starts_with("iVBORw0KGgo")
                                        || data_str.starts_with("/9j/"))
                                {
                                    // This looks like base64 image data
                                    content_obj.insert(
                                        "data".to_string(),
                                        serde_json::Value::String(format!(
                                            "[BASE64_IMAGE_DATA_FILTERED - {} bytes]",
                                            data_str.len()
                                        )),
                                    );
                                }
                            }
                        }
                        // Also check for type field indicating image
                        if let Some(type_val) = content_obj.get("type") {
                            if type_val.as_str() == Some("image") {
                                if let Some(data) = content_obj.get("data") {
                                    if let Some(data_str) = data.as_str() {
                                        content_obj.insert(
                                            "data".to_string(),
                                            serde_json::Value::String(format!(
                                                "[IMAGE_DATA_FILTERED - {} bytes]",
                                                data_str.len()
                                            )),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Handle results array that might contain base64 data
            if let Some(results) = map.get_mut("results") {
                filter_base64_content(results);
            }

            // Handle debug_info_on_failure that might contain large ui_tree data
            if let Some(serde_json::Value::Object(debug_map)) = map.get_mut("debug_info_on_failure")
            {
                if let Some(ui_tree) = debug_map.get("ui_tree") {
                    let ui_tree_str = serde_json::to_string(ui_tree).unwrap_or_default();
                    if ui_tree_str.len() > 5000 {
                        debug_map.insert(
                            "ui_tree".to_string(),
                            serde_json::Value::String(format!(
                                "[UI_TREE_DATA_FILTERED - {} bytes]",
                                ui_tree_str.len()
                            )),
                        );
                    }
                }
            }

            // Recursively filter nested objects
            for (_, v) in map.iter_mut() {
                filter_base64_content(v);
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                filter_base64_content(item);
            }
        }
        _ => {} // No filtering needed for other types
    }
}

/// Pretty-print JSON with nested JSON strings parsed and formatted
fn pretty_print_json_with_nested(value: &serde_json::Value) -> Result<String> {
    let mut pretty_value = value.clone();
    expand_nested_json(&mut pretty_value);
    Ok(serde_json::to_string_pretty(&pretty_value)?)
}

/// Recursively find and parse JSON strings within the structure
fn expand_nested_json(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            // Handle content array with text fields that might contain JSON
            if let Some(serde_json::Value::Array(content_array)) = map.get_mut("content") {
                for item in content_array.iter_mut() {
                    if let serde_json::Value::Object(content_obj) = item {
                        if let Some(serde_json::Value::String(text)) = content_obj.get("text") {
                            // Try to parse the text as JSON
                            if let Ok(parsed_json) = serde_json::from_str::<serde_json::Value>(text)
                            {
                                // Replace the string with the parsed JSON object
                                content_obj.insert("parsed_json".to_string(), parsed_json);
                                content_obj.insert(
                                    "text".to_string(),
                                    serde_json::Value::String(
                                        "[JSON_CONTENT_EXPANDED_BELOW]".to_string(),
                                    ),
                                );
                            }
                        }
                    }
                }
            }

            // Recursively process nested objects
            for (_, v) in map.iter_mut() {
                expand_nested_json(v);
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                expand_nested_json(item);
            }
        }
        _ => {} // No processing needed for other types
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if args.verbose {
        std::env::set_var("RUST_LOG", "debug");
    }

    init_logging()?;

    info!("Starting Terminator MCP Gist Executor");
    info!("Input: {}", args.input);
    info!("Input type: {:?}", args.input_type);

    // Determine input type and fetch content
    let json_content = match determine_input_type(&args.input, args.input_type) {
        InputType::File => {
            info!("Reading from local file: {}", args.input);
            read_local_file(&args.input).await?
        }
        InputType::Gist => {
            info!("Fetching from GitHub gist: {}", args.input);
            let raw_url = convert_gist_to_raw_url(&args.input)?;
            fetch_remote_content(&raw_url).await?
        }
        InputType::Raw => {
            info!("Fetching from raw URL: {}", args.input);
            fetch_remote_content(&args.input).await?
        }
        InputType::Auto => unreachable!(), // Should be resolved by determine_input_type
    };

    // Parse JSON content
    info!("Parsing content (JSON/YAML)...");

    let workflow: ExecuteSequenceArgs = parse_execute_sequence(&json_content)
        .context("Failed to parse input content as a workflow")?;

    info!(
        "Successfully parsed workflow with {} steps",
        workflow.steps.len()
    );

    if args.dry_run {
        info!("Dry run mode - validating workflow structure");
        validate_workflow(&workflow)?;
        println!("✅ Workflow validation successful!");
        println!("📊 Workflow Summary:");
        println!("   • Steps: {}", workflow.steps.len());
        println!(
            "   • Variables: {}",
            workflow.variables.as_ref().map_or(0, |v| v.len())
        );
        println!(
            "   • Selectors: {}",
            workflow
                .selectors
                .as_ref()
                .map_or(0, |v| v.as_object().map_or(0, |o| o.len()))
        );
        println!(
            "   • Stop on error: {}",
            workflow.stop_on_error.unwrap_or(true)
        );
        return Ok(());
    }

    // Initialize desktop wrapper
    info!("Initializing desktop automation...");
    let desktop_wrapper = DesktopWrapper::new().await?;

    // Override settings based on command line args
    let mut modified_workflow = workflow;
    if args.no_stop_on_error {
        modified_workflow.stop_on_error = Some(false);
    }
    if args.no_detailed_results {
        modified_workflow.include_detailed_results = Some(false);
    }

    // Execute the workflow
    info!("Executing workflow sequence...");
    match desktop_wrapper
        .execute_sequence(Parameters(modified_workflow))
        .await
    {
        Ok(result) => {
            info!("Workflow execution completed successfully");
            println!("✅ Execution completed!");

            // Prepare the results
            let mut filtered_result = serde_json::to_value(&result)?;
            filter_base64_content(&mut filtered_result);
            let pretty_json = pretty_print_json_with_nested(&filtered_result)?;

            // Write to file if output specified
            if let Some(output_file) = &args.output {
                write_and_open_json_file(&pretty_json, output_file, !args.no_open).await?;
            }

            // Extract and display results
            if args.verbose {
                println!("📋 Detailed Results:");
                println!("{pretty_json}");
            } else {
                println!("📊 Execution Summary: Check verbose output for details");
            }
        }
        Err(e) => {
            error!("Workflow execution failed: {}", e);

            // Parse the error to see if it contains base64 content and filter it
            let error_str = e.to_string();
            if error_str.len() > 2000 && error_str.contains("base64") {
                println!(
                    "❌ Execution failed: [Error message filtered - contains large base64 data]"
                );
                println!(
                    "💡 Use --verbose flag to see filtered details, or check logs for full error"
                );
            } else {
                println!("❌ Execution failed: {e}");
            }

            if args.verbose {
                // Try to parse error as JSON and filter if possible
                if let Ok(mut error_json) = serde_json::from_str::<serde_json::Value>(&error_str) {
                    filter_base64_content(&mut error_json);
                    let pretty_error = pretty_print_json_with_nested(&error_json)?;
                    
                    // Write error to file if output specified
                    if let Some(output_file) = &args.output {
                        let error_file = format!("{output_file}.error");
                        write_and_open_json_file(&pretty_error, &error_file, !args.no_open).await?;
                    }
                    
                    println!("📋 Filtered Error Details:");
                    println!("{pretty_error}");
                }
            }

            std::process::exit(1);
        }
    }

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
                // Default to file for any local path (relative or absolute)
                InputType::File
            }
        }
        other => other,
    }
}

fn convert_gist_to_raw_url(gist_url: &str) -> Result<String> {
    // Convert GitHub gist URL to raw URL
    // Example: https://gist.github.com/username/gist_id -> https://gist.githubusercontent.com/username/gist_id/raw

    if !gist_url.starts_with("https://gist.github.com/") {
        return Err(anyhow::anyhow!("Invalid GitHub gist URL format"));
    }

    let raw_url = gist_url.replace(
        "https://gist.github.com/",
        "https://gist.githubusercontent.com/",
    );

    // If URL doesn't end with /raw, add it
    if raw_url.ends_with("/raw") {
        Ok(raw_url)
    } else {
        Ok(format!("{raw_url}/raw"))
    }
}

async fn read_local_file(file_path: &str) -> Result<String> {
    // Validate file path and provide better error messages
    let path = Path::new(file_path);

    if !path.exists() {
        return Err(anyhow::anyhow!(
            "File does not exist: {} (resolved to: {})",
            file_path,
            path.canonicalize()
                .unwrap_or_else(|_| path.to_path_buf())
                .display()
        ));
    }

    if !path.is_file() {
        return Err(anyhow::anyhow!("Path is not a file: {}", path.display()));
    }

    // Check file extension for better error reporting
    if let Some(extension) = path.extension() {
        let ext = extension.to_string_lossy().to_lowercase();
        if !["json", "yaml", "yml"].contains(&ext.as_str()) {
            println!("⚠️  Warning: File extension '{ext}' is not .json, .yaml, or .yml. Attempting to parse anyway...");
        }
    }

    info!("Reading local file: {} ({})", file_path, path.display());

    tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("Failed to read file: {} ({})", file_path, path.display()))
}

async fn fetch_remote_content(url: &str) -> Result<String> {
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header("User-Agent", "terminator-mcp-gist-executor/1.0")
        .send()
        .await
        .with_context(|| format!("Failed to fetch URL: {url}"))?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "HTTP request failed with status: {} for URL: {}",
            response.status(),
            url
        ));
    }

    response
        .text()
        .await
        .with_context(|| format!("Failed to read response body from URL: {url}"))
}

fn validate_workflow(workflow: &ExecuteSequenceArgs) -> Result<()> {
    if workflow.steps.is_empty() {
        return Err(anyhow::anyhow!("Workflow must contain at least one step"));
    }

    for (i, step) in workflow.steps.iter().enumerate() {
        if step.tool_name.is_none() && step.group_name.is_none() {
            return Err(anyhow::anyhow!(
                "Step {} must have either tool_name or group_name",
                i
            ));
        }
        if step.tool_name.is_some() && step.group_name.is_some() {
            return Err(anyhow::anyhow!(
                "Step {} cannot have both tool_name and group_name",
                i
            ));
        }
    }

    // Validate variable schema if present
    if let Some(variables) = &workflow.variables {
        for (name, def) in variables {
            if name.is_empty() {
                return Err(anyhow::anyhow!("Variable name cannot be empty"));
            }
            if def.label.is_empty() {
                return Err(anyhow::anyhow!("Variable '{}' must have a label", name));
            }
        }
    }

    info!("Workflow validation passed");
    Ok(())
}

/// Parse the input content (JSON or YAML) into an `ExecuteSequenceArgs` workflow.
///
/// The function attempts the following strategies in order:
/// 1. Direct JSON deserialization into `ExecuteSequenceArgs`.
/// 2. Direct YAML deserialization into `ExecuteSequenceArgs`.
/// 3. JSON wrapper object containing `tool_name == "execute_sequence"` and `arguments`.
/// 4. YAML wrapper object with the same structure as #3.
fn parse_execute_sequence(content: &str) -> Result<ExecuteSequenceArgs> {
    // 1) Try direct JSON -> ExecuteSequenceArgs
    if let Ok(wf) = serde_json::from_str::<ExecuteSequenceArgs>(content) {
        return Ok(wf);
    }

    // 2) Try direct YAML -> ExecuteSequenceArgs
    if let Ok(wf) = serde_yaml::from_str::<ExecuteSequenceArgs>(content) {
        return Ok(wf);
    }

    // 3) Try JSON wrapper
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(content) {
        if let Some(wf) = extract_from_wrapper(&val)? {
            return Ok(wf);
        }
    }

    // 4) Try YAML wrapper
    if let Ok(val) = serde_yaml::from_str::<serde_json::Value>(content) {
        if let Some(wf) = extract_from_wrapper(&val)? {
            return Ok(wf);
        }
    }

    Err(anyhow::anyhow!(
        "Unable to parse content as JSON or YAML ExecuteSequenceArgs or wrapper"
    ))
}

/// Attempt to extract `ExecuteSequenceArgs` from a wrapper object produced by tool calls.
fn extract_from_wrapper(value: &serde_json::Value) -> Result<Option<ExecuteSequenceArgs>> {
    if let Some(tool_name) = value.get("tool_name") {
        if tool_name == "execute_sequence" {
            if let Some(arguments) = value.get("arguments") {
                let wf = serde_json::from_value::<ExecuteSequenceArgs>(arguments.clone())
                    .context("Failed to deserialize 'arguments' as ExecuteSequenceArgs")?;
                return Ok(Some(wf));
            } else {
                return Err(anyhow::anyhow!("Tool call missing 'arguments' field"));
            }
        }
    }
    Ok(None)
}

/// Write JSON content to a file and optionally open it
async fn write_and_open_json_file(
    content: &str,
    file_path: &str,
    should_open: bool,
) -> Result<()> {
    // Write to file
    tokio::fs::write(file_path, content)
        .await
        .with_context(|| format!("Failed to write JSON to file: {file_path}"))?;

    println!("📄 Results written to: {file_path}");

    // Open file if requested
    if should_open {
        open_file_with_default_program(file_path)?;
    }

    Ok(())
}

/// Open a file with the system's default program
fn open_file_with_default_program(file_path: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", file_path])
            .spawn()
            .with_context(|| format!("Failed to open file: {file_path}"))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(file_path)
            .spawn()
            .with_context(|| format!("Failed to open file: {}", file_path))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(file_path)
            .spawn()
            .with_context(|| format!("Failed to open file: {}", file_path))?;
    }

    println!("🚀 Opening file with default program: {file_path}");
    Ok(())
}
