use crate::helpers::substitute_variables;
use crate::output_parser;
use crate::server::extract_content_json;
use crate::telemetry::{StepSpan, WorkflowSpan};
use crate::utils::{DesktopWrapper, ExecuteSequenceArgs, SequenceItem, ToolCall, ToolGroup};
use rmcp::model::{CallToolResult, Content};
use rmcp::service::{Peer, RequestContext, RoleServer};
use rmcp::ErrorData as McpError;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{debug, info, warn};

impl DesktopWrapper {
    // Get the state file path for a workflow
    async fn get_state_file_path(workflow_url: &str) -> Option<PathBuf> {
        if let Some(file_path) = workflow_url.strip_prefix("file://") {
            let workflow_path = Path::new(file_path);
            let workflow_dir = workflow_path.parent()?;
            let workflow_name = workflow_path.file_stem()?;

            let state_file = workflow_dir
                .join(".workflow_state")
                .join(format!("{}.json", workflow_name.to_string_lossy()));

            Some(state_file)
        } else {
            None
        }
    }

    // Save env state after any step that modifies it
    async fn save_workflow_state(
        workflow_url: &str,
        step_id: Option<&str>,
        step_index: usize,
        env: &serde_json::Value,
    ) -> Result<(), McpError> {
        if let Some(state_file) = Self::get_state_file_path(workflow_url).await {
            if let Some(state_dir) = state_file.parent() {
                tokio::fs::create_dir_all(state_dir).await.map_err(|e| {
                    McpError::internal_error(format!("Failed to create state directory: {e}"), None)
                })?;
            }

            let state = json!({
                "last_updated": chrono::Utc::now().to_rfc3339(),
                "last_step_id": step_id,
                "last_step_index": step_index,
                "workflow_file": Path::new(workflow_url.strip_prefix("file://").unwrap_or(workflow_url))
                    .file_name()
                    .and_then(|n| n.to_str()),
                "env": env,
            });

            tokio::fs::write(
                &state_file,
                serde_json::to_string_pretty(&state).map_err(|e| {
                    McpError::internal_error(format!("Failed to serialize state: {e}"), None)
                })?,
            )
            .await
            .map_err(|e| {
                McpError::internal_error(format!("Failed to write state file: {e}"), None)
            })?;

            debug!("Saved workflow state to: {:?}", state_file);
        }
        Ok(())
    }

