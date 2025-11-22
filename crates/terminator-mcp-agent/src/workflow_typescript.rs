// TypeScript workflow executor

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, info, warn};

use rmcp::ErrorData as McpError;

#[derive(Debug, Clone, PartialEq)]
pub enum JsRuntime {
    Bun,
    Node,
}

/// Detect available JavaScript runtime (prefer bun, fallback to node)
pub fn detect_js_runtime() -> JsRuntime {
    // Try bun first
    if let Ok(output) = Command::new("bun").arg("--version").output() {
        if output.status.success() {
            info!("Using bun runtime");
            return JsRuntime::Bun;
        }
    }

    // Fallback to node
    info!("Bun not found, using node runtime");
    JsRuntime::Node
}

/// Copy directory contents recursively (cross-platform)
fn copy_dir_recursive(src: &PathBuf, dst: &PathBuf) -> Result<(), McpError> {
    use std::fs;

    debug!("Copying {} to {}", src.display(), dst.display());

    // Create destination directory
    fs::create_dir_all(dst).map_err(|e| {
        McpError::internal_error(
            format!("Failed to create temp directory: {e}"),
            Some(json!({"error": e.to_string(), "path": dst.display().to_string()})),
        )
    })?;

    // On Windows, use robocopy for better performance and symlink handling
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("robocopy")
            .arg(src)
            .arg(dst)
            .arg("/E") // Copy subdirectories, including empty ones
            .arg("/NFL") // No file list
            .arg("/NDL") // No directory list
            .arg("/NJH") // No job header
            .arg("/NJS") // No job summary
            .arg("/nc") // No class
            .arg("/ns") // No size
            .arg("/np") // No progress
            .output()
            .map_err(|e| {
                McpError::internal_error(
                    format!("Failed to execute robocopy: {e}"),
                    Some(json!({"error": e.to_string()})),
                )
            })?;

        // robocopy exit codes: 0-7 are success, 8+ are errors
        let exit_code = output.status.code().unwrap_or(16);
        if exit_code >= 8 {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(McpError::internal_error(
                format!("Robocopy failed with exit code {exit_code}: {stderr}"),
                Some(json!({
                    "exit_code": exit_code,
                    "stderr": stderr.to_string(),
                })),
            ));
        }

        debug!("Successfully copied directory using robocopy");
        Ok(())
    }

    // On Unix systems, use cp -r
    #[cfg(not(target_os = "windows"))]
    {
        let output = Command::new("cp")
            .arg("-r")
            .arg(src)
            .arg(dst)
            .output()
            .map_err(|e| {
                McpError::internal_error(
                    format!("Failed to execute cp: {e}"),
                    Some(json!({"error": e.to_string()})),
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(McpError::internal_error(
                format!("cp failed: {stderr}"),
                Some(json!({
                    "stderr": stderr.to_string(),
                })),
            ));
        }

        debug!("Successfully copied directory using cp");
        Ok(())
    }
}

/// Clean up temporary directory
fn cleanup_temp_dir(path: &PathBuf) {
    use std::fs;
    if let Err(e) = fs::remove_dir_all(path) {
        warn!(
            "Failed to clean up temporary directory {}: {}",
            path.display(),
            e
        );
    } else {
        debug!("Cleaned up temporary directory: {}", path.display());
    }
}

#[derive(Debug)]
pub struct TypeScriptWorkflow {
    workflow_path: PathBuf,
    entry_file: String,
}

impl TypeScriptWorkflow {
    /// Validate that only one workflow exists in the folder
    fn validate_single_workflow(path: &PathBuf) -> Result<(), McpError> {
        use std::fs;

        // Count .ts files that might be workflows (excluding terminator.ts itself)
        let mut workflow_files = Vec::new();

        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_file() {
                        if let Some(file_name) = entry.file_name().to_str() {
                            // Check for common workflow file patterns (but not terminator.ts)
                            if file_name.ends_with(".workflow.ts")
                                || (file_name.ends_with(".ts")
                                    && file_name != "terminator.ts"
                                    && file_name.contains("workflow"))
                            {
                                workflow_files.push(file_name.to_string());
                            }
                        }
                    }
                }
            }
        }

        if !workflow_files.is_empty() {
            return Err(McpError::invalid_params(
                format!(
                    "Multiple workflow files detected. Only one workflow per folder is allowed. Found: {}",
                    workflow_files.join(", ")
                ),
                Some(json!({
                    "path": path.display().to_string(),
                    "conflicting_files": workflow_files,
                    "hint": "Move additional workflows to separate folders or rename them to not include 'workflow' in the filename"
                })),
            ));
        }

        Ok(())
    }

    pub fn new(url: &str) -> Result<Self, McpError> {
        let path_str = url.strip_prefix("file://").ok_or_else(|| {
            McpError::invalid_params(
                "TypeScript workflows must use file:// URLs".to_string(),
                Some(json!({"url": url})),
            )
        })?;

        let path = PathBuf::from(path_str);

        // Determine workflow path and entry file
        let (workflow_path, entry_file) = if path.is_dir() {
            // Directory: Check for terminator.ts in root or src/
            let root_terminator = path.join("terminator.ts");
            let src_terminator = path.join("src").join("terminator.ts");

            let entry_file = if root_terminator.exists() {
                "terminator.ts".to_string()
            } else if src_terminator.exists() {
                "src/terminator.ts".to_string()
            } else {
                return Err(McpError::invalid_params(
                    "Missing required entrypoint: terminator.ts or src/terminator.ts. TypeScript workflows must use 'terminator.ts' as the entry file.".to_string(),
                    Some(json!({
                        "path": path.display().to_string(),
                        "hint": "Create a terminator.ts or src/terminator.ts file that exports your workflow"
                    })),
                ));
            };

            // Validate single workflow per folder
            Self::validate_single_workflow(&path)?;

            (path, entry_file)
        } else if path.is_file() {
            // File: determine the workflow root directory
            let parent = path.parent().ok_or_else(|| {
                McpError::invalid_params(
                    "Cannot determine parent directory".to_string(),
                    Some(json!({"path": path.display().to_string()})),
                )
            })?;

            // If the file is in a src/ directory, use the parent of src/ as the workflow path
            let (workflow_path, relative_entry) =
                if parent.file_name() == Some(std::ffi::OsStr::new("src")) {
                    let grandparent = parent.parent().ok_or_else(|| {
                        McpError::invalid_params(
                            "Cannot determine workflow root directory".to_string(),
                            Some(json!({"path": path.display().to_string()})),
                        )
                    })?;
                    let file_name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
                        McpError::invalid_params(
                            "Invalid file name".to_string(),
                            Some(json!({"path": path.display().to_string()})),
                        )
                    })?;
                    (grandparent.to_path_buf(), format!("src/{file_name}"))
                } else {
                    // Use parent directory and file name
                    let file_name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
                        McpError::invalid_params(
                            "Invalid file name".to_string(),
                            Some(json!({"path": path.display().to_string()})),
                        )
                    })?;
                    (parent.to_path_buf(), file_name.to_string())
                };

            (workflow_path, relative_entry)
        } else {
            return Err(McpError::invalid_params(
                "Workflow path does not exist".to_string(),
                Some(json!({"path": path.display().to_string()})),
            ));
        };

        Ok(Self {
            workflow_path,
            entry_file,
        })
    }

    /// Execute the entire TypeScript workflow with state management
    pub async fn execute(
        &self,
        inputs: Value,
        start_from_step: Option<&str>,
        end_at_step: Option<&str>,
        restored_state: Option<Value>,
    ) -> Result<TypeScriptWorkflowResult, McpError> {
        use std::env;

        // Check execution mode
        let execution_mode = env::var("MCP_EXECUTION_MODE").unwrap_or_default();
        let use_local_copy = execution_mode == "local-copy";

        // Determine execution directory
        let (execution_dir, temp_dir_guard) = if use_local_copy {
            info!("🔄 Local-copy mode enabled - copying workflow to temporary directory");

            // Create unique temporary directory
            let temp_base = env::var("TEMP")
                .or_else(|_| env::var("TMP"))
                .unwrap_or_else(|_| {
                    if cfg!(target_os = "windows") {
                        "C:\\Temp".to_string()
                    } else {
                        "/tmp".to_string()
                    }
                });

            let temp_dir =
                PathBuf::from(temp_base).join(format!("mcp-exec-{}", uuid::Uuid::new_v4()));

            info!("📁 Temporary directory: {}", temp_dir.display());

            // Copy workflow files to temp directory
            copy_dir_recursive(&self.workflow_path, &temp_dir)?;

            info!("✅ Files copied successfully");

            (temp_dir.clone(), Some(temp_dir))
        } else {
            debug!("📍 Direct mode - executing from source directory");
            (self.workflow_path.clone(), None)
        };

        // Ensure dependencies are installed and cached
        self.ensure_dependencies_in(&execution_dir).await?;

        // Create execution script (using execution_dir for imports)
        let exec_script = self.create_execution_script(
            &execution_dir,
            inputs,
            start_from_step,
            end_at_step,
            restored_state,
        )?;

        debug!(
            "Executing TypeScript workflow with script:\n{}",
            exec_script
        );

        // Execute via bun (priority) or node (fallback)
        // CRITICAL: Use spawn() with inherited stderr for real-time log streaming
        // The .output() method buffers all output until completion, hiding logs during execution
        let runtime = detect_js_runtime();

        use std::process::Stdio;
        let child = match runtime {
            JsRuntime::Bun => {
                info!(
                    "Executing workflow with bun: {}/{}",
                    execution_dir.display(),
                    self.entry_file
                );
                Command::new("bun")
                    .current_dir(&execution_dir)
                    .arg("--eval")
                    .arg(&exec_script)
                    .stdout(Stdio::piped()) // Capture stdout for JSON result
                    .stderr(Stdio::inherit()) // Stream stderr (logs) in real-time
                    .spawn()
                    .map_err(|e| {
                        McpError::internal_error(
                            format!("Failed to execute workflow with bun: {e}"),
                            Some(json!({"error": e.to_string()})),
                        )
                    })?
            }
            JsRuntime::Node => {
                info!(
                    "Executing workflow with node: {}/{}",
                    execution_dir.display(),
                    self.entry_file
                );
                Command::new("node")
                    .current_dir(&execution_dir)
                    .arg("--import")
                    .arg("tsx/esm")
                    .arg("--eval")
                    .arg(&exec_script)
                    .stdout(Stdio::piped()) // Capture stdout for JSON result
                    .stderr(Stdio::inherit()) // Stream stderr (logs) in real-time
                    .spawn()
                    .map_err(|e| {
                        McpError::internal_error(
                            format!("Failed to execute workflow with node: {e}"),
                            Some(json!({"error": e.to_string()})),
                        )
                    })?
            }
        };

        // Wait for completion and get output
        let output = child.wait_with_output().map_err(|e| {
            McpError::internal_error(
                format!("Failed to wait for workflow completion: {e}"),
                Some(json!({"error": e.to_string()})),
            )
        })?;

        if !output.status.success() {
            // Note: stderr was streamed to console in real-time (Stdio::inherit)
            // so we won't have it captured here - that's intentional for better UX
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(McpError::internal_error(
                format!(
                    "Workflow execution failed with exit code: {:?}",
                    output.status.code()
                ),
                Some(json!({
                    "stdout": stdout.to_string(),
                    "exit_code": output.status.code(),
                    "note": "stderr was streamed to console in real-time"
                })),
            ));
        }

        // Parse result - try to extract JSON from potentially mixed output
        let result_json = String::from_utf8_lossy(&output.stdout);
        debug!("Workflow output:\n{}", result_json);

        // Try to find JSON in the output (it should start with { and end with })
        let json_result = if let Some(start) = result_json.rfind("\n{") {
            // Found JSON after newline, extract from there
            &result_json[start + 1..]
        } else if result_json.trim().starts_with('{') {
            // The whole output is JSON
            result_json.trim()
        } else {
            // Try to find any JSON object in the output
            if let Some(start) = result_json.find('{') {
                if let Some(end) = result_json.rfind('}') {
                    &result_json[start..=end]
                } else {
                    &result_json[start..]
                }
            } else {
                // No JSON found at all
                return Err(McpError::internal_error(
                    "No JSON output found in workflow result".to_string(),
                    Some(json!({
                        "output": result_json.to_string(),
                        "stderr": String::from_utf8_lossy(&output.stderr).to_string(),
                    })),
                ));
            }
        };

        let result: TypeScriptWorkflowResult = serde_json::from_str(json_result).map_err(|e| {
            McpError::internal_error(
                format!("Invalid workflow result: {e}"),
                Some(json!({
                    "error": e.to_string(),
                    "output": result_json.to_string(),
                    "extracted_json": json_result,
                })),
            )
        })?;

        // Clean up temporary directory if used
        if let Some(temp_dir) = temp_dir_guard {
            cleanup_temp_dir(&temp_dir);
        }

        Ok(result)
    }

    fn create_execution_script(
        &self,
        execution_dir: &Path,
        inputs: Value,
        start_from_step: Option<&str>,
        end_at_step: Option<&str>,
        restored_state: Option<Value>,
    ) -> Result<String, McpError> {
        // Convert Windows path to forward slashes for file:// URL
        let workflow_path_str = execution_dir.display().to_string();
        let workflow_path = workflow_path_str.replace('\\', "/");
        let entry_file = &self.entry_file;

        // Serialize inputs
        let inputs_json = serde_json::to_string(&inputs).map_err(|e| {
            McpError::internal_error(
                format!("Failed to serialize inputs: {e}"),
                Some(json!({"error": e.to_string()})),
            )
        })?;

        // Build step control options object
        let mut step_options_obj = serde_json::Map::new();
        if let Some(start) = start_from_step {
            step_options_obj.insert("startFromStep".to_string(), json!(start));
        }
        if let Some(end) = end_at_step {
            step_options_obj.insert("endAtStep".to_string(), json!(end));
        }
        if let Some(state) = restored_state {
            step_options_obj.insert("restoredState".to_string(), state);
        }

        let step_options_json =
            serde_json::to_string(&Value::Object(step_options_obj)).map_err(|e| {
                McpError::internal_error(
                    format!("Failed to serialize step options: {e}"),
                    Some(json!({"error": e.to_string()})),
                )
            })?;

        // Clean approach: Call workflow.run() with step control options
        // This automatically skips onError when step control options are present
        Ok(format!(
            r#"
// Suppress workflow progress output by redirecting console methods to stderr
const originalLog = console.log;
const originalInfo = console.info;
console.log = (...args) => {{
    // Only allow JSON output to stdout
    if (args.length === 1 && typeof args[0] === 'string' && args[0].startsWith('{{')) {{
        originalLog(...args);
    }} else {{
        console.error(...args);
    }}
}};
console.info = console.error;

// Set environment to suppress workflow output if supported
process.env.WORKFLOW_SILENT = 'true';
process.env.CI = 'true';

try {{
    // Import workflow
    const workflowModule = await import('file://{workflow_path}/{entry_file}');
    const workflow = workflowModule.default || workflowModule.bestPlanProWorkflow || workflowModule;

    // Check if we're just getting metadata
    if (process.argv.includes('--get-metadata')) {{
        const metadata = workflow.getMetadata ? workflow.getMetadata() : {{
            name: workflow.config?.name || 'Unknown',
            version: workflow.config?.version || '1.0.0',
            description: workflow.config?.description || '',
            steps: workflow.steps || []
        }};
        originalLog(JSON.stringify({{ metadata }}, null, 2));
        process.exit(0);
    }}

    // Execute workflow using workflow.run() with step control options
    // This automatically skips onError when step control options are present
    const inputs = {inputs_json};
    const stepOptions = {step_options_json};

    // Debug logging
    console.error('[DEBUG] Step options being passed to workflow.run():', JSON.stringify(stepOptions));
    console.error('[DEBUG] Workflow has run method?', typeof workflow.run);
    console.error('[DEBUG] Inputs:', JSON.stringify(inputs));

    const result = await workflow.run(inputs, undefined, undefined, stepOptions);

    // Debug the result
    console.error('[DEBUG] Result from workflow.run():', JSON.stringify(result));

    // Get workflow metadata for response
    const metadata = workflow.getMetadata ? workflow.getMetadata() : {{
        name: workflow.config?.name || 'Unknown',
        version: workflow.config?.version || '1.0.0',
        description: workflow.config?.description || ''
    }};

    // Output clean JSON result
    // CRITICAL: Include lastStepId and lastStepIndex from SDK for state persistence
    originalLog(JSON.stringify({{
        metadata,
        result: {{
            status: result.status || 'success',
            message: result.message || result.error || 'Workflow completed',
            data: result.data || result.context?.data || null,
            last_step_id: result.lastStepId,
            last_step_index: result.lastStepIndex
        }},
        state: result.state || {{ context: {{ data: result.data }} }}
    }}, null, 2));

    process.exit(result.status === 'success' ? 0 : 1);
}} catch (error) {{
    console.error('Workflow execution error:', error);
    originalLog(JSON.stringify({{
        metadata: {{ name: 'Error', version: '0.0.0' }},
        result: {{
            status: 'error',
            error: error.message || String(error)
        }},
        state: {{}}
    }}, null, 2));
    process.exit(1);
}}
"#
        ))
    }

    /// Ensure dependencies are installed in a specific directory
    ///
    /// Simple strategy: Just run bun/npm install in the workflow directory.
    /// In local-copy mode, installs in temp dir. In direct mode, installs in source (S3).
    async fn ensure_dependencies_in(&self, workflow_dir: &PathBuf) -> Result<(), McpError> {
        let package_json_path = workflow_dir.join("package.json");

        // Check if package.json exists
        if !package_json_path.exists() {
            info!("No package.json found - skipping dependency installation");
            return Ok(());
        }

        let workflow_node_modules = workflow_dir.join("node_modules");
        let runtime = detect_js_runtime();

        // Check if dependencies need updating by comparing package.json mtime with lockfile
        let needs_install = if workflow_node_modules.exists() {
            let lockfile_path = match runtime {
                JsRuntime::Bun => workflow_dir.join("bun.lockb"),
                JsRuntime::Node => workflow_dir.join("package-lock.json"),
            };

            // If lockfile doesn't exist, need to install
            if !lockfile_path.exists() {
                info!("⏳ Lockfile not found - running install to generate it");
                true
            } else {
                // Compare modification times
                let package_json_mtime =
                    package_json_path.metadata().and_then(|m| m.modified()).ok();
                let lockfile_mtime = lockfile_path.metadata().and_then(|m| m.modified()).ok();

                match (package_json_mtime, lockfile_mtime) {
                    (Some(pkg_time), Some(lock_time)) => {
                        if pkg_time > lock_time {
                            info!("⏳ package.json newer than lockfile - updating dependencies");
                            true
                        } else {
                            info!("✓ Dependencies up to date (lockfile is fresh)");
                            false
                        }
                    }
                    _ => {
                        // Can't determine - safer to reinstall
                        info!("⏳ Could not check file times - reinstalling dependencies");
                        true
                    }
                }
            }
        } else {
            info!("⏳ node_modules not found - installing dependencies");
            true
        };

        if !needs_install {
            return Ok(());
        }

        // Install dependencies in workflow directory (will be persisted to S3)
        info!("⏳ Installing dependencies...");

        let install_result = match runtime {
            JsRuntime::Bun => Command::new("bun")
                .arg("install")
                .current_dir(workflow_dir)
                .output(),
            JsRuntime::Node => Command::new("npm")
                .arg("install")
                .current_dir(workflow_dir)
                .output(),
        }
        .map_err(|e| {
            McpError::internal_error(
                format!("Failed to run dependency installation: {e}"),
                Some(json!({"error": e.to_string()})),
            )
        })?;

        if !install_result.status.success() {
            let stderr = String::from_utf8_lossy(&install_result.stderr);
            return Err(McpError::internal_error(
                format!("Dependency installation failed: {stderr}"),
                Some(json!({
                    "stderr": stderr.to_string(),
                    "stdout": String::from_utf8_lossy(&install_result.stdout).to_string(),
                })),
            ));
        }

        info!("✓ Dependencies installed successfully");
        info!("✓ node_modules will be persisted to S3 with workflow");

        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TypeScriptWorkflowResult {
    pub metadata: WorkflowMetadata,
    pub result: WorkflowExecutionResult,
    pub state: Value,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WorkflowMetadata {
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub input: Value,
    pub steps: Vec<StepMetadata>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StepMetadata {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WorkflowExecutionResult {
    pub status: String,
    pub message: Option<String>,
    pub data: Option<Value>,
    // Fields from WorkflowRunner (optional for backward compat)
    pub last_step_id: Option<String>,
    pub last_step_index: Option<usize>,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_bun_or_node() {
        let runtime = detect_js_runtime();
        // Should return either Bun or Node (depending on environment)
        assert!(matches!(runtime, JsRuntime::Bun) || matches!(runtime, JsRuntime::Node));
    }

    #[test]
    fn test_typescript_workflow_from_file() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let workflow_file = temp_dir.path().join("test-workflow.ts");
        fs::write(&workflow_file, "export default {};").unwrap();

        let url = format!("file://{}", workflow_file.display());
        let ts_workflow = TypeScriptWorkflow::new(&url).unwrap();

        assert_eq!(ts_workflow.entry_file, "test-workflow.ts");
        assert_eq!(ts_workflow.workflow_path, temp_dir.path());
    }

    #[test]
    fn test_typescript_workflow_requires_terminator_ts() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();

        // Create terminator.ts
        fs::write(temp_dir.path().join("terminator.ts"), "export default {};").unwrap();

        let url = format!("file://{}", temp_dir.path().display());
        let ts_workflow = TypeScriptWorkflow::new(&url).unwrap();

        assert_eq!(ts_workflow.entry_file, "terminator.ts");
        assert_eq!(ts_workflow.workflow_path, temp_dir.path());
    }

    #[test]
    fn test_typescript_workflow_missing_terminator_ts() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();

        // Create other workflow file, but no terminator.ts
        fs::write(temp_dir.path().join("my-workflow.ts"), "export default {};").unwrap();

        let url = format!("file://{}", temp_dir.path().display());
        let result = TypeScriptWorkflow::new(&url);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err
            .message
            .contains("Missing required entrypoint: terminator.ts"));
    }

    #[test]
    fn test_single_workflow_validation_passes() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();

        // Create only terminator.ts (no other workflow files)
        fs::write(temp_dir.path().join("terminator.ts"), "export default {};").unwrap();
        fs::write(
            temp_dir.path().join("utils.ts"),
            "export const helper = () => {};",
        )
        .unwrap();

        let url = format!("file://{}", temp_dir.path().display());
        let result = TypeScriptWorkflow::new(&url);

        assert!(result.is_ok());
    }

    #[test]
    fn test_single_workflow_validation_fails_with_multiple_workflows() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();

        // Create terminator.ts and another workflow file
        fs::write(temp_dir.path().join("terminator.ts"), "export default {};").unwrap();
        fs::write(temp_dir.path().join("my-workflow.ts"), "export default {};").unwrap();

        let url = format!("file://{}", temp_dir.path().display());
        let result = TypeScriptWorkflow::new(&url);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Multiple workflow files detected"));
        assert!(err.message.contains("my-workflow.ts"));
    }
}
