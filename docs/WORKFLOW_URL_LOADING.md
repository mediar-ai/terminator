# Loading Workflows from YAML Files

The `execute_sequence` tool supports loading workflow definitions from external YAML files using the `url` parameter. This allows you to store workflow definitions separately and reuse them across different executions.

## Basic Usage

Instead of providing `steps` directly in the tool call, use the `url` parameter to load the workflow from a file:

```json
{
  "tool_name": "execute_sequence",
  "arguments": {
    "url": "file://C:/workflows/my_workflow.yml"
  }
}
```

## Supported URL Schemes

- **Local files**: `file://absolute/path/to/workflow.yml`
- **HTTP/HTTPS**: `http://example.com/workflow.yml` or `https://example.com/workflow.yml`

## Workflow File Format

The YAML file should contain the workflow definition directly (without wrapping in `execute_sequence`):

```yaml
# my_workflow.yml
steps:
  - tool_name: navigate_browser
    arguments:
      url: "https://example.com"
  - tool_name: click_element
    arguments:
      selector: "role:Button|name:Submit"

stop_on_error: true
include_detailed_results: false
```

## Overriding Workflow Parameters

You can override workflow parameters when loading from a file:

```json
{
  "tool_name": "execute_sequence",
  "arguments": {
    "url": "file://C:/workflows/base_workflow.yml",
    "inputs": {
      "username": "alice",
      "api_key": "secret123"
    },
    "verbosity": "verbose",
    "stop_on_error": false
  }
}
```

Parameters specified in the tool call will override those in the workflow file.

## Using Variables in Workflows

Define variables in the workflow file that can be passed at runtime:

```yaml
# parameterized_workflow.yml
variables:
  url:
    type: string
    label: "Target URL"
    required: true
  username:
    type: string
    label: "Login Username"
    default: "testuser"
  max_retries:
    type: number
    label: "Maximum Retries"
    default: 3

steps:
  - tool_name: navigate_browser
    arguments:
      url: "{{url}}"
  - tool_name: type_into_element
    arguments:
      selector: "role:Edit|name:Username"
      text_to_type: "{{username}}"
```

Then invoke with inputs:

```json
{
  "tool_name": "execute_sequence",
  "arguments": {
    "url": "file://C:/workflows/parameterized_workflow.yml",
    "inputs": {
      "url": "https://myapp.com/login",
      "username": "alice"
    }
  }
}
```

## State Persistence with File URLs

When using `file://` URLs, workflow state is automatically persisted:

- State saved to `.workflow_state/<workflow_name>.json` in the workflow's directory
- Enables resuming workflows from specific steps
- Useful for debugging and partial execution

```json
{
  "tool_name": "execute_sequence",
  "arguments": {
    "url": "file://C:/workflows/complex_workflow.yml",
    "start_from_step": "step_5",
    "end_at_step": "step_10"
  }
}
```

## Complete Examples

### Example 1: Simple Workflow Loading

**Workflow file** (`examples/simple_test.yml`):
```yaml
steps:
  - tool_name: open_application
    arguments:
      app_name: "notepad"
  - tool_name: delay
    arguments:
      delay_ms: 1000
  - tool_name: get_focused_window_tree
    include_tree: true
```

**Tool call**:
```json
{
  "tool_name": "execute_sequence",
  "arguments": {
    "url": "file://C:/terminator/examples/simple_test.yml"
  }
}
```

### Example 2: Workflow with Input Variables

**Workflow file** (`examples/browser_automation.yml`):
```yaml
variables:
  target_url:
    type: string
    label: "URL to Navigate"
    required: true
  search_term:
    type: string
    label: "Search Term"
    default: ""

steps:
  - tool_name: navigate_browser
    arguments:
      url: "{{target_url}}"
  - tool_name: type_into_element
    if: "search_term != ''"
    arguments:
      selector: "role:Edit|name:Search"
      text_to_type: "{{search_term}}"
  - tool_name: click_element
    if: "search_term != ''"
    arguments:
      selector: "role:Button|name:Search"

stop_on_error: true
```

**Tool call**:
```json
{
  "tool_name": "execute_sequence",
  "arguments": {
    "url": "file://C:/terminator/examples/browser_automation.yml",
    "inputs": {
      "target_url": "https://google.com",
      "search_term": "terminator automation"
    }
  }
}
```

### Example 3: Remote Workflow Loading

```json
{
  "tool_name": "execute_sequence",
  "arguments": {
    "url": "https://example.com/workflows/data_extraction.yml",
    "inputs": {
      "api_key": "xyz123",
      "environment": "production"
    },
    "verbosity": "verbose"
  }
}
```

## CLI Usage

The Terminator CLI also supports URL loading:

```bash
# Local file
terminator mcp run workflow.yml

# With inputs
terminator mcp run workflow.yml --inputs '{"username":"alice","count":5}'

# From HTTP URL
terminator mcp run https://example.com/workflow.yml --verbose

# Partial execution
terminator mcp run workflow.yml --start-from "step_5" --end-at "step_10"
```

## Benefits of URL-Based Loading

1. **Separation of Concerns**: Keep workflow logic separate from execution parameters
2. **Reusability**: Share workflows across different projects and executions
3. **Version Control**: Store workflows in git repositories
4. **State Persistence**: Automatic state saving for `file://` URLs
5. **Remote Workflows**: Load workflows from HTTP endpoints for centralized management
6. **Parameter Override**: Easily customize workflows without modifying files

## Notes

- Workflow files are parsed as YAML by default
- The workflow directory is stored and used for resolving relative script paths
- State persistence only works for `file://` URLs
- Remote workflows (`http://`, `https://`) are fetched once at execution start
- Local overrides (inputs, selectors, etc.) take precedence over file definitions
