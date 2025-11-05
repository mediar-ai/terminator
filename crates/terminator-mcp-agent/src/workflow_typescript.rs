// TypeScript workflow executor

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;
use tracing::{debug, info};

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
                    (grandparent.to_path_buf(), format!("src/{}", file_name))
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
        // Ensure dependencies are installed and cached
        self.ensure_dependencies().await?;

        // Create execution script
        let exec_script =
            self.create_execution_script(inputs, start_from_step, end_at_step, restored_state)?;

        debug!(
            "Executing TypeScript workflow with script:\n{}",
            exec_script
        );

        // Execute via bun (priority) or node (fallback)
        let runtime = detect_js_runtime();
        let output = match runtime {
            JsRuntime::Bun => {
                info!(
                    "Executing workflow with bun: {}/{}",
                    self.workflow_path.display(),
                    self.entry_file
                );
                Command::new("bun")
                    .current_dir(&self.workflow_path)
                    .arg("--eval")
                    .arg(&exec_script)
                    .output()
                    .map_err(|e| {
                        McpError::internal_error(
                            format!("Failed to execute workflow with bun: {}", e),
                            Some(json!({"error": e.to_string()})),
                        )
                    })?
            }
            JsRuntime::Node => {
                info!(
                    "Executing workflow with node: {}/{}",
                    self.workflow_path.display(),
                    self.entry_file
                );
                Command::new("node")
                    .current_dir(&self.workflow_path)
                    .arg("--import")
                    .arg("tsx/esm")
                    .arg("--eval")
                    .arg(&exec_script)
                    .output()
                    .map_err(|e| {
                        McpError::internal_error(
                            format!("Failed to execute workflow with node: {}", e),
                            Some(json!({"error": e.to_string()})),
                        )
                    })?
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(McpError::internal_error(
                format!("Workflow execution failed: {}", stderr),
                Some(json!({
                    "stderr": stderr.to_string(),
                    "stdout": stdout.to_string(),
                    "exit_code": output.status.code(),
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
                format!("Invalid workflow result: {}", e),
                Some(json!({
                    "error": e.to_string(),
                    "output": result_json.to_string(),
                    "extracted_json": json_result,
                })),
            )
        })?;

        Ok(result)
    }

    fn create_execution_script(
        &self,
        inputs: Value,
        start_from_step: Option<&str>,
        end_at_step: Option<&str>,
        restored_state: Option<Value>,
    ) -> Result<String, McpError> {
        let inputs_json = serde_json::to_string(&inputs).map_err(|e| {
            McpError::internal_error(
                format!("Failed to serialize inputs: {}", e),
                Some(json!({"error": e.to_string()})),
            )
        })?;

        let start_from_step_json = start_from_step.map(|s| format!("\"{}\"", s)).unwrap_or_else(|| "undefined".to_string());
        let end_at_step_json = end_at_step.map(|s| format!("\"{}\"", s)).unwrap_or_else(|| "undefined".to_string());
        let restored_state_json = restored_state.as_ref()
            .map(|s| serde_json::to_string(s).unwrap_or_else(|_| "undefined".to_string()))
            .unwrap_or_else(|| "undefined".to_string());

        // Convert Windows path to forward slashes for file:// URL
        let workflow_path_str = self.workflow_path.display().to_string();
        let workflow_path = workflow_path_str.replace('\\', "/");
        let entry_file = &self.entry_file;

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
process.env.CI = 'true';  // Many tools respect CI env var for silent mode

try {{
    // Dynamically import the workflow
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

    // Prepare execution options
    const inputs = {inputs_json};
    const startFromStep = {start_from_step_json};
    const endAtStep = {end_at_step_json};
    const restoredState = {restored_state_json};

    // Use WorkflowRunner for advanced features (partial execution, state restoration)
    let result;
    let finalState;

    if (startFromStep || endAtStep || restoredState) {{
        // Import WorkflowRunner for partial execution
        const {{ createWorkflowRunner }} = await import('file://{workflow_path}/node_modules/@mediar-ai/workflow/dist/runner.js');

        const runner = createWorkflowRunner({{
            workflow: workflow,
            inputs: inputs,
            startFromStep: startFromStep,
            endAtStep: endAtStep,
            restoredState: restoredState
        }});

        const runnerResult = await runner.run();
        finalState = runner.getState();

        // Convert runner result to workflow result format
        result = {{
            status: runnerResult.status,
            message: runnerResult.error || `Workflow partial execution completed`,
            data: finalState.context.data
        }};
    }} else {{
        // Simple execution - use workflow.run() directly
        result = await workflow.run(inputs);
        finalState = {{}};
    }}

    // Get metadata
    const metadata = workflow.getMetadata ? workflow.getMetadata() : {{
        name: workflow.config?.name || 'Unknown',
        version: workflow.config?.version || '1.0.0',
        description: workflow.config?.description || '',
        steps: workflow.steps || []
    }};

    // Output clean JSON result
    originalLog(JSON.stringify({{
        metadata: metadata,
        result: result,
        state: finalState
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

    /// Ensure dependencies are installed
    ///
    /// Simple strategy: Just run bun/npm install in the workflow directory.
    /// Since workflow is mounted from S3, node_modules will be persisted there automatically.
    async fn ensure_dependencies(&self) -> Result<(), McpError> {
        let package_json_path = self.workflow_path.join("package.json");

        // Check if package.json exists
        if !package_json_path.exists() {
            info!("No package.json found - skipping dependency installation");
            return Ok(());
        }

        let workflow_node_modules = self.workflow_path.join("node_modules");
        let runtime = detect_js_runtime();

        // Check if dependencies need updating by comparing package.json mtime with lockfile
        let needs_install = if workflow_node_modules.exists() {
            let lockfile_path = match runtime {
                JsRuntime::Bun => self.workflow_path.join("bun.lockb"),
                JsRuntime::Node => self.workflow_path.join("package-lock.json"),
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
                .current_dir(&self.workflow_path)
                .output(),
            JsRuntime::Node => Command::new("npm")
                .arg("install")
                .current_dir(&self.workflow_path)
                .output(),
        }
        .map_err(|e| {
            McpError::internal_error(
                format!("Failed to run dependency installation: {}", e),
                Some(json!({"error": e.to_string()})),
            )
        })?;

        if !install_result.status.success() {
            let stderr = String::from_utf8_lossy(&install_result.stderr);
            return Err(McpError::internal_error(
                format!("Dependency installation failed: {}", stderr),
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
