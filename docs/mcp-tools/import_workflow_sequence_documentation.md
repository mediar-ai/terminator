# Import Workflow Sequence Tool Documentation

## Overview

The `import_workflow_sequence` tool is designed to load YAML workflow files for the Terminator MCP agent. It can either load a single specific YAML workflow file or scan an entire folder to discover all YAML workflow files within it.

## Tool Definition

```rust
#[tool(description = "Load a YAML workflow file or scan folder for YAML workflow files")]
pub async fn import_workflow_sequence(
    &self,
    Parameters(args): Parameters<ImportWorkflowSequenceArgs>,
) -> Result<CallToolResult, McpError>
```

## Parameters Structure

The tool accepts parameters defined by the `ImportWorkflowSequenceArgs` struct:

```rust
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ImportWorkflowSequenceArgs {
    #[schemars(description = "Path to specific YAML workflow file to load")]
    pub file_path: Option<String>,

    #[schemars(description = "Path to folder to scan for YAML workflow files")]
    pub folder_path: Option<String>,
}
```

### Parameter Details

- **`file_path`** (Optional): Absolute path to a specific YAML workflow file you want to load
- **`folder_path`** (Optional): Path to a directory to scan for all YAML workflow files

**Important**: You must provide either `file_path` OR `folder_path`, but not both.

## Core Implementation

The tool uses pattern matching to handle different parameter combinations:

```rust
match (args.file_path, args.folder_path) {
    // Load single file
    (Some(file_path), None) => {
        let content = std::fs::read_to_string(&file_path).map_err(|e| {
            McpError::invalid_params(
                "Failed to read file",
                Some(json!({"error": e.to_string(), "path": file_path})),
            )
        })?;

        let workflow: serde_json::Value = serde_yaml::from_str(&content).map_err(|e| {
            McpError::invalid_params(
                "Invalid YAML format",
                Some(json!({"error": e.to_string()})),
            )
        })?;

        Ok(CallToolResult::success(vec![Content::json(json!({
            "operation": "load_file",
            "file_path": file_path,
            "workflow": workflow
        }))?]))
    }
    // Scan folder
    (None, Some(folder_path)) => {
        let files = scan_yaml_files(&folder_path)?;

        Ok(CallToolResult::success(vec![Content::json(json!({
            "operation": "scan_folder",
            "folder_path": folder_path,
            "files": files,
            "count": files.len()
        }))?]))
    }
    // Error cases
    (Some(_), Some(_)) => Err(McpError::invalid_params(
        "Provide either file_path OR folder_path, not both",
        None,
    )),
    (None, None) => Err(McpError::invalid_params(
        "Must provide either file_path or folder_path",
        None,
    )),
}
```

## Folder Scanning Helper Function

When scanning folders, the tool uses the `scan_yaml_files` helper function:

```rust
fn scan_yaml_files(folder_path: &str) -> Result<Vec<serde_json::Value>, McpError> {
    let mut files = Vec::new();

    let dir = std::fs::read_dir(folder_path).map_err(|e| {
        McpError::invalid_params(
            "Failed to read directory",
            Some(json!({"error": e.to_string(), "path": folder_path})),
        )
    })?;

    for entry in dir {
        let entry = entry.map_err(|e| {
            McpError::internal_error(
                "Directory entry error",
                Some(json!({"error": e.to_string()})),
            )
        })?;

        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext == "yaml" || ext == "yml" {
                    let metadata = entry.metadata().ok();
                    let file_name = path
                        .file_stem()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    files.push(json!({
                        "name": file_name,
                        "file_path": path.to_string_lossy(),
                        "size": metadata.as_ref().map(|m| m.len()).unwrap_or(0),
                        "modified": metadata.and_then(|m| m.modified().ok())
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs())
                    }));
                }
            }
        }
    }

    Ok(files)
}
```

## Usage Examples

### 1. Loading a Single Workflow File

```json
{
  "tool_name": "import_workflow_sequence",
  "arguments": {
    "file_path": "C:/workflows/calculator_automation.yaml"
  }
}
```

### 2. Scanning a Folder for All YAML Files

