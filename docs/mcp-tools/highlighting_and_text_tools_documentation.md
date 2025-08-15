# Highlighting and Text Display Tools Documentation

## Overview

This document describes the available highlighting and text display tools in the Terminator MCP agent, including both current functionality and proposed new tools for enhanced visual feedback and text overlay capabilities.

## Current Available Tool

### `highlight_element`

Highlights an element with a colored border and optional text overlay for visual confirmation.

#### Parameters

| Parameter                     | Type    | Required | Default    | Description                                        |
| ----------------------------- | ------- | -------- | ---------- | -------------------------------------------------- |
| `selector`                    | string  | Yes      | -          | Element selector (can be chained with `>>`)        |
| `alternative_selectors`       | string  | No       | -          | Alternative selectors to try in parallel           |
| `fallback_selectors`          | string  | No       | -          | Fallback selectors to try if primary fails         |
| `color`                       | number  | No       | `0x00FF00` | BGR color code for highlight border (bright green) |
| `duration_ms`                 | number  | No       | `2000`     | Duration in milliseconds (2 seconds recommended)   |
| `text`                        | string  | No       | -          | Text to display as overlay (truncated to 10 chars) |
| `text_position`               | enum    | No       | -          | Position of text relative to element               |
| `font_style`                  | object  | No       | -          | Font styling options                               |
| `timeout_ms`                  | number  | No       | -          | Timeout for finding element                        |
| `include_tree`                | boolean | No       | `true`     | Include UI tree in response                        |
| `include_detailed_attributes` | boolean | No       | -          | Include detailed element attributes                |
| `retries`                     | number  | No       | -          | Number of retries for finding element              |

#### Text Position Options

```typescript
enum TextPosition {
  Top, // Above the element
  TopRight, // Top-right corner
  Right, // Right side of the element
  BottomRight, // Bottom-right corner
  Bottom, // Below the element
  BottomLeft, // Bottom-left corner
  Left, // Left side of the element
  TopLeft, // Top-left corner
  Inside, // Inside the element
}
```

#### Font Style Options

```typescript
interface FontStyle {
  size: number; // Font size in pixels (default: 12)
  bold: boolean; // Bold text (default: false)
  color: number; // BGR color code (default: 0x000000 - black)
}
```

#### Usage Examples

**Basic Highlighting:**

```javascript
{
  "tool_name": "highlight_element",
  "args": {
    "selector": "role:Button|name:Submit",
    "color": 0x00FF00,  // Bright green border (recommended)
    "duration_ms": 2000  // 2 seconds for good visibility
  }
}
```

**Highlighting with Text Overlay:**

```javascript
{
  "tool_name": "highlight_element",
  "args": {
    "selector": "#submit-button",
    "color": 0xFF0000,  // Blue border (BGR format)
    "duration_ms": 3000,
    "text": "Click Me!",
    "text_position": "Top",
    "font_style": {
      "size": 16,
      "bold": true,
      "color": 0x0000FF  // Red text (BGR format)
    }
  }
}
```

**Persistent Highlighting (Long Duration):**

```javascript
{
  "tool_name": "highlight_element",
  "args": {
    "selector": "role:TextBox|name:Username",
    "color": 0x00FFFF,  // Yellow border
    "duration_ms": 30000,  // 30 seconds
    "text": "Required",
    "text_position": "TopRight"
  }
}
```

## Proposed New Tools

### `stop_highlighting`

**Purpose:** Stops all active highlights immediately.

**Implementation Concept:**

```rust
#[tool(description = "Stops all active element highlights immediately.")]
async fn stop_highlighting(&self) -> Result<CallToolResult, McpError> {
    // Implementation would require tracking active highlight handles
    // in the server state and calling close() on all of them

    Ok(CallToolResult::success(vec![Content::json(json!({
        "action": "stop_highlighting",
        "status": "success",
        "highlights_stopped": 0, // Count of stopped highlights
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))?]))
}
```

**Usage:**

```javascript
{
  "tool_name": "stop_highlighting",
  "args": {}
}
```

### `show_text_overlay`

