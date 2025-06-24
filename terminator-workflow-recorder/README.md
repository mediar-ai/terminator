# Terminator Workflow Recorder

A Rust crate for recording user workflows on Windows, part of the [Terminator](https://github.com/mediar-ai/terminator) project.

## Features

- Records user interactions including clicks, keyboard input, and UI navigation
- Captures UI element metadata using Windows UIAutomation
- Supports complex workflows with autocomplete detection
- Outputs structured workflow data for playback or analysis
- Highly performant with minimal system overhead

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
terminator-workflow-recorder = "0.5.12"
```

## Usage

```rust
use terminator_workflow_recorder::{WorkflowRecorder, WorkflowRecorderConfig};
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create recorder with default config
    let config = WorkflowRecorderConfig::default();
    let recorder = WorkflowRecorder::new("My Workflow".to_string(), config);
    
    // Start recording
    println!("Recording started. Press Ctrl+C to stop...");
    recorder.start().await?;
    
    // Handle Ctrl+C
    tokio::signal::ctrl_c().await?;
    
    // Stop and get workflow
    let workflow = recorder.stop().await?;
    println!("Recorded {} events", workflow.events.len());
    
    Ok(())
}
```

## Platform Support

Currently supports Windows only. The recorder uses Windows UIAutomation APIs for capturing UI element information.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
