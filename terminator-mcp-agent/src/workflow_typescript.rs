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

pub struct TypeScriptWorkflow {
    workflow_path: PathBuf,
    entry_file: String,
}

impl TypeScriptWorkflow {
    pub fn new(url: &str) -> Result<Self, McpError> {
        let path_str = url
            .strip_prefix("file://")
            .ok_or_else(|| {
                McpError::invalid_params(
                    "TypeScript workflows must use file:// URLs".to_string(),
                    Some(json!({"url": url})),
                )
            })?;

        let path = PathBuf::from(path_str);

        // Determine workflow path and entry file
        let (workflow_path, entry_file) = if path.is_dir() {
            // Directory: look for workflow.ts or index.ts
            if path.join("workflow.ts").exists() {
                (path, "workflow.ts".to_string())
            } else if path.join("index.ts").exists() {
                (path, "index.ts".to_string())
            } else {
                return Err(McpError::invalid_params(
                    "No workflow.ts or index.ts found in directory".to_string(),
                    Some(json!({"path": path.display().to_string()})),
                ));
            }
        } else if path.is_file() {
            // File: use parent directory and file name
            let parent = path.parent().ok_or_else(|| {
                McpError::invalid_params(
                    "Cannot determine parent directory".to_string(),
                    Some(json!({"path": path.display().to_string()})),
                )
            })?;
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| {
                    McpError::invalid_params(
                        "Invalid file name".to_string(),
                        Some(json!({"path": path.display().to_string()})),
                    )
                })?;

            (parent.to_path_buf(), file_name.to_string())
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
        // Create execution script
        let exec_script = self.create_execution_script(
            inputs,
            start_from_step,
            end_at_step,
            restored_state,
        )?;

        debug!("Executing TypeScript workflow with script:\n{}", exec_script);

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

        // Parse result
        let result_json = String::from_utf8_lossy(&output.stdout);
        debug!("Workflow output:\n{}", result_json);

        let result: TypeScriptWorkflowResult = serde_json::from_str(&result_json).map_err(|e| {
            McpError::internal_error(
                format!("Invalid workflow result: {}", e),
                Some(json!({
                    "error": e.to_string(),
                    "output": result_json.to_string(),
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

        let start_from_json = start_from_step
            .map(|s| format!("'{}'", s.replace("'", "\\'")))
            .unwrap_or_else(|| "null".to_string());

        let end_at_json = end_at_step
            .map(|s| format!("'{}'", s.replace("'", "\\'")))
            .unwrap_or_else(|| "null".to_string());

        let restored_state_json = if let Some(state) = restored_state {
            serde_json::to_string(&state).map_err(|e| {
                McpError::internal_error(
                    format!("Failed to serialize restored state: {}", e),
                    Some(json!({"error": e.to_string()})),
                )
            })?
        } else {
            "null".to_string()
        };

        let workflow_path = self.workflow_path.display();
        let entry_file = &self.entry_file;

        // Use relative path to terminator-workflow package
        let terminator_workflow_path = self.workflow_path
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("packages/terminator-workflow/dist"))
            .unwrap_or_else(|| PathBuf::from("../../packages/terminator-workflow/dist"));

        Ok(format!(
            r#"
import {{ createWorkflowRunner }} from 'file://{terminator_workflow_path}/runner.js';

const workflow = await import('file://{workflow_path}/{entry_file}');

// Create a silent logger that writes to stderr instead of stdout
const silentLogger = {{
    info: (msg) => console.error(msg),
    success: (msg) => console.error(msg),
    warn: (msg) => console.error(msg),
    error: (msg) => console.error(msg),
    debug: (msg) => console.error(msg),
}};

const runner = createWorkflowRunner({{
    workflow: workflow.default,
    inputs: {inputs_json},
    startFromStep: {start_from_json},
    endAtStep: {end_at_json},
    restoredState: {restored_state_json},
    logger: silentLogger,
}});

const result = await runner.run();

// Output ONLY JSON to stdout (logs went to stderr)
console.log(JSON.stringify({{
    metadata: workflow.default.getMetadata(),
    result: result,
    state: runner.getState(),
}}));
"#,
            terminator_workflow_path = terminator_workflow_path.display().to_string().replace('\\', "/")
        ))
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
#[serde(rename_all = "camelCase")]
pub struct WorkflowExecutionResult {
    pub status: String,
    pub last_step_id: Option<String>,
    pub last_step_index: usize,
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
        let workflow_file = temp_dir.path().join("workflow.ts");
        fs::write(&workflow_file, "export default {};").unwrap();

        let url = format!("file://{}", workflow_file.display());
        let ts_workflow = TypeScriptWorkflow::new(&url).unwrap();

        assert_eq!(ts_workflow.entry_file, "workflow.ts");
        assert_eq!(ts_workflow.workflow_path, temp_dir.path());
    }

    #[test]
    fn test_typescript_workflow_from_directory() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("workflow.ts"), "export default {};").unwrap();

        let url = format!("file://{}", temp_dir.path().display());
        let ts_workflow = TypeScriptWorkflow::new(&url).unwrap();

        assert_eq!(ts_workflow.entry_file, "workflow.ts");
        assert_eq!(ts_workflow.workflow_path, temp_dir.path());
    }

    #[test]
    fn test_typescript_workflow_index_ts() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("index.ts"), "export default {};").unwrap();

        let url = format!("file://{}", temp_dir.path().display());
        let ts_workflow = TypeScriptWorkflow::new(&url).unwrap();

        assert_eq!(ts_workflow.entry_file, "index.ts");
    }

    #[test]
    fn test_typescript_workflow_missing_file() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let url = format!("file://{}", temp_dir.path().display());
        let result = TypeScriptWorkflow::new(&url);

        assert!(result.is_err());
    }
}
