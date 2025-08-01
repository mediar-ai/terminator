# Parallel Tool Execution in MCP Server

This document describes the new parallel execution capabilities added to the MCP (Model Context Protocol) server, allowing tools to run concurrently for improved performance.

## Overview

The parallel execution feature allows multiple tools to run simultaneously when they don't depend on each other, significantly reducing the total execution time for complex workflows. This is particularly useful for:

- Read-only operations that can run independently
- UI automation tasks that can be parallelized
- Data gathering operations across multiple sources
- Any workflow where order doesn't matter for some steps

## New Features

### 1. Tool-Level Parallelization Metadata

Each tool step can now specify parallel execution hints:

```json
{
  "tool_name": "get_applications",
  "arguments": {},
  "parallelizable": true,
  "parallel_group_id": "read_operations",
  "depends_on": ["previous_step_id"],
  "max_parallel": 2
}
```

**New Fields:**
- `parallelizable`: Boolean indicating if this tool can run in parallel with others
- `parallel_group_id`: String ID grouping tools that can run together
- `depends_on`: Array of step IDs that must complete before this tool runs
- `max_parallel`: Maximum number of tools in this group that can run simultaneously

### 2. Sequence-Level Execution Control

The `execute_sequence` tool now supports parallel execution configuration:

```json
{
  "tool_name": "execute_sequence",
  "arguments": {
    "enable_parallel_execution": true,
    "execution_strategy": "mixed",
    "global_max_parallel": 4,
    "steps": [...]
  }
}
```

**New Fields:**
- `enable_parallel_execution`: Boolean to enable parallel execution mode
- `execution_strategy`: Strategy for parallelization ("sequential", "parallel", "mixed")
- `global_max_parallel`: Maximum number of tools that can run simultaneously across all groups

### 3. Execution Strategies

#### Sequential (Default)
All tools run one after another, maintaining the original behavior.

```json
{
  "execution_strategy": "sequential"
}
```

#### Parallel
Automatically attempts to parallelize all tools marked as `parallelizable`.

```json
{
  "execution_strategy": "parallel"
}
```

#### Mixed (Recommended)
Uses `parallel_group_id` to group tools that can run together, keeping others sequential.

```json
{
  "execution_strategy": "mixed"
}
```

## Usage Examples

### Example 1: Parallel Data Gathering

```json
{
  "tool_name": "execute_sequence",
  "arguments": {
    "enable_parallel_execution": true,
    "execution_strategy": "mixed",
    "global_max_parallel": 3,
    "steps": [
      {
        "tool_name": "get_applications",
        "arguments": {},
        "id": "apps",
        "parallelizable": true,
        "parallel_group_id": "data_gathering"
      },
      {
        "tool_name": "get_focused_window_tree",
        "arguments": {},
        "id": "window",
        "parallelizable": true,
        "parallel_group_id": "data_gathering"
      },
      {
        "tool_name": "run_command",
        "arguments": {
          "command": "ps aux"
        },
        "id": "processes",
        "parallelizable": true,
        "parallel_group_id": "data_gathering"
      }
    ]
  }
}
```

In this example, all three tools will run simultaneously since they're in the same parallel group.

### Example 2: Sequential UI Operations with Parallel Setup

```json
{
  "tool_name": "execute_sequence",
  "arguments": {
    "enable_parallel_execution": true,
    "execution_strategy": "mixed",
    "global_max_parallel": 2,
    "steps": [
      {
        "tool_name": "get_applications",
        "arguments": {},
        "id": "get_apps",
        "parallelizable": true,
        "parallel_group_id": "setup"
      },
      {
        "tool_name": "get_focused_window_tree",
        "arguments": {},
        "id": "get_window",
        "parallelizable": true,
        "parallel_group_id": "setup"
      },
      {
        "tool_name": "click_element",
        "arguments": {
          "selector": "button[text='Submit']",
          "pid": 12345
        },
        "id": "click_submit",
        "depends_on": ["get_apps", "get_window"]
      },
      {
        "tool_name": "type_into_element",
        "arguments": {
          "text": "Hello",
          "selector": "input",
          "pid": 12345
        },
        "id": "type_text",
        "depends_on": ["click_submit"]
      }
    ]
  }
}
```

Here, the setup operations run in parallel first, then the UI operations run sequentially as they depend on the setup completing.

## Dependency Management

The system supports sophisticated dependency management:

### Simple Dependencies
```json
{
  "tool_name": "second_step",
  "depends_on": ["first_step"]
}
```

### Multiple Dependencies
```json
{
  "tool_name": "final_step",
  "depends_on": ["step_1", "step_2", "step_3"]
}
```

### Dependency Resolution
The execution engine automatically:
1. Builds a dependency graph from step IDs
2. Ensures dependent steps wait for their dependencies
3. Runs independent steps in parallel when possible
4. Maintains execution order for sequential steps

## Performance Considerations

### When to Use Parallel Execution

**Good candidates:**
- Read-only operations (get_applications, get_window_tree, etc.)
- Independent API calls or data fetching
- File operations that don't conflict
- Multiple UI queries that don't interfere

**Avoid parallelizing:**
- UI interactions that must happen in order (click then type)
- Operations that modify the same resource
- Steps where timing is critical
- Context-dependent operations

### Concurrency Limits

Set appropriate limits to avoid overwhelming the system:

```json
{
  "global_max_parallel": 4,        // System-wide limit
  "max_parallel": 2               // Per-group limit
}
```

## Error Handling

Parallel execution maintains the same error handling behavior:

- `stop_on_error`: When true, stops all execution on first error
- `continue_on_error`: Per-step error handling still applies
- Failed parallel tasks don't prevent other parallel tasks from completing

## Response Format

Parallel execution responses include additional metadata:

```json
{
  "action": "execute_sequence",
  "status": "success",
  "execution_mode": "parallel",
  "total_tools": 6,
  "executed_tools": 6,
  "total_duration_ms": 1250,
  "results": [...]
}
```

The `execution_mode` field indicates whether parallel execution was used.

## Migration Guide

### From Sequential to Parallel

1. **Identify Independent Operations**: Look for steps that don't depend on each other
2. **Add Parallelization Hints**: Mark independent steps with `parallelizable: true`
3. **Group Related Operations**: Use `parallel_group_id` to group steps that can run together
4. **Set Appropriate Limits**: Configure `global_max_parallel` based on your system
5. **Test Thoroughly**: Verify that parallel execution doesn't break your workflow

### Backward Compatibility

All existing workflows continue to work without changes. Parallel execution is opt-in through:
- `enable_parallel_execution: true`
- `execution_strategy: "parallel"` or `"mixed"`

## Best Practices

1. **Start Conservative**: Begin with low parallelism limits and increase gradually
2. **Group Logically**: Use meaningful `parallel_group_id` names that reflect the operation type
3. **Handle Dependencies**: Explicitly declare dependencies with `depends_on` when order matters
4. **Monitor Performance**: Compare execution times to verify parallel execution benefits
5. **Consider Context**: UI automation may need more careful parallelization than data gathering

## Limitations

- Context-dependent tools (like click operations) should be used carefully with parallelization
- Some tools may have internal state that prevents safe parallel execution
- System resources (CPU, memory) may limit effective parallelism
- Network-bound operations may not benefit from parallelization due to bandwidth limits

## Future Enhancements

Planned improvements include:
- Automatic dependency detection for common patterns
- Dynamic parallelism adjustment based on system load
- Better error reporting for parallel execution failures
- Tool-specific parallelization metadata in tool definitions