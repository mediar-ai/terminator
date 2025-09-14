use serde_json::Value;
use anyhow::Context;
use crate::cli::{InputType, McpRunArgs};
use super::{
    result::{WorkflowResult, WorkflowState},
    cron::extract_cron_from_workflow,
    validation::validate_workflow,
    parsing::parse_workflow_content,
    exec::execute_command_with_progress_and_retry,
    input::{
        determine_input_type,
        read_local_file, 
        convert_gist_to_raw_url,
        fetch_remote_content
    }
};

#[derive(Clone)]
pub enum Transport {
    Http(String),
    Stdio(Vec<String>),
}

pub async fn run_workflow(transport: Transport, args: McpRunArgs) -> anyhow::Result<()> {
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

    // Handle cron scheduling if specified in workflow
    if let Some(cron_expr) = extract_cron_from_workflow(&workflow_val) {
        info!(
            "🕐 Starting cron scheduler with workflow expression: {}",
            cron_expr
        );
        return run_workflow_with_cron(transport, args, &cron_expr).await;
    }

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

    let workflow_str = serde_json::to_string(&workflow_val)?;

    let result_json = execute_command_with_progress_and_retry(
        transport,
        "execute_sequence".to_string(),
        Some(workflow_str),
        true, // Show progress for workflow steps
        args.no_retry,
    )
    .await?;

    // Parse and display the workflow result
    let workflow_result = WorkflowResult::from_mcp_response(&result_json)?;

    // Display result in user-friendly format
    workflow_result.display();

    // If verbose mode, also show raw JSON
    if args.verbose {
        println!("📝 Raw MCP Response:");
        println!("{}", serde_json::to_string_pretty(&result_json)?);
    }

    // Exit with appropriate code based on success
    if !workflow_result.success {
        std::process::exit(1);
    }

    Ok(())
}

/// Execute workflow with cron scheduling
async fn run_workflow_with_cron(
    transport: Transport,
    args: McpRunArgs,
    cron_expr: &str,
) -> anyhow::Result<()> {
    use tokio_cron_scheduler::{Job, JobScheduler};
    use tracing::error;

    println!("🕐 Setting up cron scheduler...");
    println!("📅 Cron expression: {cron_expr}");
    println!("🔄 Workflow will run continuously at scheduled intervals");
    println!("💡 Press Ctrl+C to stop the scheduler");

    // Try to parse the cron expression to validate it (tokio-cron-scheduler will handle this)
    // We'll let tokio-cron-scheduler validate it when we create the job

    // For preview, we'll just show a generic message since calculating next times
    // with tokio-cron-scheduler is more complex
    println!("📋 Workflow will run according to cron schedule: {cron_expr}");
    println!("💡 Note: Exact execution times depend on system clock and scheduler timing");

    // Create scheduler
    let mut sched = JobScheduler::new().await?;

    // Clone transport for the job closure
    let transport_clone = transport.clone();
    let args_clone = args.clone();

    // Create the scheduled job
    let job = Job::new_async(cron_expr, move |_uuid, _lock| {
        let transport = transport_clone.clone();
        let args = args_clone.clone();

        Box::pin(async move {
            let start_time = std::time::Instant::now();
            println!(
                "\n🚀 Starting scheduled workflow execution at {}",
                chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
            );

            match run_workflow_once(transport, args).await {
                Ok(_) => {
                    let duration = start_time.elapsed();
                    println!(
                        "✅ Scheduled workflow completed successfully in {:.2}s",
                        duration.as_secs_f64()
                    );
                }
                Err(e) => {
                    let duration = start_time.elapsed();
                    println!(
                        "❌ Scheduled workflow failed after {:.2}s: {}",
                        duration.as_secs_f64(),
                        e
                    );
                }
            }
        })
    })?;

    // Add job to scheduler
    sched.add(job).await?;
    println!("✅ Cron job scheduled successfully");

    // Start the scheduler
    sched.start().await?;
    println!("▶️  Scheduler started - workflow will run at scheduled intervals");

    // Set up graceful shutdown
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel(1);

    // Spawn a task to handle Ctrl+C
    tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                println!("\n🛑 Received shutdown signal");
                let _ = shutdown_tx.send(()).await;
            }
            Err(e) => {
                error!("Failed to listen for shutdown signal: {}", e);
            }
        }
    });

    // Wait for shutdown signal
    let _ = shutdown_rx.recv().await;

    println!("🛑 Shutting down scheduler...");
    sched.shutdown().await?;
    println!("✅ Scheduler stopped successfully");

    Ok(())
}


/// Execute a single workflow run (used by cron scheduler)
async fn run_workflow_once(
    transport: Transport,
    args: McpRunArgs,
) -> anyhow::Result<()> {
    // Resolve actual input type (auto-detect if needed)
    let resolved_type = determine_input_type(&args.input, args.input_type);

    // Fetch workflow content
    let content = match resolved_type {
        InputType::File => read_local_file(&args.input).await?,
        InputType::Gist => {
            let raw_url = convert_gist_to_raw_url(&args.input)?;
            fetch_remote_content(&raw_url).await?
        }
        InputType::Raw => fetch_remote_content(&args.input).await?,
        InputType::Auto => unreachable!(),
    };

    // Parse workflow using the same robust logic as gist_executor
    let mut workflow_val = parse_workflow_content(&content)
        .with_context(|| format!("Failed to parse workflow from {}", args.input))?;

    // Validate workflow structure early to catch issues
    validate_workflow(&workflow_val).with_context(|| "Workflow validation failed")?;

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

    // For cron jobs, use simple execution to avoid connection spam
    let workflow_str = serde_json::to_string(&workflow_val)?;
    let result_json = execute_command_with_progress_and_retry(
        transport,
        "execute_sequence".to_string(),
        Some(workflow_str),
        true, // Show progress for workflow steps
        args.no_retry,
    )
    .await?;

    // Parse the workflow result
    let workflow_result = WorkflowResult::from_mcp_response(&result_json)?;

    // For cron jobs, log success/failure/skipped
    match workflow_result.state {
        WorkflowState::Success => {
            println!("   ✅ {}", workflow_result.message);
            if let Some(Value::Array(arr)) = &workflow_result.data {
                println!("   📊 Extracted {} items", arr.len());
            }
        }
        WorkflowState::Skipped => {
            println!("   ⏭️  {}", workflow_result.message);
            if let Some(Value::Object(data)) = &workflow_result.data {
                if let Some(reason) = data.get("reason").and_then(|r| r.as_str()) {
                    println!("   📝 Reason: {reason}");
                }
            }
        }
        WorkflowState::Failure => {
            println!("   ❌ {}", workflow_result.message);
            if let Some(error) = &workflow_result.error {
                println!("   ⚠️  {error}");
            }
        }
    }

    Ok(())
}
