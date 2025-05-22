use crate::{
    RecordedWorkflow,
    WorkflowEvent, WorkflowRecorderError, Result
};
use std::{
    fs::File,
    io::Write,
    path::Path,
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;
use tokio_stream::{Stream};
use tracing::{info};

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
pub use self::windows::*;

/// Configuration for the workflow recorder
#[derive(Debug, Clone)]
pub struct WorkflowRecorderConfig {
    /// Whether to record mouse events
    pub record_mouse: bool,
    
    /// Whether to record keyboard events
    pub record_keyboard: bool,
    
    /// Whether to record window events
    pub record_window: bool,
    
    /// Whether to capture UI element information
    pub capture_ui_elements: bool,
}

impl Default for WorkflowRecorderConfig {
    fn default() -> Self {
        Self {
            record_mouse: true,
            record_keyboard: true,
            record_window: true,
            capture_ui_elements: true,
        }
    }
}

/// The workflow recorder
pub struct WorkflowRecorder {
    /// The recorded workflow
    workflow: Arc<Mutex<RecordedWorkflow>>,
    
    /// The event sender
    event_tx: broadcast::Sender<WorkflowEvent>,
    
    /// The configuration
    config: WorkflowRecorderConfig,
    
    /// The platform-specific recorder
    #[cfg(target_os = "windows")]
    windows_recorder: Option<WindowsRecorder>,
}

impl WorkflowRecorder {
    /// Create a new workflow recorder
    pub fn new(name: String, config: WorkflowRecorderConfig) -> Self {
        let workflow = Arc::new(Mutex::new(RecordedWorkflow::new(name)));
        let (event_tx, _) = broadcast::channel(100); // Buffer size of 100 events
        
        Self {
            workflow,
            event_tx,
            config,
            #[cfg(target_os = "windows")]
            windows_recorder: None,
        }
    }

    /// Get a stream of events
    pub fn event_stream(&self) -> impl Stream<Item = WorkflowEvent> {
        let mut rx = self.event_tx.subscribe();
        Box::pin(async_stream::stream! {
            while let Ok(event) = rx.recv().await {
                yield event;
            }
        })
    }
    
    /// Start recording
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting workflow recording");
        
        #[cfg(target_os = "windows")]
        {
            let workflow = Arc::clone(&self.workflow);
            let event_tx = self.event_tx.clone();
            
            // Start the Windows recorder
            let windows_recorder = WindowsRecorder::new(self.config.clone(), event_tx)?;
            self.windows_recorder = Some(windows_recorder);
            
            // Start the event processing task
            let mut event_rx = self.event_tx.subscribe();
            tokio::spawn(async move {
                Self::process_events(workflow, event_rx).await;
            });
            
            Ok(())
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            Err(WorkflowRecorderError::InitializationError(
                "Workflow recording is only supported on Windows".to_string(),
            ))
        }
    }
    
    /// Stop recording
    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping workflow recording");
        
        #[cfg(target_os = "windows")]
        {
            if let Some(windows_recorder) = self.windows_recorder.take() {
                windows_recorder.stop()?;
            }
        }
        
        // Mark the workflow as finished
        if let Ok(mut workflow) = self.workflow.lock() {
            workflow.finish();
        }
        
        Ok(())
    }
    
    /// Save the recorded workflow to a file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        info!("Saving workflow recording to {:?}", path.as_ref());
        
        let workflow = self.workflow.lock().map_err(|e| {
            WorkflowRecorderError::SaveError(format!("Failed to lock workflow: {}", e))
        })?;
        
        let json = serde_json::to_string_pretty(&*workflow)?;
        let mut file = File::create(path)?;
        file.write_all(json.as_bytes())?;
        
        Ok(())
    }
    
    /// Process events from the event receiver
    async fn process_events(
        workflow: Arc<Mutex<RecordedWorkflow>>,
        mut event_rx: broadcast::Receiver<WorkflowEvent>,
    ) {
        while let Ok(event) = event_rx.recv().await {
            if let Ok(mut workflow) = workflow.lock() {
                workflow.add_event(event);
            }
        }
    }
} 