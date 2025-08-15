use anyhow::Context;
use crate::command;
use crate::cli::{InputType, McpRunArgs};
use crate::utils::Transport;
use super::{
    validation::validate_workflow,
    parsing::parse_workflow_content,
    input::{
        determine_input_type,
        read_local_file, 
        convert_gist_to_raw_url,
        fetch_remote_content
    }
};

pub async fn run_workflow(transport: Transport, args: McpRunArgs) -> anyhow::Result<()> {
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

    command::execute_command(
        transport,
        "execute_sequence".to_string(),
        Some(workflow_str),
    )
    .await?;

    Ok(())
}
