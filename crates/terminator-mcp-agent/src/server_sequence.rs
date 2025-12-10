



use crate::utils::{
    DesktopWrapper, ExecuteSequenceArgs,
};

use crate::workflow_typescript::{TypeScriptWorkflow, WorkflowEvent};
use rmcp::model::{
    CallToolResult, Content, LoggingLevel, LoggingMessageNotificationParam, NumberOrString,
    ProgressNotificationParam, ProgressToken,
};
use rmcp::service::{Peer, RequestContext, RoleServer};
use rmcp::ErrorData as McpError;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;
use tracing::{debug, info, info_span, Instrument};
use uuid::Uuid;

/// RAII guard to automatically reset the in_sequence flag when dropped
struct SequenceGuard {
    flag: Arc<Mutex<bool>>,
}

impl Drop for SequenceGuard {
    fn drop(&mut self) {
        // Reset flag to false when guard is dropped (function exits)
        if let Ok(mut flag) = self.flag.lock() {
            *flag = false;
        }
    }
}

impl SequenceGuard {
    fn new(flag: Arc<Mutex<bool>>) -> Self {
        // Set flag to true when guard is created
        if let Ok(mut f) = flag.lock() {
            *f = true;
        }
        Self { flag }
    }
}


impl DesktopWrapper {
    // Get the state file path for a workflow
    // Uses OS-standard data directories:
    //   Windows: %LOCALAPPDATA%\mediar\workflows\<workflow_id>\state.json
    //   macOS: ~/Library/Application Support/mediar/workflows/<workflow_id>/state.json
    //   Linux: ~/.local/share/mediar/workflows/<workflow_id>/state.json
    // Priority: workflow_id > URL hash (for backward compatibility)
    async fn get_state_file_path(
        workflow_id: Option<&str>,
        workflow_url: Option<&str>,
    ) -> Option<PathBuf> {
        let data_dir = dirs::data_local_dir()?;

        // Priority 1: Use workflow_id if provided (cleaner, no hashing needed)
        if let Some(id) = workflow_id {
            debug!("Using workflow_id for state file: {}", id);
            let state_file = data_dir
                .join("mediar")
                .join("workflows")
                .join(id)
                .join("state.json");
            return Some(state_file);
        }

        // Priority 2: Fallback to URL hash for backward compatibility
        if let Some(url) = workflow_url {
            if let Some(file_path) = url.strip_prefix("file://") {
                debug!("Using URL hash for state file: {}", url);
                // Create a stable hash of the workflow file path
                let workflow_hash = {
                    use std::collections::hash_map::DefaultHasher;
                    use std::hash::{Hash, Hasher};
                    let mut hasher = DefaultHasher::new();
                    file_path.hash(&mut hasher);
                    format!("{:x}", hasher.finish())
                };

                let state_file = data_dir
                    .join("mediar")
                    .join("workflows")
                    .join(workflow_hash)
                    .join("state.json");

                return Some(state_file);
            }
        }

        None
    }

