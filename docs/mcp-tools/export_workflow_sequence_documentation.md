# Export Workflow Sequence Tool Documentation

## Overview

The `export_workflow_sequence` tool is a powerful file editor designed specifically for creating and modifying workflow files. It operates similar to the Unix `sed` command, providing find/replace functionality and content appending capabilities. This tool is essential for programmatically managing workflow files in the Terminator MCP agent.

## Tool Definition

```rust
#[tool(
    description = "Edits workflow files using simple text find/replace operations. Works like sed - finds text patterns and replaces them, or appends content if no pattern specified."
)]
pub async fn export_workflow_sequence(
    &self,
    Parameters(args): Parameters<ExportWorkflowSequenceArgs>,
) -> Result<CallToolResult, McpError>
```

## Parameters Structure

The tool accepts parameters defined by the `ExportWorkflowSequenceArgs` struct:

```rust
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ExportWorkflowSequenceArgs {
    #[schemars(description = "Absolute path to the workflow file to create or edit")]
    pub file_path: String,

    #[schemars(description = "Text content to add to the workflow file")]
    pub content: String,

    #[schemars(
        description = "Text pattern to find and replace (optional - if not provided, content will be appended)"
    )]
    pub find_pattern: Option<String>,

    #[schemars(description = "Whether to use regex for pattern matching (default: false)")]
    pub use_regex: Option<bool>,

    #[schemars(description = "Create new file if it doesn't exist (default: true)")]
    pub create_if_missing: Option<bool>,
}
```

### Parameter Details

- **`file_path`** (Required): Absolute path to the workflow file you want to create or edit
- **`content`** (Required): The text content to add or use as replacement text
- **`find_pattern`** (Optional): Text pattern to search for and replace. If not provided, content will be appended
- **`use_regex`** (Optional): Whether to treat `find_pattern` as a regular expression (default: false)
- **`create_if_missing`** (Optional): Whether to create a new file if it doesn't exist (default: true)

## Core Implementation

### Initial Setup and File Reading

```rust
let use_regex = args.use_regex.unwrap_or(false);
let create_if_missing = args.create_if_missing.unwrap_or(true);

// Read existing file or start with empty content
let current_content = match std::fs::read_to_string(&args.file_path) {
    Ok(content) => content,
    Err(_) => {
        if create_if_missing {
            String::new()
        } else {
            return Err(McpError::invalid_params(
                "File does not exist and create_if_missing is false",
                Some(json!({"file_path": args.file_path})),
            ));
        }
    }
};
```

### Content Processing Logic

The tool operates in two distinct modes based on whether a `find_pattern` is provided:

```rust
let new_content = if let Some(find_pattern) = &args.find_pattern {
    // Replace mode - find and replace pattern with content
    if use_regex {
        // Use regex replacement
        match regex::Regex::new(find_pattern) {
            Ok(re) => {
                let result = re.replace_all(&current_content, args.content.as_str());
                if result == current_content {
                    return Err(McpError::invalid_params(
                        "Pattern not found in file",
                        Some(json!({"pattern": find_pattern, "file": args.file_path})),
                    ));
                }
                result.to_string()
            }
            Err(e) => {
                return Err(McpError::invalid_params(
                    "Invalid regex pattern",
                    Some(json!({"pattern": find_pattern, "error": e.to_string()})),
                ));
            }
        }
    } else {
        // Simple string replacement
        if !current_content.contains(find_pattern) {
            return Err(McpError::invalid_params(
                "Pattern not found in file",
                Some(json!({"pattern": find_pattern, "file": args.file_path})),
            ));
        }
        current_content.replace(find_pattern, &args.content)
    }
} else {
    // Append mode - add content to end of file
    if current_content.is_empty() {
        args.content
    } else if current_content.ends_with('\n') {
        format!("{}{}", current_content, args.content)
    } else {
        format!("{}\n{}", current_content, args.content)
    }
};
```

### File Writing and Response