**Purpose:** Displays text overlay at specific screen coordinates without highlighting any element.

**Parameters:**

```typescript
interface ShowTextOverlayArgs {
  text: string; // Text to display
  x: number; // X coordinate on screen
  y: number; // Y coordinate on screen
  duration_ms?: number; // Duration (default: 3000ms)
  font_style?: FontStyle; // Font styling options
  background_color?: number; // Background color (BGR, optional)
  padding?: number; // Padding around text (default: 5px)
}
```

**Implementation Concept:**

```rust
#[tool(description = "Displays text overlay at specified screen coordinates.")]
async fn show_text_overlay(
    &self,
    Parameters(args): Parameters<ShowTextOverlayArgs>
) -> Result<CallToolResult, McpError> {
    // Implementation would create a floating text overlay
    // using Windows GDI or equivalent on other platforms

    let duration = Duration::from_millis(args.duration_ms.unwrap_or(3000));
    // Create and manage text overlay window/drawing

    Ok(CallToolResult::success(vec![Content::json(json!({
        "action": "show_text_overlay",
        "status": "success",
        "text": args.text,
        "position": {"x": args.x, "y": args.y},
        "duration_ms": duration.as_millis(),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))?]))
}
```

**Usage Examples:**

**Simple Text Display:**

```javascript
{
  "tool_name": "show_text_overlay",
  "args": {
    "text": "Processing...",
    "x": 500,
    "y": 300,
    "duration_ms": 2000
  }
}
```

**Styled Text with Background:**

```javascript
{
  "tool_name": "show_text_overlay",
  "args": {
    "text": "⚠️ Warning: Action Required",
    "x": 100,
    "y": 100,
    "duration_ms": 5000,
    "font_style": {
      "size": 18,
      "bold": true,
      "color": 0x0000FF  // Red text
    },
    "background_color": 0xFFFFE0,  // Light yellow background
    "padding": 10
  }
}
```

### `highlight_area`

**Purpose:** Highlights a rectangular area of the screen without targeting a specific element.

**Parameters:**

```typescript
interface HighlightAreaArgs {
  x: number; // Top-left X coordinate
  y: number; // Top-left Y coordinate
  width: number; // Width of area
  height: number; // Height of area
  color?: number; // Border color (BGR, default: 0x0000FF)
  duration_ms?: number; // Duration (default: 1000ms)
  text?: string; // Optional text overlay
  text_position?: TextPosition; // Text position within area
  font_style?: FontStyle; // Font styling
}
```

**Usage:**

```javascript
{
  "tool_name": "highlight_area",
  "args": {
    "x": 100,
    "y": 200,
    "width": 300,
    "height": 150,
    "color": 0x00FF00,  // Green border
    "duration_ms": 4000,
    "text": "Important Area",
    "text_position": "Inside"
  }
}
```

## Color Format Reference

All colors use BGR (Blue-Green-Red) format as 32-bit integers:

| Color   | BGR Value  | Hex       |
| ------- | ---------- | --------- |
| Red     | `0x0000FF` | `#FF0000` |
| Green   | `0x00FF00` | `#00FF00` |
| Blue    | `0xFF0000` | `#0000FF` |
| Yellow  | `0x00FFFF` | `#FFFF00` |
| Magenta | `0xFF00FF` | `#FF00FF` |
| Cyan    | `0xFFFF00` | `#00FFFF` |
| White   | `0xFFFFFF` | `#FFFFFF` |
| Black   | `0x000000` | `#000000` |

## Best Practices

### Highlighting Elements

1. **Use appropriate durations:** 1-2 seconds for quick feedback, 5+ seconds for important instructions
2. **Choose contrasting colors:** Ensure visibility against different backgrounds
3. **Position text wisely:** Use `TopRight` or `BottomRight` to avoid blocking the element
4. **Keep text concise:** Text is truncated, so use short, clear messages

### Text Overlays

1. **Strategic positioning:** Place text where it won't interfere with user interaction
2. **Readable fonts:** Use size 14+ for better visibility
3. **Background contrast:** Add background colors for text that might blend with content
4. **Appropriate timing:** Match duration to reading time and importance

