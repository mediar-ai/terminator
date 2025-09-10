use crate::helpers::substitute_variables;
use crate::output_parser;
use crate::server::extract_content_json;
use crate::utils::{DesktopWrapper, ExecuteSequenceArgs, SequenceItem, ToolCall, ToolGroup};
use rmcp::model::{CallToolResult, Content};
use rmcp::service::{Peer, RequestContext, RoleServer};
use rmcp::ErrorData as McpError;
use serde_json::{json, Value};
use std::time::Duration;
use tracing::{info, warn};
use uuid::Uuid;

impl DesktopWrapper {
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
            info!(
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

            // Debug: Log the loaded steps
            if let Some(steps) = &args.steps {
                for (i, step) in steps.iter().enumerate() {
                    info!(
                        "Loaded step {}: tool_name={:?}, group_name={:?}",
                        i, step.tool_name, step.group_name
                    );
                }
            }
        }

        let stop_on_error = args.stop_on_error.unwrap_or(true);
        let include_detailed = args.include_detailed_results.unwrap_or(true);

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

        let execution_context = serde_json::Value::Object(execution_context_map.clone());
        info!(
            "Executing sequence with context: {}",
            serde_json::to_string_pretty(&execution_context).unwrap_or_default()
        );
        info!(
            "Starting execute_sequence: steps={}, stop_on_error={}, include_detailed_results={}",
            args.steps.as_ref().map(|s| s.len()).unwrap_or(0),
            stop_on_error,
            include_detailed
        );

        // Root tracing span for sequence execution
        let seq_span = tracing::info_span!(
            "mcp_sequence",
            steps = args.steps.as_ref().map(|s| s.len()).unwrap_or(0) as i64,
            stop_on_error = stop_on_error,
            include_detailed = include_detailed,
        );
        let _seq_guard = seq_span.enter();
        // Publish start event via HTTP event bus
        let request_id = Uuid::new_v4().to_string();
        terminator_mcp_agent::event_bus::publish(json!({
            "type": "sequence",
            "phase": "start",
            "request_id": request_id,
            "steps": args.steps.as_ref().map(|s| s.len()).unwrap_or(0),
            "stop_on_error": stop_on_error,
            "include_detailed_results": include_detailed,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }));

        terminator_mcp_agent::event_bus::publish(json!({
            "type": "sequence_progress",
            "request_id": request_id,
            "progress": 0,
            "total": args.steps.as_ref().map(|s| s.len()).unwrap_or(0),
            "message": "Starting execute_sequence",
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }));

