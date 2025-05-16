use workflow_recorder::{WorkflowRecorder, WorkflowRecorderConfig, IntentGroupingConfig};
use std::path::PathBuf;
use tokio::signal::ctrl_c;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    
    info!("Workflow Recorder Example");
    info!("Press Ctrl+C to stop recording");
    
    // Create a configuration with enhanced metadata capture
    let config = WorkflowRecorderConfig {
        record_mouse: true,
        record_keyboard: true,
        record_window: true,
        capture_ui_elements: true,
    };
    
    // Create a recorder
    let mut recorder = WorkflowRecorder::new("Example Workflow".to_string(), config);
    
    // Start recording
    recorder.start().await?;
    
    // Wait for Ctrl+C
    info!("Recording started. Interact with your desktop...");
    ctrl_c().await?;
    
    // Stop recording
    info!("Stopping recording...");
    recorder.stop().await?;
    
    // Save the raw recording
    let output_path = PathBuf::from("workflow_recording.json");
    recorder.save(&output_path)?;
    info!("Recording saved to {:?}", output_path);
    
    // Extract and save intent groups with default configuration
    let intent_groups_path = PathBuf::from("workflow_intent_groups.json");
    recorder.save_intent_groups(&intent_groups_path)?;
    info!("Intent groups saved to {:?}", intent_groups_path);
    
    // Extract and save intent groups with custom configuration
    let custom_config = IntentGroupingConfig {
        max_time_gap: 1000,               // 1 second
        split_on_focus_change: true,
        split_on_pause: true,
        min_pause_duration: 2000,         // 2 seconds
    };
    
    let custom_groups = recorder.extract_intent_groups_with_config(custom_config)?;
    info!("Extracted {} intent groups with custom configuration", custom_groups.len());
    
    // Print summary of intent groups
    info!("Intent Group Summary:");
    for (i, group) in custom_groups.iter().enumerate() {
        let duration_ms = group.end_time - group.start_time;
        let app_context = group.application_context.as_ref()
            .and_then(|ctx| ctx.application_name.as_ref())
            .unwrap_or(&"Unknown".to_string());
        
        info!(
            "  Group {}: '{}' - {} events over {}ms in {}",
            i + 1,
            group.name,
            group.events.len(),
            duration_ms,
            app_context
        );
    }
    
    Ok(())
} 