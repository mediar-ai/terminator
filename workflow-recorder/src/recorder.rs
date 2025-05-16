use crate::{
    MouseButton, MouseEvent, MouseEventType, Position, RecordedWorkflow, UiElement, WindowEvent,
    WorkflowEvent, WorkflowRecorderError, Result, IntentGroup, IntentGroupingConfig, extract_intent_groups
};
use std::{
    fs::File,
    io::Write,
    path::Path,
    sync::{Arc, Mutex},
};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tracing::{debug, error, info, warn};

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
    event_tx: UnboundedSender<WorkflowEvent>,
    
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
        let (event_tx, _) = mpsc::unbounded_channel();
        
        Self {
            workflow,
            event_tx,
            config,
            #[cfg(target_os = "windows")]
            windows_recorder: None,
        }
    }
    
    /// Start recording
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting workflow recording");
        
        #[cfg(target_os = "windows")]
        {
            let workflow = Arc::clone(&self.workflow);
            let (event_tx, mut event_rx) = mpsc::unbounded_channel();
            self.event_tx = event_tx.clone();
            
            // Start the Windows recorder
            let windows_recorder = WindowsRecorder::new(self.config.clone(), event_tx)?;
            self.windows_recorder = Some(windows_recorder);
            
            // Start the event processing task
            tokio::spawn(async move {
                Self::process_events(workflow, &mut event_rx).await;
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
        event_rx: &mut UnboundedReceiver<WorkflowEvent>,
    ) {
        while let Some(event) = event_rx.recv().await {
            if let Ok(mut workflow) = workflow.lock() {
                workflow.add_event(event);
            }
        }
    }
    
    /// Extract intent groups from the recorded workflow
    pub fn extract_intent_groups(&self) -> Result<Vec<IntentGroup>> {
        let workflow = self.workflow.lock().map_err(|e| {
            WorkflowRecorderError::SaveError(format!("Failed to lock workflow: {}", e))
        })?;
        
        Ok(extract_intent_groups(&workflow))
    }
    
    /// Extract intent groups with custom configuration
    pub fn extract_intent_groups_with_config(&self, config: IntentGroupingConfig) -> Result<Vec<IntentGroup>> {
        let workflow = self.workflow.lock().map_err(|e| {
            WorkflowRecorderError::SaveError(format!("Failed to lock workflow: {}", e))
        })?;
        
        Ok(crate::intent::group_events(&workflow, &config))
    }
    
    /// Save the extracted intent groups to a file
    pub fn save_intent_groups<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let groups = self.extract_intent_groups()?;
        
        // Create a serializable representation of the groups
        #[derive(serde::Serialize)]
        struct SerializableIntentGroup {
            name: String,
            start_time: u64,
            end_time: u64,
            event_count: usize,
            application_context: Option<SerializableAppContext>,
        }
        
        #[derive(serde::Serialize)]
        struct SerializableAppContext {
            application_name: Option<String>,
            window_title: Option<String>,
        }
        
        let serializable_groups: Vec<SerializableIntentGroup> = groups.into_iter()
            .map(|group| {
                SerializableIntentGroup {
                    name: group.name,
                    start_time: group.start_time,
                    end_time: group.end_time,
                    event_count: group.events.len(),
                    application_context: group.application_context.map(|ctx| {
                        SerializableAppContext {
                            application_name: ctx.application_name,
                            window_title: ctx.window_title,
                        }
                    }),
                }
            })
            .collect();
        
        let json = serde_json::to_string_pretty(&serializable_groups)?;
        let mut file = File::create(path)?;
        file.write_all(json.as_bytes())?;
        
        Ok(())
    }
} 