```rust
// Write back to file
std::fs::write(&args.file_path, &new_content).map_err(|e| {
    McpError::internal_error(
        "Failed to write file",
        Some(json!({"error": e.to_string(), "path": args.file_path})),
    )
})?;

// Return success
Ok(CallToolResult::success(vec![Content::json(json!({
    "action": "edit_workflow_file",
    "status": "success",
    "file_path": args.file_path,
    "operation": if args.find_pattern.is_some() { "replace" } else { "append" },
    "pattern_type": if use_regex { "regex" } else { "string" },
    "file_size": new_content.len(),
    "timestamp": chrono::Utc::now().to_rfc3339()
}))?]))
```

## Operation Modes

### 1. Append Mode (No `find_pattern` provided)

When no `find_pattern` is specified, the tool operates in append mode:

```rust
// Append mode - add content to end of file
if current_content.is_empty() {
    args.content
} else if current_content.ends_with('\n') {
    format!("{}{}", current_content, args.content)
} else {
    format!("{}\n{}", current_content, args.content)
}
```

**Behavior**:

- If file is empty: Use content as-is
- If file ends with newline: Append content directly
- If file doesn't end with newline: Add newline before content

### 2. Replace Mode (String Replacement)

When `find_pattern` is provided and `use_regex` is false:

```rust
// Simple string replacement
if !current_content.contains(find_pattern) {
    return Err(McpError::invalid_params(
        "Pattern not found in file",
        Some(json!({"pattern": find_pattern, "file": args.file_path})),
    ));
}
current_content.replace(find_pattern, &args.content)
```

**Behavior**:

- Performs exact string matching (case-sensitive)
- Replaces ALL occurrences of the pattern
- Fails if pattern is not found

### 3. Replace Mode (Regex Replacement)

When `find_pattern` is provided and `use_regex` is true:

```rust
// Use regex replacement
match regex::Regex::new(find_pattern) {
    Ok(re) => {
        let result = re.replace_all(&current_content, args.content.as_str());
        if result == current_content {
            return Err(McpError::invalid_params(
                "Pattern not found in file",
                Some(json!({"pattern": find_pattern, "file": args.file_path})),
            ));
        }
        result.to_string()
    }
    Err(e) => {
        return Err(McpError::invalid_params(
            "Invalid regex pattern",
            Some(json!({"pattern": find_pattern, "error": e.to_string()})),
        ));
    }
}
```

**Behavior**:

- Uses full regex pattern matching
- Supports capture groups and advanced regex features
- Validates regex pattern before execution
- Fails if pattern doesn't match anything

## Usage Examples

### 1. Create New Workflow File

```json
{
  "tool_name": "export_workflow_sequence",
  "arguments": {
    "file_path": "C:/workflows/new_automation.yaml",
    "content": "---\ntool_name: execute_sequence\narguments:\n  steps:\n    - tool_name: open_application\n      arguments:\n        app_name: \"notepad\""
  }
}
```

### 2. Append Steps to Existing Workflow

```json
{
  "tool_name": "export_workflow_sequence",
  "arguments": {
    "file_path": "C:/workflows/existing_workflow.yaml",
    "content": "    - tool_name: delay\n      arguments:\n        delay_ms: 2000"
  }
}
```

### 3. Replace Specific Text (String Mode)

```json
{
  "tool_name": "export_workflow_sequence",
  "arguments": {
    "file_path": "C:/workflows/web_automation.yaml",
    "content": "https://new-website.com",
    "find_pattern": "https://old-website.com"
  }
}
```

### 4. Replace Using Regex Pattern

```json
{
  "tool_name": "export_workflow_sequence",
  "arguments": {
    "file_path": "C:/workflows/dynamic_workflow.yaml",
    "content": "timeout_ms: 10000",
    "find_pattern": "timeout_ms:\\s*\\d+",
    "use_regex": true
  }
}
```

### 5. Prevent File Creation

```json
{
  "tool_name": "export_workflow_sequence",
  "arguments": {
    "file_path": "C:/workflows/must_exist.yaml",
    "content": "new content",
    "create_if_missing": false
  }
}
```

## Response Format

### Successful Operation Response

