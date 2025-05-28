# Workflow Recorder

A Rust crate for recording user interactions with the desktop UI, designed for capturing and replaying workflows.

## Features

- Records mouse events (clicks, movements)
- Records keyboard events
- Captures UI element information (using Windows UI Automation)
- Rich contextual metadata for UI elements and applications
- Intelligent intent grouping to cluster related events
- Exports recordings to JSON format
- Cross-platform design (currently Windows-only implementation)

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
workflow-recorder = { path = "path/to/workflow-recorder" }
```

### Basic Example

```rust
use workflow_recorder::{WorkflowRecorder, WorkflowRecorderConfig};
use std::path::PathBuf;
use tokio::signal::ctrl_c;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a default configuration
    let config = WorkflowRecorderConfig::default();
    
    // Create a recorder
    let mut recorder = WorkflowRecorder::new("Example Workflow".to_string(), config);
    
    // Start recording
    recorder.start().await?;
    
    // Wait for Ctrl+C
    println!("Recording started. Press Ctrl+C to stop...");
    ctrl_c().await?;
    
    // Stop recording
    recorder.stop().await?;
    
    // Save the recording
    let output_path = PathBuf::from("workflow_recording.json");
    recorder.save(&output_path)?;
    println!("Recording saved to {:?}", output_path);
    
    Ok(())
}
```

### Intent Grouping Example

```rust
use workflow_recorder::{WorkflowRecorder, WorkflowRecorderConfig, IntentGroupingConfig};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create and run recorder (as in basic example)
    // ...

    // Extract intent groups with default configuration
    let intent_groups = recorder.extract_intent_groups()?;
    println!("Extracted {} intent groups", intent_groups.len());

    // Save intent groups to a file
    let intent_groups_path = PathBuf::from("workflow_intent_groups.json");
    recorder.save_intent_groups(&intent_groups_path)?;
    
    // Use custom grouping configuration
    let custom_config = IntentGroupingConfig {
        max_time_gap: 1000,               // 1 second
        split_on_focus_change: true,
        split_on_pause: true,
        min_pause_duration: 2000,         // 2 seconds
    };
    
    let custom_groups = recorder.extract_intent_groups_with_config(custom_config)?;
    
    Ok(())
}
```

## Running the Example

```bash
cargo run --example record_workflow
```

Press `Ctrl+C` to stop recording. The workflow will be saved to:
- `workflow_recording.json` - Raw event recording
- `workflow_intent_groups.json` - Extracted intent groups

## Output Format

### Raw Recording

The recorded workflow is saved as a JSON file with the following structure:

```json
{
  "name": "Example Workflow",
  "start_time": 1621234567890,
  "end_time": 1621234598765,
  "events": [
    {
      "timestamp": 1621234568000,
      "event": {
        "Mouse": {
          "event_type": "Click",
          "button": "Left",
          "position": {
            "x": 100,
            "y": 200
          },
          "ui_element": {
            "name": "Button",
            "automation_id": "button1",
            "class_name": "Button",
            "control_type": "Button",
            "process_id": 1234,
            "application_name": "example.exe",
            "window_title": "Example Window",
            "bounding_rect": {
              "x": 90,
              "y": 190,
              "width": 100,
              "height": 30
            },
            "is_enabled": true,
            "has_keyboard_focus": false,
            "hierarchy_path": "Window[Example]/Panel[main]/Button[button1]",
            "value": null
          }
        }
      }
    }
  ]
}
```

### Intent Groups

Intent groups are saved as a JSON file with the following structure:

```json
[
  {
    "name": "Text input in notepad.exe",
    "start_time": 1621234567890,
    "end_time": 1621234570000,
    "event_count": 25,
    "application_context": {
      "application_name": "notepad.exe",
      "window_title": "Untitled - Notepad"
    }
  },
  {
    "name": "Navigation in chrome.exe",
    "start_time": 1621234571000,
    "end_time": 1621234580000,
    "event_count": 12,
    "application_context": {
      "application_name": "chrome.exe",
      "window_title": "Google - Google Chrome"
    }
  }
]
```

## Platform Support

Currently, this crate only supports Windows. The architecture is designed to be cross-platform, but implementations for macOS and Linux are not yet available.
