# Simple Parallel Execution

A much easier way to run tools in parallel! 🚀

## How It Works

Just add two simple things:

1. **Enable parallel mode**: `"parallel_execution": true` in your sequence
2. **Mark parallel tools**: `"parallel": true` on tools that can run together

That's it! The system automatically groups consecutive parallel tools and runs them simultaneously.

## Basic Example

```json
{
  "tool_name": "execute_sequence",
  "arguments": {
    "parallel_execution": true,
    "steps": [
      {
        "tool_name": "get_applications",
        "arguments": {}
      },
      {
        "tool_name": "get_focused_window_tree", 
        "arguments": {},
        "parallel": true
      },
      {
        "tool_name": "run_command",
        "arguments": {"command": "ps aux"},
        "parallel": true  
      },
      {
        "tool_name": "click_element",
        "arguments": {"selector": "button", "pid": 123}
      }
    ]
  }
}
```

**Execution Order:**
1. `get_applications` runs first (sequential)
2. `get_focused_window_tree` and `run_command` run **at the same time** (parallel)
3. `click_element` runs last (sequential)

## When to Use Parallel

### ✅ Good for Parallel
- Reading data: `get_applications`, `get_window_tree`, `run_command`
- Independent operations that don't affect each other
- Data gathering from multiple sources

### ❌ Avoid Parallel  
- UI interactions: clicking, typing, pressing keys
- Operations that depend on previous results
- Anything that changes the UI state

## Simple Rules

1. **Default behavior**: Tools run one after another (sequential)
2. **Add `"parallel": true`**: Tool can run with other parallel tools
3. **Consecutive parallel tools**: Run together as a group
4. **Sequential tools**: Break up parallel groups

## Real Example

```json
{
  "tool_name": "execute_sequence", 
  "arguments": {
    "parallel_execution": true,
    "steps": [
      {
        "tool_name": "get_applications",
        "parallel": true
      },
      {
        "tool_name": "get_focused_window_tree",
        "parallel": true  
      },
      {
        "tool_name": "run_command",
        "arguments": {"command": "date"},
        "parallel": true
      },
      {
        "tool_name": "delay",
        "arguments": {"delay_ms": 500}
      },
      {
        "tool_name": "click_element",
        "arguments": {"selector": "button[text='OK']", "pid": 1234}
      }
    ]
  }
}
```

This will:
1. Run the first 3 tools **simultaneously** (parallel group)  
2. Wait 500ms (sequential)
3. Click the button (sequential)

## Performance Benefits

- **Faster execution**: Independent tools run simultaneously
- **Better resource usage**: No waiting for unrelated operations
- **Simple to use**: Just add `"parallel": true` where it makes sense

## Migration from Sequential

Change this:
```json
{
  "steps": [
    {"tool_name": "get_applications"},
    {"tool_name": "get_window_tree"}, 
    {"tool_name": "run_command"}
  ]
}
```

To this:
```json
{
  "parallel_execution": true,
  "steps": [
    {"tool_name": "get_applications", "parallel": true},
    {"tool_name": "get_window_tree", "parallel": true},
    {"tool_name": "run_command", "parallel": true}
  ]
}
```

Result: All three tools run **at the same time** instead of one by one!

## Error Handling

- Parallel tools that fail don't stop other parallel tools
- `"stop_on_error": true` still works - stops the whole sequence on any error
- `"continue_on_error": true` on individual tools still works

That's it! Much simpler than the previous complex version.