```json
{
  "action": "edit_workflow_file",
  "status": "success",
  "file_path": "/path/to/workflow.yaml",
  "operation": "replace", // or "append"
  "pattern_type": "regex", // or "string"
  "file_size": 2048,
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### Response Fields

- **`action`**: Always "edit_workflow_file"
- **`status`**: Always "success" for successful operations
- **`file_path`**: The path to the file that was modified
- **`operation`**: Either "replace" or "append" depending on mode
- **`pattern_type`**: Either "regex" or "string" for replace operations
- **`file_size`**: Size of the final file in bytes
- **`timestamp`**: ISO 8601 timestamp of the operation

## Error Handling

### File Creation Error

When `create_if_missing` is false and file doesn't exist:

```rust
return Err(McpError::invalid_params(
    "File does not exist and create_if_missing is false",
    Some(json!({"file_path": args.file_path})),
));
```

### Pattern Not Found Error

When find pattern doesn't match anything in the file:

```rust
return Err(McpError::invalid_params(
    "Pattern not found in file",
    Some(json!({"pattern": find_pattern, "file": args.file_path})),
));
```

### Invalid Regex Error

When regex pattern is malformed:

```rust
return Err(McpError::invalid_params(
    "Invalid regex pattern",
    Some(json!({"pattern": find_pattern, "error": e.to_string()})),
));
```

### File Write Error

When file cannot be written to disk:

```rust
std::fs::write(&args.file_path, &new_content).map_err(|e| {
    McpError::internal_error(
        "Failed to write file",
        Some(json!({"error": e.to_string(), "path": args.file_path})),
    )
})?;
```

## Advanced Features

### Regex Capture Groups

When using regex mode, you can use capture groups in replacements:

```json
{
  "find_pattern": "timeout_ms: (\\d+)",
  "content": "timeout_ms: $1000", // Append "000" to existing number
  "use_regex": true
}
```

### Multi-line Patterns

Regex mode supports multi-line patterns:

```json
{
  "find_pattern": "steps:\\s*\\[\\s*\\]",
  "content": "steps:\n  - tool_name: delay\n    arguments:\n      delay_ms: 1000",
  "use_regex": true
}
```

### Global Replacements

String mode replaces ALL occurrences:

```json
{
  "find_pattern": "old_selector",
  "content": "new_selector"
}
```

## Best Practices

### 1. File Path Management

- Always use absolute paths for reliability
- Ensure proper file permissions for writing
- Use forward slashes or escape backslashes in paths

### 2. Content Formatting

- Include proper YAML indentation when appending
- End content with newlines when appropriate
- Validate YAML syntax after modifications

### 3. Pattern Matching

- Test regex patterns before use in production
- Use specific patterns to avoid unintended replacements
- Consider case sensitivity in string matching

### 4. Error Handling

- Always check for pattern existence before replacement
- Handle file permission errors gracefully
- Validate file paths before operations

### 5. Workflow Integration

- Use `import_workflow_sequence` to validate after export
- Combine with `execute_sequence` for testing
- Maintain backup copies of critical workflows

## Common Use Cases

### 1. Dynamic Configuration Updates

```yaml
# Update timeout values across multiple workflows
find_pattern: "timeout_ms: \\d+"
content: "timeout_ms: 5000"
use_regex: true
```

### 2. Environment-Specific Modifications

```yaml
# Change URLs for different environments
find_pattern: "https://staging.example.com"
content: "https://production.example.com"
```

### 3. Selector Updates

```yaml
# Update UI selectors when application changes
find_pattern: "role:Button\\|name:OldButton"
content: "role:Button|name:NewButton"
use_regex: true
```

### 4. Adding Monitoring Steps

```yaml
# Append logging/monitoring to existing workflows
content: |
  - tool_name: get_focused_window_tree
    arguments:
      random_string: "checkpoint_1"
```

## Dependencies

The implementation relies on these Rust crates:

- `regex` for regular expression support
- `serde_json` for JSON serialization
- `chrono` for timestamp generation
- `std::fs` for file system operations
- Custom `McpError` and `CallToolResult` types for error handling and responses

## Security Considerations

- File paths are not sandboxed - ensure proper access controls
- Regex patterns can be computationally expensive - validate complexity
- File write operations require appropriate permissions
- Backup critical files before performing bulk replacements
