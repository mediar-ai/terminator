use anyhow::Context;
use crate::mcp_client;
use crate::cli::{InputType, McpRunArgs};

pub async fn run_workflow(transport: mcp_client::Transport, args: McpRunArgs) -> anyhow::Result<()> {
    use tracing::info;

    if args.verbose {
        std::env::set_var("RUST_LOG", "debug");
    }

    // Initialize simple logging (only if not already initialized)
    {
        use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
        let _ = tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "info".into()),
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

pub fn determine_input_type(input: &str, specified_type: InputType) -> InputType {
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

pub fn convert_gist_to_raw_url(gist_url: &str) -> anyhow::Result<String> {
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

pub async fn read_local_file(path: &str) -> anyhow::Result<String> {
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

pub async fn fetch_remote_content(url: &str) -> anyhow::Result<String> {
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
pub fn parse_workflow_content(content: &str) -> anyhow::Result<serde_json::Value> {
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
pub fn extract_workflow_from_wrapper(
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
pub fn validate_workflow(workflow: &serde_json::Value) -> anyhow::Result<()> {
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
