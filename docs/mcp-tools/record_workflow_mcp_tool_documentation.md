# MCP Record Workflow Tool Documentation

## Quick Start

### 1. Start Recording

```javascript
await mcp.callTool("record_workflow", {
  action: "start",
  workflow_name: "My Workflow",
  highlight_mode: {
    // Optional: Visual feedback during recording
    enabled: true, // Red borders + event labels on UI elements
    duration_ms: 500, // Highlight duration per event
  },
});
```

### 2. Perform Actions

- Click buttons, links, UI elements
- Type text into fields
- Switch between applications
- Navigate browser tabs

**With highlighting enabled:** Look for red borders and event labels ("CLICK", "TYPE", etc.) confirming each action is captured.

### 3. Stop & Get Results

```javascript
const result = await mcp.callTool("record_workflow", {
  action: "stop",
});
```

## Understanding the Output

### What Gets Converted to MCP

✅ **High-level semantic events** → MCP tool sequences:

- **Button clicks** → `click_element` (clicks on buttons, links, menu items, tabs)
- **Text input completion** → `type_into_element` (typing + focus loss via Tab/Enter/click elsewhere)
- **Application switches** → `activate_element` (Alt+Tab, taskbar clicks, window focus changes)
- **Browser navigation** → `navigate_browser` (URL changes, new tabs, page navigation)

❌ **Raw hardware events** → Not converted (by design):

- Individual mouse moves/down/up
- Individual key presses
- These are used internally for event aggregation

### Event Completion Triggers

**Text Input Completion** occurs when:

- User types text AND moves focus away (Tab, Enter, click elsewhere)
- Autocomplete/dropdown selection is made
- Form field loses focus after content change

**Button Click Detection** requires:

- Click on interactive UI elements (buttons, links, tabs, menu items)
- Clicks on containers/empty space are ignored

### Response Format

#### Successful Start

```javascript
{
  action: "record_workflow",
  status: "started",
  workflow_name: "My Workflow",
  highlighting_enabled: true,        // Visual feedback is active
  highlight_duration_ms: 500,        // Duration of highlights
  message: "Recording started. Perform the UI actions you want to record."
}
```

#### Successful Recording Stop

```javascript
{
  action: "record_workflow",
  status: "stopped",
  workflow_name: "My Workflow",
  file_path: "/path/to/workflow.json", // Optional, if file saved

  // Ready-to-execute MCP sequence (null if no events converted)
  mcp_workflow: {
    tool_name: "execute_sequence",
    arguments: {
      items: [
        {
          tool_name: "click_element",
          arguments: {
            selector: "role:Button|name:Submit",
            timeout_ms: 3000
          },
          delay_ms: 200
        }
      ]
    },
    total_steps: 1,
    conversion_notes: ["Converted 1 click event", "Ignored 15 raw mouse events"],
    confidence_score: 0.85 // Optional quality metric
  },

  // Raw event data (for debugging)
  file_content: "{\"events\": [...] }" // JSON string of all captured events
}
```

#### Empty Recording (No Convertible Events)

```javascript
{
  action: "record_workflow",
  status: "stopped",
  workflow_name: "My Workflow",
  file_path: "/path/to/workflow.json",
  mcp_workflow: null, // No convertible events found
  file_content: "{\"events\": [...] }" // Still contains raw events for analysis
}
```

#### Error Response

```javascript
{
  action: "record_workflow",
  status: "error",
  error: "Recording failed: Could not initialize recorder",
  workflow_name: "My Workflow" // If provided in request
}
```

## Using the Output

### Execute Immediately

```javascript
// Always check for successful conversion
if (result.status === "stopped" && result.mcp_workflow) {
  await mcp.callTool("execute_sequence", result.mcp_workflow.arguments);
} else if (result.mcp_workflow === null) {
  console.log(
    "No convertible events recorded - only raw mouse/keyboard detected"
  );
} else if (result.status === "error") {
  console.error("Recording failed:", result.error);
}
```

### Save for Later

```javascript
// Save the workflow
const workflow = result.mcp_workflow;
localStorage.setItem("myWorkflow", JSON.stringify(workflow));

// Execute later
const savedWorkflow = JSON.parse(localStorage.getItem("myWorkflow"));
await mcp.callTool("execute_sequence", savedWorkflow.arguments);
```

## Best Practices

### ✅ Do This

