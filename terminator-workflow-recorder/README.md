# Terminator Workflow Recorder

A comprehensive workflow recording library for Windows that captures user interactions with UI elements, including mouse clicks, keyboard input, clipboard operations, and UI automation events.

## Features

- **Input Recording**: Mouse movements, clicks, keyboard input
- **UI Element Capture**: Detailed information about UI elements being interacted with
- **Clipboard Monitoring**: Track copy/paste operations
- **Hotkey Detection**: Record keyboard shortcuts and hotkey combinations
- **UI Automation Events**: Focus changes, property changes, structure changes
- **Noise Filtering**: Built-in filtering to ignore system UI noise like clock updates

## Usage

### Basic Recording

```rust
use terminator_workflow_recorder::{WorkflowRecorder, WorkflowRecorderConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WorkflowRecorderConfig::default();
    let mut recorder = WorkflowRecorder::new("My Workflow".to_string(), config);
    
    recorder.start().await?;
    
    // ... perform your workflow ...
    
    recorder.stop().await?;
    recorder.save("workflow.json")?;
    
    Ok(())
}
```

### Filtering System UI Noise

The recorder includes built-in filtering to ignore noisy system UI elements like the clock, notifications, and other system components. You can customize this filtering:

```rust
let config = WorkflowRecorderConfig {
    // Enable UI event recording
    record_ui_focus_changes: true,
    record_ui_property_changes: true,
    
    // Filter out noisy system elements
    ignore_focus_patterns: vec![
        "clock".to_string(),
        "notification".to_string(),
        "tooltip".to_string(),
        "popup".to_string(),
    ],
    ignore_property_patterns: vec![
        "clock".to_string(),
        "time".to_string(),
        "pm".to_string(),
        "am".to_string(),
    ],
    ignore_window_titles: vec![
        "Windows Security".to_string(),
        "Action Center".to_string(),
        "Task Manager".to_string(),
    ],
    ignore_applications: vec![
        "explorer.exe".to_string(),
        "dwm.exe".to_string(),
        "winlogon.exe".to_string(),
    ],
    
    // Other configuration options
    ..Default::default()
};
```

### Configuration Options

#### Recording Controls
- `record_mouse`: Enable/disable mouse event recording
- `record_keyboard`: Enable/disable keyboard event recording
- `record_clipboard`: Enable/disable clipboard operation recording
- `record_ui_focus_changes`: Enable/disable UI focus change events
- `record_ui_property_changes`: Enable/disable UI property change events
- `capture_ui_elements`: If enabled, UI element info is captured asynchronously and events are enriched in the background. This is now non-blocking and does not cause lag.

#### Noise Reduction
- `mouse_move_throttle_ms`: Minimum time between mouse move events (default: 50ms)
- `ignore_focus_patterns`: Patterns to ignore in focus change events
- `ignore_property_patterns`: Patterns to ignore in property change events
- `ignore_window_titles`: Window titles to ignore for all UI events
- `ignore_applications`: Application names to ignore for all UI events

#### Content Limits
- `max_clipboard_content_length`: Maximum clipboard content to record (default: 1KB)
- `max_text_selection_length`: Maximum text selection length to record (default: 512 chars)

## Common Filtering Patterns

### Clock and Time Elements
```rust
ignore_property_patterns: vec![
    "clock".to_string(),
    "time".to_string(),
    "pm".to_string(),
    "am".to_string(),
],
```

### System Notifications
```rust
ignore_focus_patterns: vec![
    "notification".to_string(),
    "action center".to_string(),
    "toast".to_string(),
],
```

### Taskbar and System Tray
```rust
ignore_focus_patterns: vec![
    "taskbar".to_string(),
    "system tray".to_string(),
    "start button".to_string(),
],
```

### Windows System Applications
```rust
ignore_applications: vec![
    "dwm.exe".to_string(),           // Desktop Window Manager
    "explorer.exe".to_string(),      // Windows Explorer
    "winlogon.exe".to_string(),      // Windows Logon
    "csrss.exe".to_string(),         // Client Server Runtime
],
```

## Output Format

The recorder saves workflows as JSON files containing timestamped events:

```json
{
  "name": "My Workflow",
  "start_time": 1748456891489,
  "end_time": 1748456956367,
  "events": [
    {
      "timestamp": 1748456891524,
      "event": {
        "Keyboard": {
          "key_code": 65,
          "is_key_down": true,
          "character": "a",
          "metadata": {
            "ui_element": {
              "role": "textfield",
              "name": "Search Box"
            }
          }
        }
      }
    }
  ]
}
```

## Performance Considerations

- UIAutomation queries for UI element info are now performed asynchronously. Events are immediately sent with `ui_element: None`, and a background thread fetches the UI element and emits a `UiElementEnriched` event if enrichment is enabled. This prevents UI freezes on click and other input events.
- Use filtering to reduce event volume for better performance
- Consider disabling UI automation events (`record_ui_*`) if not needed
- Adjust `mouse_move_throttle_ms` to balance accuracy vs. performance
- Set appropriate content length limits for clipboard and text selection

## Platform Support

Currently supports Windows only. Requires Windows 10/11 with UI Automation support.