```json
{
  "tool_name": "import_workflow_sequence",
  "arguments": {
    "folder_path": "C:/workflows"
  }
}
```

## Response Formats

### Single File Load Response

When loading a single file, the response includes the parsed workflow content:

```json
{
  "operation": "load_file",
  "file_path": "/path/to/workflow.yaml",
  "workflow": {
    // Complete parsed YAML content as JSON
    "tool_name": "execute_sequence",
    "arguments": {
      "steps": [...],
      "variables": {...},
      // ... rest of workflow definition
    }
  }
}
```

### Folder Scan Response

When scanning a folder, the response includes metadata for all discovered YAML files:

```json
{
  "operation": "scan_folder",
  "folder_path": "/path/to/folder",
  "files": [
    {
      "name": "workflow1",
      "file_path": "/path/to/folder/workflow1.yaml",
      "size": 1024,
      "modified": 1672531200
    },
    {
      "name": "automation_script",
      "file_path": "/path/to/folder/automation_script.yml",
      "size": 2048,
      "modified": 1672617600
    }
  ],
  "count": 2
}
```

## Error Handling

The tool handles several error scenarios with specific error messages:

### Parameter Validation Errors

```rust
// Both parameters provided
(Some(_), Some(_)) => Err(McpError::invalid_params(
    "Provide either file_path OR folder_path, not both",
    None,
))

// No parameters provided
(None, None) => Err(McpError::invalid_params(
    "Must provide either file_path or folder_path",
    None,
))
```

### File Reading Errors

```rust
let content = std::fs::read_to_string(&file_path).map_err(|e| {
    McpError::invalid_params(
        "Failed to read file",
        Some(json!({"error": e.to_string(), "path": file_path})),
    )
})?;
```

### YAML Parsing Errors

```rust
let workflow: serde_json::Value = serde_yaml::from_str(&content).map_err(|e| {
    McpError::invalid_params(
        "Invalid YAML format",
        Some(json!({"error": e.to_string()})),
    )
})?;
```

### Directory Access Errors

```rust
let dir = std::fs::read_dir(folder_path).map_err(|e| {
    McpError::invalid_params(
        "Failed to read directory",
        Some(json!({"error": e.to_string(), "path": folder_path})),
    )
})?;
```

## File Type Detection

The tool specifically looks for files with `.yaml` or `.yml` extensions:

```rust
if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
    if ext == "yaml" || ext == "yml" {
        // Process this file
    }
}
```

## Supported YAML Workflow Structure

The tool can import YAML files that follow the workflow structure expected by the `execute_sequence` tool. Here's an example of a compatible YAML structure:

```yaml
---
tool_name: execute_sequence
arguments:
  variables:
    username:
      type: string
      label: Username
      default: "user"

  inputs:
    username: "testuser"

  selectors:
    login_button: "role:Button|name:Login"
    username_field: "role:Edit|name:Username"

  steps:
    - tool_name: open_application
      arguments:
        app_name: "notepad"
      continue_on_error: false

    - tool_name: type_into_element
      arguments:
        selector: "${{ selectors.username_field }}"
        text_to_type: "${{ inputs.username }}"

  stop_on_error: true
```

## Integration with Execute Sequence

After importing a workflow, you can execute it using the `execute_sequence` tool by passing the loaded workflow data:

1. **Import**: Use `import_workflow_sequence` to load and validate the YAML
2. **Execute**: Use `execute_sequence` with the loaded workflow data
3. **Monitor**: Track execution results and handle any errors

## Best Practices

1. **File Paths**: Use absolute paths for better reliability
2. **YAML Validation**: The tool validates YAML syntax during import
3. **Error Handling**: Check the response for any parsing or file access errors
4. **Folder Organization**: Keep related workflows in dedicated folders for easier management
5. **File Naming**: Use descriptive names with `.yaml` or `.yml` extensions

## Dependencies

The implementation relies on these Rust crates:

- `serde_yaml` for YAML parsing
- `serde_json` for JSON serialization
- `std::fs` for file system operations
- Custom `McpError` and `CallToolResult` types for error handling and responses