- Use descriptive workflow names
- Perform deliberate, clear actions
- Wait for UI to respond between actions
- Test workflows on same application/website

### ❌ Avoid This

- Recording during loading states
- Very rapid mouse movements
- Recording system notifications
- Recording while other automations run

## Selectors Generated

### Chrome/Browser (Optimized)

```json
{
  "selector": "role:Pane|name:contains:Website Title >> role:Button|name:Submit"
}
```

### Desktop Applications

```json
{ "selector": "role:Window|name:contains:App Name >> role:Button|name:Submit" }
```

## Troubleshooting

### No MCP Sequences Generated (`mcp_workflow: null`)

**Common causes:**

- Only mouse movements recorded (no actual clicks on UI elements)
- Clicks on empty space, containers, or non-interactive elements
- Text typing without focus loss (no Tab/Enter/click to complete input)
- Only raw keyboard events (individual key presses vs semantic actions)

**Solutions:**

- Click actual buttons, links, or interactive elements
- Complete text input by moving focus away (Tab, Enter, click elsewhere)
- Use deliberate actions that create semantic meaning

### Execution Fails

**Element not found errors:**

- UI elements changed since recording
- Target application in different state
- Window size or layout changed

**Timeout errors:**

- Application not focused during execution
- Elements loading slower than expected
- Network delays for web applications

**Solutions:**

- Ensure target application matches recording state
- Verify application has focus before execution
- Test workflows immediately after recording

### Debug Mode

**Enable verbose logging:**

```javascript
// Add debug flag to see detailed conversion info
await mcp.callTool("record_workflow", {
  action: "start",
  workflow_name: "Debug Session",
  debug: true, // Shows raw events and conversion details
});
```

**Analyze raw events:**

```javascript
// Check file_content for raw event data
const rawEvents = JSON.parse(result.file_content);
console.log("Raw events captured:", rawEvents.events.length);
console.log("Conversion notes:", result.mcp_workflow?.conversion_notes);
```

## Parameters

| Parameter         | Required      | Description                                      |
| ----------------- | ------------- | ------------------------------------------------ |
| `action`          | ✅            | `"start"` or `"stop"`                            |
| `workflow_name`   | When starting | Descriptive name for the workflow                |
| `file_path`       | ❌            | Custom save location (auto-generated if omitted) |
| `highlight_mode`  | ❌            | Visual feedback config (see below)               |
| `low_energy_mode` | ❌            | Reduce system load on less powerful machines     |
| `debug`           | ❌            | Enable verbose logging for troubleshooting       |

### Highlight Mode Options

```javascript
highlight_mode: {
  enabled: true,              // Enable visual highlighting (default: true)
  duration_ms: 500,           // Highlight duration in ms (default: 500)
  color: 0x0000FF,           // Border color in BGR format (default: red)
  show_labels: true,         // Show event type labels (default: true)
  label_position: "Top",     // Label position: Top, Inside, Bottom, etc.
  label_style: {
    size: 14,               // Font size in pixels
    bold: true,             // Bold text
    color: 0xFFFFFF        // Text color in BGR (default: white)
  }
}
```

All properties are optional. Minimal config: `{ enabled: true }`

## Complete Response Properties

| Property                | Type    | Description                                      |
| ----------------------- | ------- | ------------------------------------------------ |
| `action`                | string  | Always `"record_workflow"`                       |
| `status`                | string  | `"started"`, `"stopped"`, or `"error"`           |
| `workflow_name`         | string  | Name provided in start request                   |
| `highlighting_enabled`  | boolean | Whether visual feedback is active (on start)     |
| `highlight_duration_ms` | number  | Duration of highlights in ms (on start)          |
| `file_path`             | string  | Path where workflow was saved (on stop)          |
| `mcp_workflow`          | object  | MCP sequence (null if no convertible events)     |
| `file_content`          | string  | Raw JSON of all captured events (on stop)        |
| `error`                 | string  | Error message (only present when status="error") |

### MCP Workflow Object Properties

| Property           | Type   | Description                                 |
| ------------------ | ------ | ------------------------------------------- |
| `tool_name`        | string | Always `"execute_sequence"`                 |
| `arguments`        | object | Arguments for execute_sequence tool         |
| `total_steps`      | number | Number of MCP tool calls in sequence        |
| `conversion_notes` | array  | Details about conversion process (optional) |
| `confidence_score` | number | Quality metric 0.0-1.0 (optional)           |
