/// TypeScript/JavaScript workflow detection and execution
use anyhow::{Context, Result};
use serde_json::Value;
use std::path::Path;

/// Check if the input is a TypeScript/JavaScript workflow
pub fn is_typescript_workflow(input: &str, is_file_input: bool) -> bool {
    use tracing::debug;

    debug!(
        "Checking TypeScript workflow: input={}, is_file_input={}",
        input, is_file_input
    );

    if !is_file_input {
        debug!("Not a file input, returning false");
        return false;
    }

    let path = Path::new(input);
    debug!(
        "Path exists: {}, is_dir: {}, is_file: {}",
        path.exists(),
        path.is_dir(),
        path.is_file()
    );

    if path.is_file() {
        // Check if it's a .ts/.js file
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext == "ts" || ext == "js")
            .unwrap_or(false)
    } else if path.is_dir() {
        // Check for package.json AND (terminator.ts OR src/terminator.ts OR workflow.ts OR index.ts)
        let package_json = path.join("package.json");
        let terminator_ts = path.join("terminator.ts");
        let src_terminator_ts = path.join("src").join("terminator.ts");
        let workflow_ts = path.join("workflow.ts");
        let index_ts = path.join("index.ts");

        debug!("Checking directory for TypeScript workflow files:");
        debug!("  package.json exists: {}", package_json.exists());
        debug!("  terminator.ts exists: {}", terminator_ts.exists());
        debug!("  src/terminator.ts exists: {}", src_terminator_ts.exists());
        debug!("  workflow.ts exists: {}", workflow_ts.exists());
        debug!("  index.ts exists: {}", index_ts.exists());

        let result = package_json.exists()
            && (terminator_ts.exists()
                || src_terminator_ts.exists()
                || workflow_ts.exists()
                || index_ts.exists());

        debug!("Is TypeScript workflow: {}", result);
        result
    } else {
        false
    }
}

/// Convert file path to file:// URL
pub fn path_to_file_url(input: &str) -> Result<String> {
    let abs_path = std::fs::canonicalize(input)
        .with_context(|| format!("Failed to resolve path: {}", input))?;

    // If it's a directory with TypeScript workflow files, point to the specific file
    let target_path = if abs_path.is_dir() {
        // Check for terminator.ts first (root), then src/terminator.ts, then workflow.ts, then index.ts
        if abs_path.join("terminator.ts").exists() {
            abs_path.join("terminator.ts")
        } else if abs_path.join("src").join("terminator.ts").exists() {
            abs_path.join("src").join("terminator.ts")
        } else if abs_path.join("workflow.ts").exists() {
            abs_path.join("workflow.ts")
        } else if abs_path.join("index.ts").exists() {
            abs_path.join("index.ts")
        } else {
            abs_path
        }
    } else {
        abs_path
    };

    // Strip Windows \\?\ prefix if present
    let path_str = target_path.display().to_string();
    let normalized_path = path_str.strip_prefix(r"\\?\").unwrap_or(&path_str);

    Ok(format!("file://{}", normalized_path))
}

/// Build execute_sequence arguments for TypeScript workflow
pub fn build_typescript_workflow_args(
    file_url: String,
    inputs: Option<&String>,
    start_from_step: Option<&String>,
    end_at_step: Option<&String>,
    follow_fallback: Option<bool>,
    execute_jumps_at_end: Option<bool>,
    no_stop_on_error: bool,
    no_detailed_results: bool,
) -> Result<Value> {
    let mut workflow_args = serde_json::Map::new();

    // Add URL
    workflow_args.insert("url".to_string(), Value::String(file_url));

    // Add execution options
    if no_stop_on_error {
        workflow_args.insert("stop_on_error".to_string(), Value::Bool(false));
    }

    workflow_args.insert(
        "include_detailed_results".to_string(),
        Value::Bool(!no_detailed_results),
    );

    // Add step control parameters
    if let Some(start_step) = start_from_step {
        workflow_args.insert(
            "start_from_step".to_string(),
            Value::String(start_step.clone()),
        );
    }

    if let Some(end_step) = end_at_step {
        workflow_args.insert("end_at_step".to_string(), Value::String(end_step.clone()));
    }

    if let Some(follow) = follow_fallback {
        workflow_args.insert("follow_fallback".to_string(), Value::Bool(follow));
    }

    if let Some(execute_jumps) = execute_jumps_at_end {
        workflow_args.insert(
            "execute_jumps_at_end".to_string(),
            Value::Bool(execute_jumps),
        );
    }

    // Add CLI inputs if provided
    if let Some(inputs_str) = inputs {
        let inputs_val = serde_json::from_str::<Value>(inputs_str)
            .with_context(|| "Invalid JSON in --inputs parameter")?;
        workflow_args.insert("inputs".to_string(), inputs_val);
    }

    Ok(Value::Object(workflow_args))
}