        // Convert flattened SequenceStep to internal SequenceItem representation
        let mut sequence_items = Vec::new();
        let empty_steps = Vec::new();
        let steps = args.steps.as_ref().unwrap_or(&empty_steps);
        for step in steps {
            let item = if let Some(tool_name) = &step.tool_name {
                let tool_call = ToolCall {
                    tool_name: tool_name.clone(),
                    arguments: step.arguments.clone().unwrap_or(serde_json::json!({})),
                    continue_on_error: step.continue_on_error,
                    delay_ms: step.delay_ms,
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

        // ---------------------------
        // Fallback-enabled execution loop (while-based)
        // ---------------------------

        let mut results = Vec::new();
        let mut sequence_had_errors = false;
        let mut critical_error_occurred = false;
        let start_time = chrono::Utc::now();

        let mut current_index: usize = 0;
        let max_iterations = sequence_items.len() * 10; // Prevent infinite fallback loops
        let mut iterations = 0usize;

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

        while current_index < sequence_items.len() && iterations < max_iterations {
            iterations += 1;

            let original_step = args.steps.as_ref().and_then(|s| s.get(current_index));
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
                    terminator_mcp_agent::event_bus::publish(json!({
                        "type": "sequence_step",
                        "phase": "begin",
                        "request_id": request_id,
                        "index": current_index,
                        "step_type": "tool",
                        "tool": tool_name,
                        "id": step.id,
                        "retries": step.retries.unwrap_or(0),
                        "if": step.r#if,
                        "fallback_id": step.fallback_id,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    }));
                } else if let Some(group_name) = &step.group_name {
                    info!(
                        "Step {} BEGIN group='{}' id='{}' steps={}",
                        current_index,
                        group_name,
                        step.id.as_deref().unwrap_or(""),
                        step.steps.as_ref().map(|v| v.len()).unwrap_or(0)
                    );
                    terminator_mcp_agent::event_bus::publish(json!({
                        "type": "sequence_step",
                        "phase": "begin",
                        "request_id": request_id,
                        "index": current_index,
                        "step_type": "group",
                        "group": group_name,
                        "id": step.id,
                        "steps": step.steps.as_ref().map(|v| v.len()).unwrap_or(0),
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    }));
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

                            if let Some(content_arr) = final_result
                                .get("result")
                                .and_then(|r| r.get("content"))
                                .and_then(|c| c.as_array())
                            {
                                for item in content_arr {
                                    // Typical engine payload is under item.result
                                    if let Some(res) = item.get("result") {
                                        if let Some(v) =
                                            res.get("set_env").or_else(|| res.get("env"))
                                        {
                                            merge_env_obj(v);
                                        }
                                    }
                                    // Also support top-level set_env/env directly on the item
                                    if let Some(v) = item.get("set_env").or_else(|| item.get("env"))
                                    {
                                        merge_env_obj(v);
                                    }
                                }
                            }
                        }
                        if result["status"] == "success" {
                            break;
                        }

                        if error_occurred {
                            critical_error_occurred = true;
                            if let Some(id) = original_step.and_then(|s| s.id.as_deref()) {
                                tracing::warn!(
                                    step_id = %id,
                                    tool = %tool_call.tool_name,
                                    attempt = attempt + 1,
                                    skippable = %tool_call.continue_on_error.unwrap_or(false),
                                    "Tool failed with unrecoverable error"
                                );
                            } else {
                                tracing::warn!(
                                    tool = %tool_call.tool_name,
                                    attempt = attempt + 1,
                                    skippable = %tool_call.continue_on_error.unwrap_or(false),
                                    "Tool failed with unrecoverable error"
                                );
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

                            let tool_failed = result["status"] != "success";
                            if tool_failed {
                                group_had_errors = true;
                                if error_occurred || is_skippable {
                                    if error_occurred && !is_skippable {
                                        critical_error_occurred = true;
                                    }
                                    tracing::warn!(
                                        group = %tool_group.group_name,
                                        tool = %step_tool_call.tool_name,
                                        step_index = step_index,
                                        step_id = %step_tool_call.id.clone().unwrap_or_default(),
                                        skippable = %is_skippable,
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
                            critical_error_occurred = true;
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
                terminator_mcp_agent::event_bus::publish(json!({
                    "type": "sequence_step",
                    "phase": "end",
                    "request_id": request_id,
                    "index": current_index,
                    "step_type": "tool",
                    "tool": tool_name,
                    "id": original_step.and_then(|s| s.id.clone()),
                    "status": step_status_str,
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                }));
            } else if let Some(group_name) = original_step.and_then(|s| s.group_name.as_ref()) {
                info!(
                    "Step {} END group='{}' id='{}' status={}",
                    current_index,
                    group_name,
                    original_step.and_then(|s| s.id.as_deref()).unwrap_or(""),
                    step_status_str
                );
                terminator_mcp_agent::event_bus::publish(json!({
                    "type": "sequence_step",
                    "phase": "end",
                    "request_id": request_id,
                    "index": current_index,
                    "step_type": "group",
                    "group": group_name,
                    "id": original_step.and_then(|s| s.id.clone()),
                    "status": step_status_str,
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                }));
            }

            if step_succeeded {
                current_index += 1;
            } else if let Some(fb_id) = fallback_id_opt {
                if let Some(&fb_idx) = id_to_index.get(&fb_id) {
                    info!(
                        "Step {} failed. Jumping to fallback step with id '{}' (index {}).",
                        current_index, fb_id, fb_idx
                    );
                    current_index = fb_idx;
                } else {
                    warn!(
                        "fallback_id '{}' for step {} not found. Continuing to next step.",
                        fb_id, current_index
                    );
                    current_index += 1;
                }
            } else {
                current_index += 1;
            }

            terminator_mcp_agent::event_bus::publish(json!({
                "type": "sequence_progress",
                "request_id": request_id,
                "progress": (current_index as u32).saturating_add(1),
                "total": args.steps.as_ref().map(|s| s.len()).unwrap_or(0) as u32,
                "message": format!("Step {current_index} {step_status_str}"),
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }));
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
        terminator_mcp_agent::event_bus::publish(json!({
            "type": "sequence",
            "phase": "end",
            "request_id": request_id,
            "status": final_status,
            "executed_tools": results.len(),
            "duration_ms": total_duration,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }));

        let mut summary = json!({
            "action": "execute_sequence",
            "status": final_status,
            "total_tools": sequence_items.len(),
            "executed_tools": results.len(),
            "total_duration_ms": total_duration,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "results": results,
        });

        if let Some(parser_def) = args.output_parser.as_ref() {
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

        let contents = vec![Content::json(summary)?];

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
        let span = tracing::info_span!(
            "mcp_tool",
            tool = %tool_name,
            index = index as i64,
            step_id = step_id.unwrap_or("")
        );
        let _guard = span.enter();
        let tool_name_short = tool_name
            .strip_prefix("mcp_terminator-mcp-agent_")
            .unwrap_or(tool_name);

        // The substitution is handled in `execute_sequence_impl`.
        let tool_result = self
            .dispatch_tool(peer, request_context, tool_name_short, arguments)
            .await;

        let (processed_result, error_occurred) = match tool_result {
            Ok(result) => {
                let mut extracted_content = Vec::new();

                if let Some(content_vec) = &result.content {
                    for content in content_vec {
                        match extract_content_json(content) {
                            Ok(json_content) => extracted_content.push(json_content),
                            Err(_) => extracted_content.push(
                                json!({ "type": "unknown", "data": "Content extraction failed" }),
                            ),
                        }
                    }
                }

                let content_count = result.content.as_ref().map(|v| v.len()).unwrap_or(0);
                let content_summary = if include_detailed {
                    json!({ "type": "tool_result", "content_count": content_count, "content": extracted_content })
                } else {
                    json!({ "type": "summary", "content": "Tool executed successfully", "content_count": content_count })
                };
                let duration_ms = (chrono::Utc::now() - tool_start_time).num_milliseconds();
                let mut result_json = json!({
                    "tool_name": tool_name,
                    "index": index,
                    "status": "success",
                    "duration_ms": duration_ms,
                    "result": content_summary,
                });

                // Metrics/logging
                crate::telemetry::record_tool_outcome(tool_name, "success", duration_ms);

                // Add step_id if provided
                if let Some(id) = step_id {
                    if let Some(obj) = result_json.as_object_mut() {
                        obj.insert("step_id".to_string(), json!(id));
                    }
                }

                let result_json =
                    serde_json::Value::Object(result_json.as_object().unwrap().clone());
                (result_json, false)
            }
            Err(e) => {
                let duration_ms = (chrono::Utc::now() - tool_start_time).num_milliseconds();
                let mut error_result = json!({
                    "tool_name": tool_name,
                    "index": index,
                    "status": if is_skippable { "skipped" } else { "error" },
                    "duration_ms": duration_ms,
                    "error": format!("{}", e),
                });

                // Metrics/logging
                crate::telemetry::record_tool_outcome(
                    tool_name,
                    if is_skippable { "skipped" } else { "error" },
                    duration_ms,
                );

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