    // Save env state after any step that modifies it
    async fn save_workflow_state(
        workflow_id: Option<&str>,
        workflow_url: Option<&str>,
        step_id: Option<&str>,
        step_index: usize,
        env: &serde_json::Value,
    ) -> Result<(), McpError> {
        if let Some(state_file) = Self::get_state_file_path(workflow_id, workflow_url).await {
            if let Some(state_dir) = state_file.parent() {
                tokio::fs::create_dir_all(state_dir).await.map_err(|e| {
                    McpError::internal_error(format!("Failed to create state directory: {e}"), None)
                })?;
            }

            let state = json!({
                "last_updated": chrono::Utc::now().to_rfc3339(),
                "last_step_id": step_id,
                "last_step_index": step_index,
                "workflow_id": workflow_id,
                "workflow_file": workflow_url.and_then(|url| {
                    Path::new(url.strip_prefix("file://").unwrap_or(url))
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string())
                }),
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
        workflow_id: Option<&str>,
        workflow_url: Option<&str>,
    ) -> Result<Option<serde_json::Value>, McpError> {
        if let Some(state_file) = Self::get_state_file_path(workflow_id, workflow_url).await {
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

    /// Helper function to create a flattened execution context where env properties
    /// are available both under 'env.' prefix and directly at the top level.
    /// This enables conditions to access env variables directly without the 'env.' prefix,
    /// matching the behavior of script execution.
    fn create_flattened_execution_context(
        execution_context_map: &serde_json::Map<String, serde_json::Value>,
    ) -> serde_json::Value {
        let mut flattened_map = execution_context_map.clone();

        // Flatten env properties to top level
        if let Some(env_value) = flattened_map.get("env") {
            if let Some(env_obj) = env_value.as_object() {
                // Clone env properties to avoid borrow issues
                let env_entries: Vec<(String, serde_json::Value)> = env_obj
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();

                // Insert each env property at top level
                // Note: env properties will override existing top-level keys with same name
                for (key, value) in env_entries {
                    flattened_map.insert(key, value);
                }
            }
        }

        serde_json::Value::Object(flattened_map)
    }

    /// Deep merge JSON values - recursively merges objects, overwrites other types
    /// This matches the Python executor's deep_merge behavior:
    /// - For objects: recursively merge keys from source into target
    /// - For other types: source value overwrites target value
    fn deep_merge_json(target: &mut serde_json::Map<String, Value>, source: &Value) {
        if let Some(source_obj) = source.as_object() {
            for (key, source_value) in source_obj {
                if let Some(target_value) = target.get_mut(key) {
                    // Key exists in target
                    if target_value.is_object() && source_value.is_object() {
                        // Both are objects - recursively merge
                        if let Some(target_obj) = target_value.as_object_mut() {
                            Self::deep_merge_json(target_obj, source_value);
                        }
                    } else {
                        // Not both objects - source overwrites target
                        *target_value = source_value.clone();
                    }
                } else {
                    // Key doesn't exist in target - add it
                    target.insert(key.clone(), source_value.clone());
                }
            }
        }
    }

    pub async fn execute_sequence_impl(
        &self,
        peer: Peer<RoleServer>,
        request_context: RequestContext<RoleServer>,
        args: ExecuteSequenceArgs,
    ) -> Result<CallToolResult, McpError> {
        // Register this execution with the request manager
        // This allows stop_execution to cancel it
        let request_id = format!("execute_sequence_{}", Uuid::new_v4());
        let cancel_context = self
            .request_manager
            .register(
                request_id.clone(),
                Some(600000), // 10 minute timeout for workflows
            )
            .await;

        // Use tokio::select to handle cancellation from request manager
        // Create span with trace_id for distributed tracing - all nested logs inherit it
        let trace_id_val = args.trace_id.clone().unwrap_or_default();
        let execution_id_val = args.execution_id.clone().unwrap_or_default();
        let tracing_span = info_span!(
            "execute_ts_workflow",
            trace_id = %trace_id_val,
            execution_id = %execution_id_val,
            log_source = "agent",
        );

        tokio::select! {
            result = self.execute_sequence_inner(peer, request_context, args, request_id.clone()).instrument(tracing_span) => {
                // Unregister when done
                self.request_manager.unregister(&request_id).await;
                result
            }
            _ = cancel_context.cancellation_token.cancelled() => {
                // Unregister on cancellation
                self.request_manager.unregister(&request_id).await;
                Err(McpError::internal_error(
                    "Workflow execution cancelled by stop_execution",
                    Some(json!({"code": -32001, "request_id": request_id}))
                ))
            }
        }
    }


    async fn execute_sequence_inner(
        &self,
        peer: Peer<RoleServer>,
        _request_context: RequestContext<RoleServer>,
        args: ExecuteSequenceArgs,
        execution_id: String,
    ) -> Result<CallToolResult, McpError> {
        // Set the in_sequence flag for the duration of this function
        let _sequence_guard = SequenceGuard::new(self.in_sequence.clone());

        // TypeScript workflows require a URL
        let url = args.url.clone().ok_or_else(|| {
            McpError::invalid_params(
                "TypeScript workflows require a 'url' parameter pointing to a workflow directory or file".to_string(),
                None,
            )
        })?;

        // Execute TypeScript workflow with MCP notification streaming
        self.execute_typescript_workflow(&url, args, execution_id, peer).await
    }
    async fn execute_typescript_workflow(
        &self,
        url: &str,
        args: ExecuteSequenceArgs,
        execution_id: String,
        peer: Peer<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        // Extract trace context for distributed tracing
        let trace_id_val = args.trace_id.clone().unwrap_or_default();
        let execution_id_val = args
            .execution_id
            .clone()
            .unwrap_or_else(|| execution_id.clone());

        // Create tracing span with execution context
        // All nested logs (including from TypeScript workflow stderr) will inherit this context
        let workflow_span = info_span!(
            "execute_typescript_workflow",
            trace_id = %trace_id_val,
            execution_id = %execution_id_val,
            log_source = "agent",
            url = %url,
        );

        // Log start with execution context for ClickHouse filtering
        info!(
            log_source = "agent",
            execution_id = %execution_id_val,
            trace_id = %trace_id_val,
            url = %url,
            "Starting TypeScript workflow execution [execution_id={}, trace_id={}]",
            execution_id_val, trace_id_val
        );

        // Execute within the span context so all nested logs inherit execution_id/trace_id
        async move {
            // Load saved state if resuming
            let restored_state = if args.start_from_step.is_some() {
                Self::load_workflow_state(args.workflow_id.as_deref(), Some(url)).await?
            } else {
                None
            };

            // Create TypeScript workflow executor
            let ts_workflow = TypeScriptWorkflow::new(url)?;

            // Create event channel for streaming workflow events to MCP client
            let (event_tx, mut event_rx) = mpsc::unbounded_channel::<WorkflowEvent>();

            // Generate a progress token for this workflow execution
            let progress_token = ProgressToken(NumberOrString::String(
                format!("workflow-{}", execution_id_val).into(),
            ));

            // Spawn task to forward events as MCP notifications
            let peer_clone = peer.clone();
            let progress_token_clone = progress_token.clone();
            let notification_handle = tokio::spawn(async move {
                let mut step_counter: u32 = 0;
                let mut total_steps: Option<u32> = None;

                while let Some(event) = event_rx.recv().await {
                    match event {
                        WorkflowEvent::Progress {
                            current,
                            total,
                            message,
                            ..
                        } => {
                            // Send MCP progress notification
                            let _ = peer_clone
                                .notify_progress(ProgressNotificationParam {
                                    progress_token: progress_token_clone.clone(),
                                    progress: current,
                                    total,
                                    message,
                                })
                                .await;
                        }
                        WorkflowEvent::StepStarted {
                            step_name,
                            step_index,
                            total_steps: steps_total,
                            ..
                        } => {
                            step_counter = step_index.unwrap_or(step_counter + 1);
                            if let Some(t) = steps_total {
                                total_steps = Some(t);
                            }
                            // Send as logging message for clients that support it
                            let _ = peer_clone
                                .notify_logging_message(LoggingMessageNotificationParam {
                                    level: LoggingLevel::Info,
                                    logger: Some("workflow".to_string()),
                                    data: json!({
                                        "type": "step_started",
                                        "step": step_counter,
                                        "total": total_steps,
                                        "name": step_name
                                    }),
                                })
                                .await;
                            // Also send as progress notification
                            let _ = peer_clone
                                .notify_progress(ProgressNotificationParam {
                                    progress_token: progress_token_clone.clone(),
                                    progress: step_counter as f64,
                                    total: total_steps.map(|t| t as f64),
                                    message: Some(format!("Starting: {}", step_name)),
                                })
                                .await;
                        }
                        WorkflowEvent::StepCompleted {
                            step_name,
                            duration,
                            step_index,
                            ..
                        } => {
                            let step = step_index.unwrap_or(step_counter);
                            let _ = peer_clone
                                .notify_logging_message(LoggingMessageNotificationParam {
                                    level: LoggingLevel::Info,
                                    logger: Some("workflow".to_string()),
                                    data: json!({
                                        "type": "step_completed",
                                        "step": step,
                                        "name": step_name,
                                        "duration_ms": duration
                                    }),
                                })
                                .await;
                        }
                        WorkflowEvent::StepFailed {
                            step_name, error, ..
                        } => {
                            let _ = peer_clone
                                .notify_logging_message(LoggingMessageNotificationParam {
                                    level: LoggingLevel::Error,
                                    logger: Some("workflow".to_string()),
                                    data: json!({
                                        "type": "step_failed",
                                        "name": step_name,
                                        "error": error
                                    }),
                                })
                                .await;
                        }
                        WorkflowEvent::Log {
                            level,
                            message,
                            data,
                            ..
                        } => {
                            let log_level = match level.as_str() {
                                "error" => LoggingLevel::Error,
                                "warn" | "warning" => LoggingLevel::Warning,
                                "debug" => LoggingLevel::Debug,
                                _ => LoggingLevel::Info,
                            };
                            let _ = peer_clone
                                .notify_logging_message(LoggingMessageNotificationParam {
                                    level: log_level,
                                    logger: Some("workflow".to_string()),
                                    data: data.unwrap_or_else(|| json!({ "message": message })),
                                })
                                .await;
                        }
                        WorkflowEvent::Data { key, value, .. } => {
                            let _ = peer_clone
                                .notify_logging_message(LoggingMessageNotificationParam {
                                    level: LoggingLevel::Info,
                                    logger: Some("workflow.data".to_string()),
                                    data: json!({ "key": key, "value": value }),
                                })
                                .await;
                        }
                        WorkflowEvent::Screenshot {
                            path, annotation, ..
                        } => {
                            let _ = peer_clone
                                .notify_logging_message(LoggingMessageNotificationParam {
                                    level: LoggingLevel::Info,
                                    logger: Some("workflow.screenshot".to_string()),
                                    data: json!({
                                        "type": "screenshot",
                                        "path": path,
                                        "annotation": annotation
                                    }),
                                })
                                .await;
                        }
                    }
                }
            });

            // Execute workflow with event streaming
            let result = ts_workflow
                .execute_with_events(
                    args.inputs.unwrap_or(json!({})),
                    args.start_from_step.as_deref(),
                    args.end_at_step.as_deref(),
                    restored_state,
                    Some(&execution_id_val),
                    Some(event_tx),
                )
                .await?;

            // Wait for notification handler to finish (it will exit when sender is dropped)
            let _ = notification_handle.await;

            // Save state for resumption (only if last_step_index is provided by runner-based workflows)
            if let (Some(ref last_step_id), Some(last_step_index)) = (
                &result.result.result.last_step_id,
                result.result.result.last_step_index,
            ) {
                Self::save_workflow_state(
                    args.workflow_id.as_deref(),
                    Some(url),
                    Some(last_step_id),
                    last_step_index,
                    &result.result.state,
                )
                .await?;
            }

            // Return result
            let mut output = json!({
                "status": result.result.result.status,
                "message": result.result.result.message,
                "data": result.result.result.data,
                "metadata": result.result.metadata,
                "state": result.result.state,
                "last_step_id": result.result.result.last_step_id,
                "last_step_index": result.result.result.last_step_index,
            });

            // If there's data from context.data, add it as parsed_output for CLI compatibility
            if let Some(data) = &result.result.result.data {
                if !data.is_null() {
                    if let Some(obj) = output.as_object_mut() {
                        obj.insert(
                            "parsed_output".to_string(),
                            json!({
                                "data": data
                            }),
                        );
                    }
                }
            }

            // Restore windows after TypeScript workflow completion (success or failure)
            let window_mgmt_enabled = args.window_mgmt.enable_window_management.unwrap_or(true);
            if window_mgmt_enabled {
                if let Err(e) = self.window_manager.restore_all_windows().await {
                    tracing::warn!("Failed to restore windows after TypeScript workflow: {}", e);
                } else {
                    tracing::info!(
                        "Restored all windows to original state after TypeScript workflow"
                    );
                }
                self.window_manager.clear_captured_state().await;
            } else {
                tracing::debug!(
                    "Window management disabled for TypeScript workflow, skipping restore"
                );
            }

            // Store captured stderr logs for dispatch_tool to include in execution log
            if let Ok(mut logs) = self.captured_stderr_logs.lock() {
                logs.clear();
                logs.extend(result.logs);
            }

            Ok(CallToolResult {
                content: vec![Content::text(
                    serde_json::to_string_pretty(&output).unwrap(),
                )],
                is_error: Some(result.result.result.status != "success"),
                meta: None,
                structured_content: None,
            })
        }
        .instrument(workflow_span)
        .await
    }
}