### Error Handling

- Always provide fallback selectors for `highlight_element`
- Use timeouts to prevent hanging on missing elements
- Check element visibility before highlighting

## Implementation Notes

### Current Behavior (Overlay-Based System)

- **Overlay windows:** Uses transparent, layered Windows API windows instead of direct screen drawing
- **Precise timing:** Highlights persist for exact requested duration (fixed 50ms timing bug)
- **DPI scaling:** Automatic DPI awareness with `TERMINATOR_NO_DPI=1` fallback for debugging
- **Enhanced logging:** `OVERLAY_THREAD_START/DONE` logs for verification without visual confirmation
- **Improved visibility:** 6px border thickness and bright green (`0x00FF00`) recommended for visibility
- **Lifecycle management:** HighlightHandle automatically cleans up when duration expires
- **Text overlays:** Text positioning and styling with multiple position options
- **Workflow integration:** Seamlessly integrated with `record_workflow` tool for visual feedback during recording

### Troubleshooting

**Highlights not visible:**

- Set `TERMINATOR_NO_DPI=1` environment variable to disable DPI scaling
- Ensure target window is in foreground and not blocked by other always-on-top applications
- Increase `duration_ms` to 3000+ for better visibility
- Use bright colors like `0x00FF00` (green) or `0x00FFFF` (yellow) for contrast
- Check server logs for `OVERLAY_THREAD_START/DONE` to confirm highlighting executed

**Timing issues:**

- Our overlay system ensures exact duration timing (fixed previous 50ms bug)
- Duration is managed by the highlight thread, not the MCP tool return
- Use longer durations (2000ms+) for manual verification

### For Developers

To implement the proposed tools, you would need to:

1. **Stop Highlighting:** Track active highlight handles in server state
2. **Text Overlays:** Implement platform-specific text rendering (already implemented in overlay system)
3. **Area Highlighting:** Create coordinate-based highlighting without element targeting

### Platform Support

- **Windows:** Full support for all highlighting features
- **macOS/Linux:** Basic highlighting support (no text overlays currently)

## Workflow Examples

### Workflow Recording with Visual Feedback

The highlighting tool is automatically used when recording workflows:

```javascript
// Start recording with visual highlighting
{
  "tool_name": "record_workflow",
  "args": {
    "action": "start",
    "workflow_name": "My Workflow",
    "highlight_mode": {
      "enabled": true,
      "duration_ms": 2000,  // 2 seconds for visibility
      "color": 0x00FF00,    // Bright green for contrast
      "show_labels": true   // Shows "CLICK", "TYPE", etc.
    }
  }
}
```

This provides immediate visual confirmation of captured events during recording.

### Visual Tutorial Sequence

```javascript
// Step 1: Highlight the menu
{
  "tool_name": "highlight_element",
  "args": {
    "selector": "role:MenuBar",
    "text": "Step 1",
    "duration_ms": 3000
  }
}

// Step 2: Show instruction
{
  "tool_name": "show_text_overlay",
  "args": {
    "text": "Click on File menu to continue",
    "x": 300,
    "y": 100,
    "duration_ms": 2000
  }
}

// Step 3: Highlight specific button
{
  "tool_name": "highlight_element",
  "args": {
    "selector": "role:Button|name:New",
    "color": 0x00FF00,
    "text": "Click!",
    "duration_ms": 5000
  }
}
```

### Form Validation Feedback

```javascript
// Highlight required field
{
  "tool_name": "highlight_element",
  "args": {
    "selector": "role:TextBox|name:Email",
    "color": 0x0000FF,  // Red for error
    "text": "Required",
    "text_position": "TopRight",
    "duration_ms": 8000
  }
}

// Show error message
{
  "tool_name": "show_text_overlay",
  "args": {
    "text": "Please enter a valid email address",
    "x": 400,
    "y": 250,
    "font_style": {
      "size": 14,
      "bold": true,
      "color": 0x0000FF
    },
    "background_color": 0xE0E0FF,
    "duration_ms": 5000
  }
}
```