    // Load env state when starting from a specific step
    async fn load_workflow_state(
        workflow_url: &str,
    ) -> Result<Option<serde_json::Value>, McpError> {
        if let Some(state_file) = Self::get_state_file_path(workflow_url).await {
            if state_file.exists() {
                let content = tokio::fs::read_to_string(&state_file).await.map_err(|e| {
                    McpError::internal_error(format!("Failed to read state file: {e}"), None)
                })?;
                let state: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
                    McpError::internal_error(format!("Failed to parse state file: {e}"), None)
                })?;

                if let Some(env) = state.get("env") {
                    debug!(
                        "Loaded workflow state from step {} ({})",
                        state["last_step_index"],
                        state["last_step_id"].as_str().unwrap_or("unknown")
                    );
                    return Ok(Some(env.clone()));
                }
            } else {
                debug!("No saved workflow state found at: {:?}", state_file);
            }
        }
        Ok(None)
    }

    pub async fn execute_sequence_impl(
        &self,
        peer: Peer<RoleServer>,
        request_context: RequestContext<RoleServer>,
        mut args: ExecuteSequenceArgs,
    ) -> Result<CallToolResult, McpError> {
        // Validate that either URL or steps are provided
        if args.url.is_none() && args.steps.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
            return Err(McpError::invalid_params(
                "Either 'url' or 'steps' must be provided".to_string(),
                None,
            ));
        }

        // Handle URL fetching if provided
        if let Some(url) = &args.url {
            info!("Fetching workflow from URL: {}", url);

            let workflow_content = if url.starts_with("file://") {
                // Handle local file URLs
                let file_path = url.strip_prefix("file://").unwrap_or(url);
                info!("Reading file from path: {}", file_path);

                // Store the workflow directory for relative path resolution
                let workflow_path = Path::new(file_path);
                if let Some(parent_dir) = workflow_path.parent() {
                    let mut workflow_dir_guard = self.current_workflow_dir.lock().await;
                    *workflow_dir_guard = Some(parent_dir.to_path_buf());
                    info!("Stored workflow directory: {:?}", parent_dir);
                }

                let content = std::fs::read_to_string(file_path).map_err(|e| {
                    McpError::invalid_params(
                        format!("Failed to read local workflow file: {e}"),
                        Some(json!({"url": url, "error": e.to_string()})),
                    )
                })?;
                info!("File content length: {}", content.len());
                content
            } else if url.starts_with("http://") || url.starts_with("https://") {
                // Handle HTTP/HTTPS URLs
                let response = reqwest::get(url).await.map_err(|e| {
                    McpError::invalid_params(
                        format!("Failed to fetch workflow from URL: {e}"),
                        Some(json!({"url": url, "error": e.to_string()})),
                    )
                })?;

                if !response.status().is_success() {
                    return Err(McpError::invalid_params(
                        format!("HTTP error fetching workflow: {}", response.status()),
                        Some(json!({"url": url, "status": response.status().as_u16()})),
                    ));
                }

                response.text().await.map_err(|e| {
                    McpError::invalid_params(
                        format!("Failed to read response text: {e}"),
                        Some(json!({"url": url, "error": e.to_string()})),
                    )
                })?
            } else {
                return Err(McpError::invalid_params(
                    "URL must start with http://, https://, or file://".to_string(),
                    Some(json!({"url": url})),
                ));
            };

            // Debug: Log the raw YAML content
            debug!(
                "Raw YAML content (first 500 chars): {}",
                workflow_content.chars().take(500).collect::<String>()
            );

            // Parse the fetched YAML workflow
            let remote_workflow: ExecuteSequenceArgs = match serde_yaml::from_str::<
                ExecuteSequenceArgs,
            >(&workflow_content)
            {
                Ok(wf) => {
                    info!(
                        "Successfully parsed YAML. Steps count: {}",
                        wf.steps.as_ref().map(|s| s.len()).unwrap_or(0)
                    );
                    wf
                }
                Err(e) => {
                    tracing::error!("Failed to parse YAML: {}", e);
                    return Err(McpError::invalid_params(
                        format!("Failed to parse remote workflow YAML: {e}"),
                        Some(
                            json!({"url": url, "error": e.to_string(), "content_preview": workflow_content.chars().take(200).collect::<String>()}),
                        ),
                    ));
                }
            };

            // Debug: Log what we got from the remote workflow
            info!(
                "Remote workflow parsed - steps present: {}, steps count: {}",
                remote_workflow.steps.is_some(),
                remote_workflow.steps.as_ref().map(|s| s.len()).unwrap_or(0)
            );

            // Merge remote workflow with local overrides
            // Only use remote steps if local steps are empty or None
            if args.steps.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
                args.steps = remote_workflow.steps;
            }
            // Also merge troubleshooting steps if not provided locally
            if args
                .troubleshooting
                .as_ref()
                .map(|t| t.is_empty())
                .unwrap_or(true)
            {
                args.troubleshooting = remote_workflow.troubleshooting;
            }
            if args.variables.is_none() {
                args.variables = remote_workflow.variables;
            }
            if args.selectors.is_none() {
                args.selectors = remote_workflow.selectors;
            }
            // Keep local inputs, variables, and other settings as they override remote ones

            info!(
                "After merge - args.steps present: {}, count: {}",
                args.steps.is_some(),
                args.steps.as_ref().map(|s| s.len()).unwrap_or(0)
            );

            info!(
                "Successfully loaded workflow from URL with {} steps",
                args.steps.as_ref().map(|s| s.len()).unwrap_or(0)
            );
        }

        // Handle backward compatibility: 'continue' is opposite of 'stop_on_error'
        let stop_on_error = if let Some(continue_exec) = args.r#continue {
            !continue_exec // continue=true means stop_on_error=false
        } else {
            args.stop_on_error.unwrap_or(true)
        };

        // Handle verbosity levels
        // quiet: minimal output (just success/failure)
        // normal: moderate output (includes tool results/logs but may omit some metadata)
        // verbose: full output (includes all details and metadata)
        let include_detailed = match args.verbosity.as_deref() {
            Some("quiet") => false,
            Some("verbose") => true,
            Some("normal") | None => args.include_detailed_results.unwrap_or(false), // Changed default to false
            _ => args.include_detailed_results.unwrap_or(false), // Changed default to false
        };

        // Re-enabling validation logic
        if let Some(variable_schema) = &args.variables {
            let inputs_map = args
                .inputs
                .as_ref()
                .and_then(|v| v.as_object())
                .cloned()
                .unwrap_or_default();

            for (key, def) in variable_schema {
                let value = inputs_map.get(key).or(def.default.as_ref());

                match value {
                    Some(val) => {
                        // Validate the value against the definition
                        match def.r#type {
                            crate::utils::VariableType::String => {
                                if !val.is_string() {
                                    return Err(McpError::invalid_params(
                                        format!("Variable '{key}' must be a string."),
                                        Some(json!({"value": val})),
                                    ));
                                }
                            }
                            crate::utils::VariableType::Number => {
                                if !val.is_number() {
                                    return Err(McpError::invalid_params(
                                        format!("Variable '{key}' must be a number."),
                                        Some(json!({"value": val})),
                                    ));
                                }
                            }
                            crate::utils::VariableType::Boolean => {
                                if !val.is_boolean() {
                                    return Err(McpError::invalid_params(
                                        format!("Variable '{key}' must be a boolean."),
                                        Some(json!({"value": val})),
                                    ));
                                }
                            }
                            crate::utils::VariableType::Enum => {
                                let val_str = val.as_str().ok_or_else(|| {
                                    McpError::invalid_params(
                                        format!("Enum variable '{key}' must be a string."),
                                        Some(json!({"value": val})),
                                    )
                                })?;
                                if let Some(options) = &def.options {
                                    if !options.contains(&val_str.to_string()) {
                                        return Err(McpError::invalid_params(
                                            format!("Variable '{key}' has an invalid value."),
                                            Some(json!({
                                                "value": val_str,
                                                "allowed_options": options
                                            })),
                                        ));
                                    }
                                }
                            }
                            crate::utils::VariableType::Array => {
                                if !val.is_array() {
                                    return Err(McpError::invalid_params(
                                        format!("Variable '{key}' must be an array."),
                                        Some(json!({"value": val})),
                                    ));
                                }
                            }
                            crate::utils::VariableType::Object => {
                                if !val.is_object() {
                                    return Err(McpError::invalid_params(
                                        format!("Variable '{key}' must be an object."),
                                        Some(json!({"value": val})),
                                    ));
                                }
                            }
                        }
                    }
                    None => {
                        if def.required.unwrap_or(true) {
                            return Err(McpError::invalid_params(
                                format!("Required variable '{key}' is missing."),
                                None,
                            ));
                        }
                    }
                }
            }
        }

        // Build the execution context. It's a combination of the 'inputs' and 'selectors'.
        // The context is a simple, flat map of variables that will be used for substitution in tool arguments.
        let mut execution_context_map = serde_json::Map::new();

        // First, populate with default values from variables schema
        if let Some(variable_schema) = &args.variables {
            for (key, def) in variable_schema {
                if let Some(default_value) = &def.default {
                    execution_context_map.insert(key.clone(), default_value.clone());
                }
            }
        }

        // Then override with user-provided inputs (inputs take precedence over defaults)
        if let Some(inputs) = &args.inputs {
            // Validate inputs is an object
            if let Err(err) = crate::utils::validate_inputs(inputs) {
                return Err(McpError::invalid_params(
                    format!(
                        "Invalid inputs: {} expected {}, got {}",
                        err.field, err.expected, err.actual
                    ),
                    None,
                ));
            }
            if let Some(inputs_map) = inputs.as_object() {
                for (key, value) in inputs_map {
                    execution_context_map.insert(key.clone(), value.clone());
                }
            }
        }

        if let Some(selectors) = args.selectors.clone() {
            // Validate selectors
            if let Err(err) = crate::utils::validate_selectors(&selectors) {
                return Err(McpError::invalid_params(
                    format!(
                        "Invalid selectors: {} expected {}, got {}",
                        err.field, err.expected, err.actual
                    ),
                    None,
                ));
            }
            // If selectors is a string, parse it as JSON first
            let selectors_value = if let serde_json::Value::String(s) = &selectors {
                match serde_json::from_str::<serde_json::Value>(s) {
                    Ok(parsed) => parsed,
                    Err(_) => selectors, // If parsing fails, treat it as a raw string
                }
            } else {
                selectors
            };
            execution_context_map.insert("selectors".to_string(), selectors_value);
        }
        // Initialize an internal env bag for dynamic, step-to-step values set at runtime (e.g., via JS)
        execution_context_map.insert("env".to_string(), json!({}));

        // NEW: Check if we should start from a specific step
        let start_from_index = if let Some(start_step) = &args.start_from_step {
            // Find the step index by ID
            args.steps
                .as_ref()
                .and_then(|steps| steps.iter().position(|s| s.id.as_ref() == Some(start_step)))
                .ok_or_else(|| {
                    McpError::invalid_params(
                        format!("start_from_step '{start_step}' not found in workflow steps"),
                        Some(json!({
                            "requested_step": start_step,
                            "available_steps": args.steps.as_ref().map(|steps|
                                steps.iter()
                                    .filter_map(|s| s.id.as_ref())
                                    .collect::<Vec<_>>()
                            ).unwrap_or_default()
                        })),
                    )
                })?
        } else {
            0
        };

        // NEW: Check if we should end at a specific step
        let end_at_index = if let Some(end_step) = &args.end_at_step {
            // Find the step index by ID (inclusive)
            args.steps
                .as_ref()
                .and_then(|steps| steps.iter().position(|s| s.id.as_ref() == Some(end_step)))
                .ok_or_else(|| {
                    McpError::invalid_params(
                        format!("end_at_step '{end_step}' not found in workflow steps"),
                        Some(json!({
                            "requested_step": end_step,
                            "available_steps": args.steps.as_ref().map(|steps|
                                steps.iter()
                                    .filter_map(|s| s.id.as_ref())
                                    .collect::<Vec<_>>()
                            ).unwrap_or_default()
                        })),
                    )
                })?
        } else {
            // No end_at_step specified, run to the end
            args.steps.as_ref().map(|s| s.len() - 1).unwrap_or(0)
        };

        // NEW: Load saved state if starting from a specific step
        if start_from_index > 0 {
            if let Some(url) = &args.url {
                if let Some(saved_env) = Self::load_workflow_state(url).await? {
                    execution_context_map.insert("env".to_string(), saved_env);
                    debug!(
                        "Loaded saved env state for resuming from step {}",
                        start_from_index
                    );
                }
            }
        }

        let execution_context = serde_json::Value::Object(execution_context_map.clone());
        debug!(
            "Executing sequence with context: {}",
            serde_json::to_string_pretty(&execution_context).unwrap_or_default()
        );
        info!(
            "Starting execute_sequence: steps={}, stop_on_error={}, include_detailed_results={}",
            args.steps.as_ref().map(|s| s.len()).unwrap_or(0),
            stop_on_error,
            include_detailed
        );

        // Start workflow telemetry span
        let workflow_name = "execute_sequence";
        let mut workflow_span = WorkflowSpan::new(workflow_name);
        workflow_span.set_attribute(
            "workflow.total_steps",
            args.steps
                .as_ref()
                .map(|s| s.len())
                .unwrap_or(0)
                .to_string(),
        );
        workflow_span.set_attribute("workflow.stop_on_error", stop_on_error.to_string());

        // Convert flattened SequenceStep to internal SequenceItem representation
        let mut sequence_items = Vec::new();
        let empty_steps = Vec::new();
        let steps = args.steps.as_ref().unwrap_or(&empty_steps);
        for step in steps {
            let item = if let Some(tool_name) = &step.tool_name {
                // Parse delay from either delay_ms or human-readable delay field
                let delay_ms = if let Some(delay_str) = &step.delay {
                    match crate::duration_parser::parse_duration(delay_str) {
                        Ok(ms) => Some(ms),
                        Err(e) => {
                            warn!("Failed to parse delay '{}': {}", delay_str, e);
                            step.delay_ms // Fall back to delay_ms
                        }
                    }
                } else {
                    step.delay_ms
                };

                let tool_call = ToolCall {
                    tool_name: tool_name.clone(),
                    arguments: step.arguments.clone().unwrap_or(serde_json::json!({})),
                    continue_on_error: step.continue_on_error,
                    delay_ms,
                    id: step.id.clone(),
                };
                SequenceItem::Tool { tool_call }
            } else if let Some(group_name) = &step.group_name {
                let tool_group = ToolGroup {
                    group_name: group_name.clone(),
                    steps: step
                        .steps
                        .clone()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|s| ToolCall {
                            tool_name: s.tool_name,
                            arguments: s.arguments,
                            continue_on_error: s.continue_on_error,
                            delay_ms: s.delay_ms,
                            id: s.id,
                        })
                        .collect(),
                    skippable: step.skippable,
                };
                SequenceItem::Group { tool_group }
            } else {
                return Err(McpError::invalid_params(
                    "Each step must have either tool_name (for single tools) or group_name (for groups)",
                    Some(json!({"invalid_step": step})),
                ));
            };
            sequence_items.push(item);
        }

        // Add troubleshooting steps to the sequence (they won't execute unless jumped to via fallback_id)
        if let Some(troubleshooting) = &args.troubleshooting {
            info!(
                "Adding {} troubleshooting steps to workflow (accessible only via fallback_id)",
                troubleshooting.len()
            );
            for step in troubleshooting {
                let item = if let Some(tool_name) = &step.tool_name {
                    // Parse delay from either delay_ms or human-readable delay field
                    let delay_ms = if let Some(delay_str) = &step.delay {
                        match crate::duration_parser::parse_duration(delay_str) {
                            Ok(ms) => Some(ms),
                            Err(e) => {
                                warn!("Failed to parse delay '{}': {}", delay_str, e);
                                step.delay_ms // Fall back to delay_ms
                            }
                        }
                    } else {
                        step.delay_ms
                    };

                    let tool_call = ToolCall {
                        tool_name: tool_name.clone(),
                        arguments: step.arguments.clone().unwrap_or(serde_json::json!({})),
                        continue_on_error: step.continue_on_error,
                        delay_ms,
                        id: step.id.clone(),
                    };
                    SequenceItem::Tool { tool_call }
                } else if let Some(group_name) = &step.group_name {
                    let tool_group = ToolGroup {
                        group_name: group_name.clone(),
                        steps: step
                            .steps
                            .clone()
                            .unwrap_or_default()
                            .into_iter()
                            .map(|s| ToolCall {
                                tool_name: s.tool_name,
                                arguments: s.arguments,
                                continue_on_error: s.continue_on_error,
                                delay_ms: s.delay_ms,
                                id: s.id,
                            })
                            .collect(),
                        skippable: step.skippable,
                    };
                    SequenceItem::Group { tool_group }
                } else {
                    return Err(McpError::invalid_params(
                        "Each troubleshooting step must have either tool_name (for single tools) or group_name (for groups)",
                        Some(json!({"invalid_step": step})),
                    ));
                };
                sequence_items.push(item);
            }
        }

        // ---------------------------
        // Fallback-enabled execution loop (while-based)
        // ---------------------------

        let mut results = Vec::new();
        let mut sequence_had_errors = false;
        let mut critical_error_occurred = false;
        let start_time = chrono::Utc::now();

        let mut current_index: usize = start_from_index;
        let max_iterations = sequence_items.len() * 10; // Prevent infinite fallback loops
        let mut iterations = 0usize;

        // Track main steps boundary and whether we've jumped to troubleshooting
        let main_steps_len = args.steps.as_ref().map(|s| s.len()).unwrap_or(0);
        let mut jumped_to_troubleshooting = false;

        // Get follow_fallback setting (default to false when end_at_step is specified)
        let follow_fallback = args.follow_fallback.unwrap_or(false);
        if args.end_at_step.is_some() {
            info!("follow_fallback={} for bounded execution", follow_fallback);
        }

        // Log if we're skipping steps
        if start_from_index > 0 {
            info!(
                "Skipping first {} steps, starting from index {}",
                start_from_index, start_from_index
            );
        }

        // Log if we're stopping at a specific step
        if end_at_index < sequence_items.len() - 1 {
            info!("Will stop after step at index {} (inclusive)", end_at_index);
        }

        // Build a map from step ID to its index for quick fallback lookup
        use std::collections::HashMap;
        let mut id_to_index: HashMap<String, usize> = HashMap::new();
        if let Some(steps) = &args.steps {
            for (idx, step) in steps.iter().enumerate() {
                if let Some(id) = &step.id {
                    if id_to_index.insert(id.clone(), idx).is_some() {
                        warn!(
                            "Duplicate step id '{}' found; later occurrence overrides earlier.",
                            id
                        );
                    }
                }
            }
        }

        // Add troubleshooting steps to the id map (they come after main steps)
        if let Some(troubleshooting) = &args.troubleshooting {
            // Use the main_steps_len we already defined above
            for (idx, step) in troubleshooting.iter().enumerate() {
                if let Some(id) = &step.id {
                    let global_idx = main_steps_len + idx;
                    if id_to_index.insert(id.clone(), global_idx).is_some() {
                        warn!(
                            "Duplicate step id '{}' found in troubleshooting; later occurrence overrides earlier.",
                            id
                        );
                    }
                }
            }
        }

        while current_index < sequence_items.len()
            && (current_index <= end_at_index || (follow_fallback && jumped_to_troubleshooting))
            && iterations < max_iterations
        {
            iterations += 1;

            // Check if the request has been cancelled
            if request_context.ct.is_cancelled() {
                warn!("Request cancelled by user, stopping sequence execution");
                return Err(McpError::internal_error(
                    "Request cancelled by user",
                    Some(json!({"code": -32001, "reason": "user_cancelled"})),
                ));
            }

            // Get the original step from either main steps or troubleshooting steps
            let original_step = if current_index < main_steps_len {
                args.steps.as_ref().and_then(|s| s.get(current_index))
            } else {
                args.troubleshooting
                    .as_ref()
                    .and_then(|t| t.get(current_index - main_steps_len))
            };
            if let Some(step) = original_step {
                if let Some(tool_name) = &step.tool_name {
                    info!(
                        "Step {} BEGIN tool='{}' id='{}' retries={} if_expr={:?} fallback_id={:?}",
                        current_index,
                        tool_name,
                        step.id.as_deref().unwrap_or(""),
                        step.retries.unwrap_or(0),
                        step.r#if,
                        step.fallback_id
                    );
                } else if let Some(group_name) = &step.group_name {
                    info!(
                        "Step {} BEGIN group='{}' id='{}' steps={}",
                        current_index,
                        group_name,
                        step.id.as_deref().unwrap_or(""),
                        step.steps.as_ref().map(|v| v.len()).unwrap_or(0)
                    );
                }
            }

            // Extract values from the step if it exists
            let (if_expr, retries, fallback_id_opt) = if let Some(step) = original_step {
                (
                    step.r#if.clone(),
                    step.retries.unwrap_or(0),
                    step.fallback_id.clone(),
                )
            } else {
                (None, 0, None)
            };

            let is_always_step = if_expr.as_deref().is_some_and(|s| s.trim() == "always()");

            // If a critical error occurred and this step is NOT an 'always' step, skip it.
            if critical_error_occurred && !is_always_step {
                results.push(json!({
                    "index": current_index,
                    "status": "skipped",
                    "reason": "Skipped due to a previous unrecoverable error in the sequence."
                }));
                current_index += 1;
                continue;
            }

            // 1. Evaluate condition, unless it's an 'always' step.
            if let Some(cond_str) = &if_expr {
                let execution_context = serde_json::Value::Object(execution_context_map.clone());
                if !is_always_step
                    && !crate::expression_eval::evaluate(cond_str, &execution_context)
                {
                    info!(
                        "Skipping step {} due to if expression not met: `{}`",
                        current_index, cond_str
                    );
                    results.push(json!({
                        "index": current_index,
                        "status": "skipped",
                        "reason": format!("if_expr not met: {}", cond_str)
                    }));
                    current_index += 1;
                    continue;
                }
            }

            // 2. Execute with retries
            let mut final_result = json!(null);
            let mut step_error_occurred = false;
            let total_steps = sequence_items.len();

            for attempt in 0..=retries {
                let item = &mut sequence_items[current_index];
                match item {
                    SequenceItem::Tool { tool_call } => {
                        // Special internal pseudo-tool to set env for subsequent steps
                        let tool_name_normalized = tool_call
                            .tool_name
                            .strip_prefix("mcp_terminator-mcp-agent_")
                            .unwrap_or(&tool_call.tool_name)
                            .to_string();

                        // Substitute variables in arguments before execution
                        let execution_context =
                            serde_json::Value::Object(execution_context_map.clone());
                        let mut substituted_args = tool_call.arguments.clone();
                        substitute_variables(&mut substituted_args, &execution_context);

                        // Inject workflow variables and accumulated env for run_command and execute_browser_script
                        if matches!(
                            tool_call.tool_name.as_str(),
                            "run_command" | "execute_browser_script"
                        ) {
                            // Get env object or create empty one
                            let mut env_obj = substituted_args
                                .get("env")
                                .and_then(|v| v.as_object())
                                .cloned()
                                .unwrap_or_else(serde_json::Map::new);

                            // Only inject state if explicitly in verbose/debug mode
                            if args.include_detailed_results.unwrap_or(false) {
                                // Add workflow variables as special env key
                                // Extract actual values from VariableDefinition defaults
                                if let Some(workflow_vars) = &args.variables {
                                    let mut variable_values = serde_json::Map::new();
                                    for (key, var_def) in workflow_vars {
                                        if let Some(default_value) = &var_def.default {
                                            variable_values
                                                .insert(key.clone(), default_value.clone());
                                        }
                                    }
                                    env_obj.insert(
                                        "_workflow_variables".to_string(),
                                        json!(variable_values),
                                    );
                                }

                                // Add accumulated env from execution context as special key
                                if let Some(accumulated_env) = execution_context.get("env") {
                                    env_obj.insert(
                                        "_accumulated_env".to_string(),
                                        accumulated_env.clone(),
                                    );
                                }
                            }

                            // Update the arguments
                            if let Some(args_obj) = substituted_args.as_object_mut() {
                                args_obj.insert("env".to_string(), json!(env_obj));
                            }
                        }

                        // Start step telemetry span
                        let step_id = original_step.and_then(|s| s.id.as_deref());
                        let mut step_span = StepSpan::new(&tool_call.tool_name, step_id);
                        step_span.set_attribute("step.number", (current_index + 1).to_string());
                        step_span.set_attribute("step.total", total_steps.to_string());
                        if attempt > 0 {
                            step_span.set_attribute("step.retry_attempt", attempt.to_string());
                        }

                        // Add event for step started
                        workflow_span.add_event(
                            "step.started",
                            vec![
                                ("step.tool", tool_call.tool_name.clone()),
                                ("step.index", current_index.to_string()),
                            ],
                        );

                        let (result, error_occurred) = self
                            .execute_single_tool(
                                peer.clone(),
                                request_context.clone(),
                                &tool_call.tool_name,
                                &substituted_args,
                                tool_call.continue_on_error.unwrap_or(false),
                                current_index,
                                include_detailed,
                                original_step.and_then(|s| s.id.as_deref()),
                            )
                            .await;

                        final_result = result.clone();

                        // NEW: Store tool result in env if step has an ID (for ALL tools, not just scripts)
                        if let Some(step_id) = original_step.and_then(|s| s.id.as_deref()) {
                            if let Some(env_value) = execution_context_map.get_mut("env") {
                                if let Some(env_map) = env_value.as_object_mut() {
                                    // Store the result with {step_id}_result pattern
                                    let result_key = format!("{step_id}_result");
                                    let status_key = format!("{step_id}_status");

                                    // Extract the meaningful content from the result
                                    let mut result_content =
                                        if let Some(result_obj) = final_result.get("result") {
                                            // For tools, extract the actual content
                                            if let Some(content) = result_obj.get("content") {
                                                content.clone()
                                            } else {
                                                result_obj.clone()
                                            }
                                        } else {
                                            // Fallback to the entire result if no nested structure
                                            final_result.clone()
                                        };

                                    // REMOVE server_logs before storing in env (they're debug data, not operational data)
                                    if let Some(obj) = result_content.as_object_mut() {
                                        if obj.contains_key("server_logs") {
                                            let log_count = obj
                                                .get("server_logs")
                                                .and_then(|logs| logs.as_array())
                                                .map(|arr| arr.len())
                                                .unwrap_or(0);
                                            obj.remove("server_logs");
                                            debug!(
                                                "Removed {} server_logs from {}_result before storing in env",
                                                log_count, step_id
                                            );
                                        }
                                    }

                                    // Store both result and status
                                    env_map.insert(result_key.clone(), result_content);
                                    env_map
                                        .insert(status_key.clone(), final_result["status"].clone());

                                    info!(
                                        "Stored tool result for step '{}' in env as '{}'",
                                        step_id, result_key
                                    );

                                    // Save state after storing tool result
                                    if let Some(url) = &args.url {
                                        Self::save_workflow_state(
                                            url,
                                            Some(step_id),
                                            current_index,
                                            env_value,
                                        )
                                        .await
                                        .ok(); // Don't fail the workflow if state save fails
                                    }
                                }
                            }
                        }

                        // Update step span status and end it
                        // Support both 'status' field and 'success' field
                        let success = result["status"] == "success"
                            || result["success"] == true
                            || (result["status"].is_null() && result["success"] != false);
                        step_span.set_status(
                            success,
                            if !success {
                                result["error"].as_str()
                            } else {
                                None
                            },
                        );
                        step_span.end();

                        // Add workflow event for step completion
                        workflow_span.add_event(
                            "step.completed",
                            vec![
                                ("step.tool", tool_call.tool_name.clone()),
                                ("step.index", current_index.to_string()),
                                (
                                    "step.status",
                                    result["status"].as_str().unwrap_or("unknown").to_string(),
                                ),
                            ],
                        );

                        // Define reserved keys that shouldn't auto-merge
                        const RESERVED_KEYS: &[&str] =
                            &["status", "error", "logs", "duration_ms", "set_env"];

                        // Merge env updates from engine/script-based steps into the internal context
                        if (tool_name_normalized == "execute_browser_script"
                            || tool_name_normalized == "run_command")
                            && final_result["status"] == "success"
                        {
                            // Helper to merge updates into the env context map
                            let mut merge_env_obj = |update_val: &serde_json::Value| {
                                if let Some(update_map) = update_val.as_object() {
                                    if let Some(env_value) = execution_context_map.get_mut("env") {
                                        if let Some(env_map) = env_value.as_object_mut() {
                                            for (k, v) in update_map.iter() {
                                                env_map.insert(k.clone(), v.clone());
                                            }
                                        }
                                    }
                                }
                            };

                            // Special handling for execute_browser_script
                            if tool_name_normalized == "execute_browser_script" {
                                // Browser scripts return their result as a plain string in final_result["result"]["content"][0]["result"]
                                if let Some(result_str) = final_result
                                    .get("result")
                                    .and_then(|r| r.get("content"))
                                    .and_then(|c| c.as_array())
                                    .and_then(|arr| arr.first())
                                    .and_then(|item| item.get("result"))
                                    .and_then(|r| r.as_str())
                                {
                                    info!(
                                        "[execute_browser_script] Browser script returned: {}",
                                        result_str
                                    );
                                    // Try to parse the browser script result as JSON
                                    match serde_json::from_str::<serde_json::Value>(result_str) {
                                        Ok(parsed_json) => {
                                            info!("[execute_browser_script] Successfully parsed browser result as JSON");

                                            // First handle explicit set_env for backward compatibility
                                            if let Some(set_env) = parsed_json.get("set_env") {
                                                info!("[execute_browser_script] Found set_env in browser script result, merging into context");
                                                merge_env_obj(set_env);
                                            }

                                            // Then auto-merge non-reserved fields
                                            if let Some(obj) = parsed_json.as_object() {
                                                if let Some(env_value) =
                                                    execution_context_map.get_mut("env")
                                                {
                                                    if let Some(env_map) = env_value.as_object_mut()
                                                    {
                                                        for (k, v) in obj {
                                                            if !RESERVED_KEYS.contains(&k.as_str())
                                                            {
                                                                env_map
                                                                    .insert(k.clone(), v.clone());
                                                                info!("[execute_browser_script] Auto-merged field '{}' to env", k);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            info!("[execute_browser_script] Browser result is not JSON: {}", e);
                                        }
                                    }
                                } else {
                                    info!("[execute_browser_script] Could not extract browser script result string from response structure");
                                }
                            } else if tool_name_normalized == "run_command" {
                                // Original logic for run_command
                                if let Some(content_arr) = final_result
                                    .get("result")
                                    .and_then(|r| r.get("content"))
                                    .and_then(|c| c.as_array())
                                {
                                    for item in content_arr {
                                        // Typical engine payload is under item.result
                                        if let Some(res) = item.get("result") {
                                            // First handle explicit set_env/env for backward compatibility
                                            if let Some(v) =
                                                res.get("set_env").or_else(|| res.get("env"))
                                            {
                                                merge_env_obj(v);
                                            }
                                        }
                                        // Also support top-level set_env/env directly on the item
                                        if let Some(v) =
                                            item.get("set_env").or_else(|| item.get("env"))
                                        {
                                            merge_env_obj(v);
                                        }
                                    }

                                    // Auto-merge non-reserved fields from run_command results
                                    for item in content_arr {
                                        if let Some(res) = item.get("result") {
                                            if let Some(obj) = res.as_object() {
                                                if let Some(env_value) =
                                                    execution_context_map.get_mut("env")
                                                {
                                                    if let Some(env_map) = env_value.as_object_mut()
                                                    {
                                                        for (k, v) in obj {
                                                            if !RESERVED_KEYS.contains(&k.as_str())
                                                            {
                                                                env_map
                                                                    .insert(k.clone(), v.clone());
                                                                info!("[run_command] Auto-merged field '{}' to env", k);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // NEW: Save state after env update
                            if let Some(url) = &args.url {
                                if let Some(env_value) = execution_context_map.get("env") {
                                    Self::save_workflow_state(
                                        url,
                                        original_step.and_then(|s| s.id.as_deref()),
                                        current_index,
                                        env_value,
                                    )
                                    .await
                                    .ok(); // Don't fail the workflow if state save fails
                                }
                            }
                        }
                        // Check for success using both 'status' and 'success' fields
                        if result["status"] == "success"
                            || result["success"] == true
                            || (result["status"].is_null() && result["success"] != false)
                        {
                            // Apply delay after successful execution
                            if let Some(delay_ms) = tool_call.delay_ms {
                                if delay_ms > 0 {
                                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                                }
                            }
                            break;
                        }

                        if error_occurred {
                            // Only mark as critical if there's no fallback to handle it
                            if fallback_id_opt.is_none() {
                                critical_error_occurred = true;
                                if let Some(id) = original_step.and_then(|s| s.id.as_deref()) {
                                    tracing::warn!(
                                        step_id = %id,
                                        tool = %tool_call.tool_name,
                                        attempt = attempt + 1,
                                        skippable = %tool_call.continue_on_error.unwrap_or(false),
                                        has_fallback = false,
                                        "Tool failed with unrecoverable error (no fallback)"
                                    );
                                } else {
                                    tracing::warn!(
                                        tool = %tool_call.tool_name,
                                        attempt = attempt + 1,
                                        skippable = %tool_call.continue_on_error.unwrap_or(false),
                                        has_fallback = false,
                                        "Tool failed with unrecoverable error (no fallback)"
                                    );
                                }
                            } else {
                                // Has fallback, log but don't mark as critical
                                if let Some(id) = original_step.and_then(|s| s.id.as_deref()) {
                                    tracing::info!(
                                        step_id = %id,
                                        tool = %tool_call.tool_name,
                                        fallback_id = %fallback_id_opt.as_ref().unwrap(),
                                        "Tool failed but has fallback configured"
                                    );
                                } else {
                                    tracing::info!(
                                        tool = %tool_call.tool_name,
                                        fallback_id = %fallback_id_opt.as_ref().unwrap(),
                                        "Tool failed but has fallback configured"
                                    );
                                }
                            }
                        }
                        step_error_occurred = true;
                        sequence_had_errors = true;

                        if let Some(delay_ms) = tool_call.delay_ms {
                            if delay_ms > 0 {
                                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                            }
                        }
                    }
                    SequenceItem::Group { tool_group } => {
                        let mut group_had_errors = false;
                        let mut group_results = Vec::new();
                        let is_skippable = tool_group.skippable.unwrap_or(false);

                        for (step_index, step_tool_call) in tool_group.steps.iter_mut().enumerate()
                        {
                            // Substitute variables in arguments before execution
                            let execution_context =
                                serde_json::Value::Object(execution_context_map.clone());
                            let mut substituted_args = step_tool_call.arguments.clone();
                            substitute_variables(&mut substituted_args, &execution_context);

                            let (result, error_occurred) = self
                                .execute_single_tool(
                                    peer.clone(),
                                    request_context.clone(),
                                    &step_tool_call.tool_name,
                                    &substituted_args,
                                    step_tool_call.continue_on_error.unwrap_or(false),
                                    step_index,
                                    include_detailed,
                                    step_tool_call.id.as_deref(), // Use step ID if available
                                )
                                .await;

                            group_results.push(result.clone());

                            if let Some(delay_ms) = step_tool_call.delay_ms {
                                if delay_ms > 0 {
                                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                                }
                            }

                            // Check for failure using both 'status' and 'success' fields
                            let tool_failed = !(result["status"] == "success"
                                || result["success"] == true
                                || (result["status"].is_null() && result["success"] != false));
                            if tool_failed {
                                group_had_errors = true;
                                if error_occurred || is_skippable {
                                    if error_occurred && !is_skippable {
                                        // Only mark as critical if there's no fallback to handle it
                                        if fallback_id_opt.is_none() {
                                            critical_error_occurred = true;
                                        }
                                    }
                                    tracing::warn!(
                                        group = %tool_group.group_name,
                                        tool = %step_tool_call.tool_name,
                                        step_index = step_index,
                                        step_id = %step_tool_call.id.clone().unwrap_or_default(),
                                        skippable = %is_skippable,
                                        has_fallback = fallback_id_opt.is_some(),
                                        "Group step failed; breaking out of group"
                                    );
                                    break;
                                }
                            }
                        }

                        let group_status = if group_had_errors {
                            "partial_success"
                        } else {
                            "success"
                        };

                        if group_status != "success" {
                            sequence_had_errors = true;
                            step_error_occurred = true;
                        }

                        if group_had_errors && !is_skippable && stop_on_error {
                            // Only mark as critical if there's no fallback to handle it
                            if fallback_id_opt.is_none() {
                                critical_error_occurred = true;
                            }
                        }

                        final_result = json!({
                            "group_name": &tool_group.group_name,
                            "status": group_status,
                            "results": group_results
                        });

                        if !group_had_errors {
                            break; // Group succeeded, break retry loop.
                        }
                    }
                }
                if attempt < retries {
                    warn!(
                        "Step {} failed on attempt {}/{}. Retrying...",
                        current_index,
                        attempt + 1,
                        retries
                    );
                    tokio::time::sleep(Duration::from_millis(500)).await; // Wait before retry
                }
            }

            results.push(final_result);

            // Decide next index based on success or fallback
            let step_succeeded = !step_error_occurred;
            let step_status_str = if step_succeeded { "success" } else { "failed" };
            if let Some(tool_name) = original_step.and_then(|s| s.tool_name.as_ref()) {
                info!(
                    "Step {} END tool='{}' id='{}' status={}",
                    current_index,
                    tool_name,
                    original_step.and_then(|s| s.id.as_deref()).unwrap_or(""),
                    step_status_str
                );
            } else if let Some(group_name) = original_step.and_then(|s| s.group_name.as_ref()) {
                info!(
                    "Step {} END group='{}' id='{}' status={}",
                    current_index,
                    group_name,
                    original_step.and_then(|s| s.id.as_deref()).unwrap_or(""),
                    step_status_str
                );
            }

            if step_succeeded {
                // For successful steps, check if we're about to enter troubleshooting section
                if !jumped_to_troubleshooting && current_index >= main_steps_len - 1 {
                    // We're at or past the last main step and haven't jumped to troubleshooting
                    // Exit the loop to prevent entering troubleshooting during normal flow
                    info!("Completed all main workflow steps successfully");
                    break;
                }
                current_index += 1;
            } else if let Some(fb_id) = fallback_id_opt {
                if let Some(&fb_idx) = id_to_index.get(&fb_id) {
                    // Check if we should follow this fallback based on end_at_step and follow_fallback setting
                    let should_follow_fallback = if args.end_at_step.is_some()
                        && current_index >= end_at_index
                    {
                        // We're at or past end_at_step boundary
                        if follow_fallback {
                            info!(
                                "Step {} failed at end_at_step boundary. Following fallback to '{}' (follow_fallback=true).",
                                current_index, fb_id
                            );
                            true
                        } else {
                            info!(
                                "Step {} failed at end_at_step boundary. NOT following fallback '{}' (follow_fallback=false).",
                                current_index, fb_id
                            );
                            false
                        }
                    } else {
                        // Normal execution, always follow fallback
                        true
                    };

                    if should_follow_fallback {
                        info!(
                            "Step {} failed. Jumping to fallback step with id '{}' (index {}).",
                            current_index, fb_id, fb_idx
                        );

                        // Check if we're jumping into the troubleshooting section
                        if fb_idx >= main_steps_len {
                            jumped_to_troubleshooting = true;
                            info!("Entered troubleshooting section via fallback");
                        }

                        current_index = fb_idx;
                    } else {
                        // Don't follow fallback, treat as normal failure
                        // Break the loop since we're at end_at_step and not following fallback
                        info!(
                            "Stopping execution at end_at_step boundary without following fallback"
                        );
                        break;
                    }
                } else {
                    warn!(
                        "fallback_id '{}' for step {} not found. Continuing to next step.",
                        fb_id, current_index
                    );
                    current_index += 1;
                }
            } else {
                // Step failed with no fallback
                current_index += 1;
            }
        }

        if iterations >= max_iterations {
            warn!("Maximum iteration count reached. Possible infinite fallback loop detected.");
        }

        let total_duration = (chrono::Utc::now() - start_time).num_milliseconds();

        let final_status = if !sequence_had_errors {
            "success"
        } else if critical_error_occurred {
            "partial_success"
        } else {
            "completed_with_errors"
        };
        info!(
            "execute_sequence completed: status={}, executed_tools={}, total_duration_ms={}",
            final_status,
            results.len(),
            total_duration
        );

        let mut summary = json!({
            "action": "execute_sequence",
            "status": final_status,
            "total_tools": sequence_items.len(),
            "executed_tools": results.len(),
            "total_duration_ms": total_duration,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "results": results,
        });

        // Support both 'output_parser' (legacy) and 'output' (simplified)
        let parser_def = args.output_parser.as_ref().or(args.output.as_ref());

        if let Some(parser_def) = parser_def {
            // Apply variable substitution to the output_parser field
            let mut parser_json = parser_def.clone();
            let execution_context = serde_json::Value::Object(execution_context_map.clone());
            substitute_variables(&mut parser_json, &execution_context);

            match output_parser::run_output_parser(&parser_json, &summary).await {
                Ok(Some(parsed_data)) => {
                    if let Some(obj) = summary.as_object_mut() {
                        obj.insert("parsed_output".to_string(), parsed_data);
                    }
                }
                Ok(None) => {
                    if let Some(obj) = summary.as_object_mut() {
                        obj.insert("parsed_output".to_string(), json!({}));
                    }
                }
                Err(e) => {
                    if let Some(obj) = summary.as_object_mut() {
                        obj.insert("parser_error".to_string(), json!(e.to_string()));
                    }
                }
            }
        }
        if final_status != "success" {
            // Capture minimal structured debug info so failures are not opaque
            let debug_info = json!({
                "final_status": final_status,
                "had_critical_error": critical_error_occurred,
                "had_errors": sequence_had_errors,
                "executed_count": results.len(),
            });

            if let Some(obj) = summary.as_object_mut() {
                obj.insert("debug_info_on_failure".to_string(), debug_info);
            }
        }

        let contents = vec![Content::json(summary.clone())?];

        // End workflow span with success status
        let had_errors = summary
            .get("had_errors")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        workflow_span.set_status(
            !had_errors,
            if had_errors {
                "Workflow completed with errors"
            } else {
                "Workflow completed successfully"
            },
        );
        workflow_span.add_event(
            "workflow.completed",
            vec![
                ("workflow.total_steps", results.len().to_string()),
                ("workflow.had_errors", had_errors.to_string()),
            ],
        );
        workflow_span.end();

        Ok(CallToolResult::success(contents))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn execute_single_tool(
        &self,
        peer: Peer<RoleServer>,
        request_context: RequestContext<RoleServer>,
        tool_name: &str,
        arguments: &Value,
        is_skippable: bool,
        index: usize,
        include_detailed: bool,
        step_id: Option<&str>,
    ) -> (serde_json::Value, bool) {
        let tool_start_time = chrono::Utc::now();
        let tool_name_short = tool_name
            .strip_prefix("mcp_terminator-mcp-agent_")
            .unwrap_or(tool_name);

        // Start log capture if in verbose mode
        if include_detailed {
            if let Some(ref log_capture) = self.log_capture {
                log_capture.start_capture();
            }
        }

        // The substitution is handled in `execute_sequence_impl`.
        let tool_result = self
            .dispatch_tool(peer, request_context, tool_name_short, arguments)
            .await;

        let (processed_result, error_occurred) = match tool_result {
            Ok(result) => {
                let mut extracted_content = Vec::new();

                if !result.content.is_empty() {
                    for content in &result.content {
                        match extract_content_json(content) {
                            Ok(json_content) => extracted_content.push(json_content),
                            Err(_) => extracted_content.push(
                                json!({ "type": "unknown", "data": "Content extraction failed" }),
                            ),
                        }
                    }
                }

                let content_count = result.content.len();
                let content_summary = if include_detailed {
                    // Verbose mode: include full content/step definitions
                    json!({ "type": "tool_result", "content_count": content_count, "content": extracted_content })
                } else {
                    // Normal/quiet mode: include extracted content (logs/output) but not step definitions
                    // The extracted_content already contains just the results, not the tool arguments/definitions
                    json!({
                        "type": "tool_result",
                        "status": "success",
                        "content_count": content_count,
                        "content": extracted_content
                    })
                };
                let duration_ms = (chrono::Utc::now() - tool_start_time).num_milliseconds();
                let mut result_json = json!({
                    "tool_name": tool_name,
                    "index": index,
                    "status": "success",
                    "duration_ms": duration_ms,
                    "result": content_summary,
                });

                // Add step_id if provided
                if let Some(id) = step_id {
                    if let Some(obj) = result_json.as_object_mut() {
                        obj.insert("step_id".to_string(), json!(id));
                    }
                }

                // Capture server logs if in verbose mode
                if include_detailed {
                    if let Some(ref log_capture) = self.log_capture {
                        let captured_logs = log_capture.stop_capture();
                        if !captured_logs.is_empty() {
                            if let Some(obj) = result_json.as_object_mut() {
                                obj.insert("server_logs".to_string(), json!(captured_logs));
                            }
                        }
                    }
                }

                // Extract and add logs if present (for run_command)
                if tool_name_short == "run_command" {
                    // Debug: log what's in extracted content
                    for (i, content) in extracted_content.iter().enumerate() {
                        if let Some(logs) = content.get("logs") {
                            info!(
                                "[execute_single_tool] Found logs in content[{}]: {} entries",
                                i,
                                logs.as_array().map(|a| a.len()).unwrap_or(0)
                            );
                        }
                    }

                    // Look for logs in the extracted content
                    if let Some(logs) = extracted_content
                        .iter()
                        .find_map(|c| c.get("logs").cloned())
                    {
                        info!("[execute_single_tool] Adding logs to result_json");
                        if let Some(obj) = result_json.as_object_mut() {
                            obj.insert("logs".to_string(), logs);
                        }
                    } else {
                        info!("[execute_single_tool] No logs found in extracted content");
                    }
                }

                let result_json =
                    serde_json::Value::Object(result_json.as_object().unwrap().clone());
                (result_json, false)
            }
            Err(e) => {
                // Stop log capture on error and collect logs
                let captured_logs = if include_detailed {
                    self.log_capture
                        .as_ref()
                        .map(|log_capture| log_capture.stop_capture())
                } else {
                    None
                };

                let duration_ms = (chrono::Utc::now() - tool_start_time).num_milliseconds();
                let mut error_result = json!({
                    "tool_name": tool_name,
                    "index": index,
                    "status": if is_skippable { "skipped" } else { "error" },
                    "duration_ms": duration_ms,
                    "error": format!("{}", e),
                });

                // Include server logs in error result if captured
                if let Some(logs) = captured_logs {
                    if !logs.is_empty() {
                        if let Some(obj) = error_result.as_object_mut() {
                            obj.insert("server_logs".to_string(), json!(logs));
                        }
                    }
                }

                // Add step_id if provided
                if let Some(id) = step_id {
                    if let Some(obj) = error_result.as_object_mut() {
                        obj.insert("step_id".to_string(), json!(id));
                    }
                }

                let error_result =
                    serde_json::Value::Object(error_result.as_object().unwrap().clone());

                if !is_skippable {
                    warn!(
                        "Tool '{}' at index {} failed. Reason: {}",
                        tool_name, index, e
                    );
                }
                (error_result, !is_skippable)
            }
        };

        (processed_result, error_occurred)
    }
}
