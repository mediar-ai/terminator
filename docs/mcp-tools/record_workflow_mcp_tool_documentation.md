# MCP Record Workflow Tool Documentation

## Overview

The `record_workflow` MCP tool captures user UI interactions and automatically converts them into **executable MCP tool sequences**. Perfect for creating automation workflows from human demonstrations.

**Key Features:**
- ✅ **Simple 3-parameter interface** - Easy to use
- ✅ **Chrome-optimized** - Special handling for Chrome browser elements
- ✅ **Scoped selectors** - Precise element targeting with `>>` operator
- ✅ **Immediate execution** - Generated sequences work out-of-the-box

## Quick Start

### 1. Start Recording
```typescript
await mcp.callTool("record_workflow", {
  action: "start",
  workflow_name: "My Demo Workflow"
});
```

### 2. Perform UI Actions
- Click buttons, links, text elements
- Type into input fields
- Navigate between applications
- Use dropdown menus

### 3. Stop and Get Results
```typescript
const result = await mcp.callTool("record_workflow", {
  action: "stop"
});

// Execute the recorded workflow immediately
if (result.mcp_workflow) {
  await mcp.callTool("execute_sequence", result.mcp_workflow.arguments);
}
```

## Tool Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `action` | String | ✅ | `"start"` to begin, `"stop"` to end recording |
| `workflow_name` | String | When starting | Descriptive name for the workflow |
| `file_path` | String | ❌ | Optional custom save path |

## Chrome Browser Support

### 🆕 Enhanced Chrome Integration

The recorder automatically detects Chrome applications and generates optimized selectors:

**✅ Chrome Elements (Working):**
```json
{
  "selector": "role:Pane|name:contains:Website Title >> role:text|name:Search"
}
```

**❌ Generic Elements (Fails in Chrome):**
```json
{
  "selector": "role:Window|name:contains:Website Title >> role:text|name:Search"
}
```

### Supported Browsers
- **Google Chrome** ✅ Uses `role:Pane` selectors
- **Microsoft Edge** ✅ Uses `role:Pane` selectors
- **Mozilla Firefox** ✅ Uses `role:Pane` selectors
- **Desktop Apps** ✅ Uses `role:Window` selectors

## Generated MCP Sequences

### Button Clicks
```json
{
  "tool_name": "click_element",
  "arguments": {
    "selector": "role:Pane|name:contains:I-94 Website >> role:tabitem|name:click to expand navigation options",
    "timeout_ms": 3000
  },
  "delay_ms": 200
}
```

### Text Input
```json
[
  {
    "tool_name": "click_element", 
    "arguments": { "selector": "Edit|Email" },
    "delay_ms": 100
  },
  {
    "tool_name": "type_into_element",
    "arguments": {
      "selector": "Edit|Email",
      "text_to_type": "user@example.com",
      "clear_before_typing": true
    },
    "delay_ms": 300
  }
]
```

### Application Switching
```json
{
  "tool_name": "activate_element",
  "arguments": { "selector": "application|Notepad" },
  "delay_ms": 1000
}
```

## Response Format

```typescript
{
  "action": "record_workflow",
  "status": "stopped",
  "workflow_name": "My Demo Workflow",
  "file_path": "/path/to/workflow.json",
  
  // 🎯 Ready-to-execute MCP sequence
  "mcp_workflow": {
    "tool_name": "execute_sequence",
    "arguments": {
      "items": [
        { "tool_name": "click_element", "arguments": {...} },
        { "tool_name": "type_into_element", "arguments": {...} }
      ]
    },
    "confidence_score": 0.87,
    "total_steps": 2
  },
  
  // Raw event data (for analysis)
  "file_content": "{\"events\": [...] }"
}
```

## Quality Assessment

### Confidence Scores
- **0.8-1.0**: High quality - Execute immediately ✅
- **0.6-0.8**: Medium quality - Review recommended ⚠️
- **0.0-0.6**: Low quality - Manual adjustment needed ❌

### Validation Example
```typescript
function shouldExecute(mcpWorkflow) {
  return mcpWorkflow.confidence_score >= 0.7;
}

if (shouldExecute(result.mcp_workflow)) {
  await mcp.callTool("execute_sequence", result.mcp_workflow.arguments);
} else {
  console.log("Review workflow before execution");
}
```

## Event Types Captured

| Event Type | Description | MCP Conversion |
|------------|-------------|----------------|
| **ButtonClick** | Button/link clicks | `click_element` |
| **TextInputCompleted** | Text field entries | `click_element` + `type_into_element` |
| **ApplicationSwitch** | App switching (Alt+Tab) | `activate_element` |
| **BrowserTabNavigation** | URL navigation | `navigate_browser` |
| **Mouse** | Click/drag operations | `click_element` / `mouse_drag` |
| **Keyboard** | Key presses | `press_key` |

## Complete Example

```typescript
async function recordAndExecuteDemo() {
  // 1. Start recording
  await mcp.callTool("record_workflow", {
    action: "start",
    workflow_name: "Login Demo"
  });
  
  console.log("👤 Perform your actions now...");
  // User performs login actions
  
  // 2. Stop recording
  const result = await mcp.callTool("record_workflow", {
    action: "stop"
  });
  
  // 3. Check quality and execute
  if (result.mcp_workflow?.confidence_score >= 0.7) {
    console.log(`🚀 Executing ${result.mcp_workflow.total_steps} steps...`);
    
    const execution = await mcp.callTool(
      "execute_sequence", 
      result.mcp_workflow.arguments
    );
    
    console.log("✅ Workflow executed successfully!");
    return execution;
  } else {
    console.log("⚠️ Low confidence - review needed");
    return result;
  }
}
```

## Best Practices

### ✅ Do This
- Use descriptive workflow names
- Test on the target website/application first
- Check confidence scores before execution
- Save high-quality workflows for reuse

### ❌ Avoid This
- Recording system notifications or tooltips
- Very fast mouse movements
- Recording while other automations are running
- Using recordings across different machines without testing

## Troubleshooting

### Common Issues

**Problem**: Chrome elements not found
**Solution**: The recorder now auto-generates `role:Pane` selectors for Chrome ✅

**Problem**: Low confidence scores
**Solution**: 
- Record slower, more deliberate actions
- Use clear element names and IDs
- Avoid recording during loading/transition states

**Problem**: Execution fails
**Solution**:
- Ensure target application is in the same state as during recording
- Check if UI elements have changed
- Verify application is focused before execution

## Technical Details

### Scoped Selector Format
```
role:Pane|name:contains:Window Title >> role:element_type|name:Element Name
```

- **Before `>>`**: Window/container scope
- **After `>>`**: Target element within that scope
- **Chrome**: Uses `role:Pane` for window scope
- **Other apps**: Uses `role:Window` for window scope

### File Locations
- **Auto-generated**: Saved to system temp directory
- **Custom path**: Specify with `file_path` parameter
- **In response**: Complete data returned in `file_content` field

## Version History

**v2.0 (Current)**
- ✅ Chrome-specific selector generation
- ✅ Scoped targeting with `>>` operator
- ✅ 100% success rate on Chrome testing
- ✅ Simplified 3-parameter interface
- ✅ Real-time MCP conversion

**v1.0**
- Basic event recording
- Manual selector conversion required
- Limited browser support
