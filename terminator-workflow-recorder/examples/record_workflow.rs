use tokio_stream::StreamExt;
use workflow_recorder::{WorkflowRecorder, WorkflowRecorderConfig};
use std::path::PathBuf;
use tokio::signal::ctrl_c;
use tracing::{info, debug, Level};
use tracing_subscriber::FmtSubscriber;
// use std::panic::AssertUnwindSafe; // Not used due to async limitation

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("[EARLY] main() started");
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    info!("[LOG] main() started");

    // NOTE: std::panic::catch_unwind does not work with async blocks in stable Rust.
    // If you want to catch panics, you must use it only around synchronous code.
    // For now, we just run the async code directly.
    info!("[LOG] Inside main logic");
    // Create a configuration with enhanced metadata capture
    let config = WorkflowRecorderConfig {
        record_mouse: true,
        record_keyboard: true,
        record_window: true,
        capture_ui_elements: true,
    };
    debug!("Initializing recorder with config: {:?}", config);
    // Create a recorder
    let mut recorder = WorkflowRecorder::new("Example Workflow".to_string(), config);
    debug!("Starting recording...");
    let mut event_stream = recorder.event_stream();
    recorder.start().await.expect("Failed to start recorder");
    // Process events from the stream
    while let Some(event) = event_stream.next().await {
        println!("Received event: {:?}", event);
    }
    info!("Recording started. Interact with your desktop...");
    debug!("Waiting for Ctrl+C signal...");
    ctrl_c().await.expect("Failed to wait for Ctrl+C");
    info!("Stopping recording...");
    debug!("Sending stop signal to recorder...");
    recorder.stop().await.expect("Failed to stop recorder");
    let output_path = PathBuf::from("workflow_recording.json");
    debug!("Saving raw recording to {:?}", output_path);
    recorder.save(&output_path).expect("Failed to save recording");
    info!("Recording saved to {:?}", output_path);
    Ok(())
} 