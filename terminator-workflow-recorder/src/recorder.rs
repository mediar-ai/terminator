use crate::{
    ConversionConfig, McpConverter, RecordedWorkflow, Result, WorkflowEvent, WorkflowRecorderError,
};
use std::{
    collections::HashSet,
    path::Path,
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;
use tokio_stream::Stream;
use tracing::info;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "windows")]
pub use self::windows::*;

/// Configuration for the workflow recorder
#[derive(Debug, Clone)]
pub struct WorkflowRecorderConfig {
    /// Whether to record keyboard events
    pub record_keyboard: bool,

    /// Whether to capture UI element information
    pub capture_ui_elements: bool,

    /// Whether to record clipboard operations
    pub record_clipboard: bool,

    /// Whether to record hotkey/shortcut events
    pub record_hotkeys: bool,

    /// Maximum clipboard content length to record (longer content will be truncated)
    pub max_clipboard_content_length: usize,

    /// Whether to track modifier key states accurately
    pub track_modifier_states: bool,

    /// Minimum drag distance to distinguish between click and drag (pixels)
    pub min_drag_distance: f64,

    /// Patterns to ignore for UI focus change events (case-insensitive)
    pub ignore_focus_patterns: HashSet<String>,

    /// Patterns to ignore for UI property change events (case-insensitive)
    pub ignore_property_patterns: HashSet<String>,

    /// Window titles to ignore for UI events (case-insensitive)
    pub ignore_window_titles: HashSet<String>,

    /// Application/process names to ignore for UI events (case-insensitive)
    pub ignore_applications: HashSet<String>,

    /// Whether to enable multithreading for COM initialization and event processing
    /// On Windows: Controls COINIT_MULTITHREADED vs COINIT_APARTMENTTHREADED
    /// On other platforms: Controls threading model for equivalent operations
    ///
    /// # Examples
    ///
    /// ```rust
    /// use terminator_workflow_recorder::WorkflowRecorderConfig;
    ///
    /// let mut config = WorkflowRecorderConfig::default();
    /// config.enable_multithreading = true;  // Use multithreaded COM (MTA)
    /// config.enable_multithreading = false; // Use apartment threaded COM (STA) - default
    /// ```
    ///
    /// Note: Apartment threaded (STA) mode may provide better system responsiveness
    /// but multithreaded (MTA) mode may be required for some complex scenarios.
    pub enable_multithreading: bool,
}

impl Default for WorkflowRecorderConfig {
    fn default() -> Self {
        Self {
            record_keyboard: true, // TODO not used
            capture_ui_elements: true,
            record_clipboard: true,
            record_hotkeys: true,
            max_clipboard_content_length: 10240, // 10KB max
            track_modifier_states: true,
            min_drag_distance: 5.0, // 5 pixels minimum for drag detection
            ignore_focus_patterns: [
                // Common system UI patterns to ignore by default
                "notification".to_string(),
                "tooltip".to_string(),
                "popup".to_string(),
                // Screen sharing/recording notifications
                "sharing your screen".to_string(),
                "recording screen".to_string(),
                "screen capture".to_string(),
                "screen share".to_string(),
                "is sharing".to_string(),
                "screen recording".to_string(),
                "presenting".to_string(), // For Google Meet, etc.
                "google meet".to_string(),
                "zoom".to_string(),
                "loom".to_string(),
                "1password".to_string(),
                "lastpass".to_string(),
                "dashlane".to_string(),
                "bitwarden".to_string(),
                // Mediar product - ignore our own product interactions
                "mediar".to_string(),
                // Common background noise patterns
                "battery".to_string(),
                "volume".to_string(),
                "network".to_string(),
                "wifi".to_string(),
                "bluetooth".to_string(),
                "download".to_string(),
                "progress".to_string(),
                "update".to_string(),
                "sync".to_string(),
                "indexing".to_string(),
                "scanning".to_string(),
                "backup".to_string(),
                "maintenance".to_string(),
                "defender".to_string(),
                "antivirus".to_string(),
                "security".to_string(),
                "system tray".to_string(),
                "hidden icons".to_string(),
            ]
            .into_iter()
            .collect(),
            ignore_property_patterns: [
                // Common property change patterns to ignore by default
                "clock".to_string(),
                "time".to_string(),
                // Screen sharing/recording related
                "sharing".to_string(),
                "recording".to_string(),
                "capture".to_string(),
                "presenting".to_string(), // For Google Meet, etc.
                "google meet".to_string(),
                "zoom".to_string(),
                "loom".to_string(),
                "1password".to_string(),
                "lastpass".to_string(),
                "dashlane".to_string(),
                "bitwarden".to_string(),
                // Mediar product - ignore our own product interactions
                "mediar".to_string(),
                // System status and background updates
                "battery".to_string(),
                "volume".to_string(),
                "network".to_string(),
                "download".to_string(),
                "progress".to_string(),
                "percent".to_string(),
                "mb".to_string(),
                "gb".to_string(),
                "kb".to_string(),
                "bytes".to_string(),
                "status".to_string(),
                "state".to_string(),
                "level".to_string(),
                "signal".to_string(),
                "connection".to_string(),
                "sync".to_string(),
                "update".to_string(),
                "version".to_string(),
            ]
            .into_iter()
            .collect(),
            ignore_window_titles: [
                // Common window titles to ignore by default
                "Windows Security".to_string(),
                "Action Center".to_string(),
                // Google Meet and other video conferencing/screen sharing overlays
                "Google Meet".to_string(),
                "meet.google.com".to_string(),
                "You're presenting".to_string(), // Covers "You're presenting to everyone"
                "Stop presenting".to_string(),
                "Zoom".to_string(),
                "Zoom Meeting".to_string(),
                "You are sharing your screen".to_string(),
                "Stop sharing".to_string(),
                "Loom".to_string(),
                "loom.com".to_string(),
                // Password manager overlays
                "1Password".to_string(),
                "LastPass".to_string(),
                "Dashlane".to_string(),
                "Bitwarden".to_string(),
                // Mediar product - ignore our own product interactions
                "Mediar".to_string(),
                // Browser screen sharing notifications
                "is sharing your screen".to_string(),
                "Screen sharing".to_string(),
                "Recording screen".to_string(),
                "Screen capture notification".to_string(),
                "Chrome is sharing".to_string(),
                "Firefox is sharing".to_string(),
                "Edge is sharing".to_string(),
                "Safari is sharing".to_string(),
                // Windows system notifications and background windows
                "Notification area".to_string(),
                "System tray".to_string(),
                "Hidden icons".to_string(),
                "Battery meter".to_string(),
                "Volume mixer".to_string(),
                "Network".to_string(),
                "Wi-Fi".to_string(),
                "Bluetooth".to_string(),
                "Windows Update".to_string(),
                "Microsoft Store".to_string(),
                "Windows Defender".to_string(),
                "Antimalware Service".to_string(),
                "Background Task Host".to_string(),
                "Desktop Window Manager".to_string(),
                "File Explorer".to_string(),
                "Windows Shell Experience".to_string(),
                "Search".to_string(),
                "Cortana".to_string(),
                "Start".to_string(),
                "Taskbar".to_string(),
                "Focus Assist".to_string(),
                "Quick Actions".to_string(),
                "Calendar".to_string(),
                "Weather".to_string(),
                "News and interests".to_string(),
                "Widgets".to_string(),
            ]
            .into_iter()
            .collect(),
            ignore_applications: [
                // Common applications to ignore by default
                "dwm.exe".to_string(),
                "taskmgr.exe".to_string(),
                "powershell.exe".to_string(),
                "cmd.exe".to_string(),
                "cursor.exe".to_string(),
                "code.exe".to_string(),
                // Mediar product - ignore our own product interactions
                "mediar.exe".to_string(),
                // Windows system processes that generate noise
                "explorer.exe".to_string(),
                "winlogon.exe".to_string(),
                "csrss.exe".to_string(),
                "wininit.exe".to_string(),
                "services.exe".to_string(),
                "lsass.exe".to_string(),
                "svchost.exe".to_string(),
                "conhost.exe".to_string(),
                "rundll32.exe".to_string(),
                "backgroundtaskhost.exe".to_string(),
                "runtimebroker.exe".to_string(),
                "applicationframehost.exe".to_string(),
                "shellexperiencehost.exe".to_string(),
                "startmenuexperiencehost.exe".to_string(),
                "searchui.exe".to_string(),
                "searchapp.exe".to_string(),
                "cortana.exe".to_string(),
                "sihost.exe".to_string(),
                "winstore.app".to_string(),
                "microsoftedge.exe".to_string(),
                "msedgewebview2.exe".to_string(),
                // Security and system maintenance
                "msmpeng.exe".to_string(), // Windows Defender
                "antimalware service executable".to_string(),
                "windows security".to_string(),
                "mssense.exe".to_string(), // Windows Defender Advanced Threat Protection
                "smartscreen.exe".to_string(), // Windows SmartScreen
                // Background services that create noise
                "audiodg.exe".to_string(), // Audio Device Graph Isolation
                "fontdrvhost.exe".to_string(), // Font Driver Host
                "lsaiso.exe".to_string(),  // Credential Guard
                "sgrmbroker.exe".to_string(), // System Guard Runtime Monitor
                "unsecapp.exe".to_string(), // Sink to receive asynchronous callbacks
                "wmiprvse.exe".to_string(), // WMI Provider Service
                "dllhost.exe".to_string(), // COM Surrogate
                "msiexec.exe".to_string(), // Windows Installer
                "trustedinstaller.exe".to_string(), // Windows Modules Installer
                // Third-party common background apps
                // "teams.exe".to_string(),
                // "slack.exe".to_string(),
                // "discord.exe".to_string(),
                // "spotify.exe".to_string(),
                // "steam.exe".to_string(),
                // "dropbox.exe".to_string(),
                // "onedrive.exe".to_string(),
                // "googledrivesync.exe".to_string(),
                // "skype.exe".to_string(),
                // "zoom.exe".to_string(),
                // Password manager applications
                "1Password.exe".to_string(),
                "LastPass.exe".to_string(),
                "Dashlane.exe".to_string(),
                "Bitwarden.exe".to_string(),
                // Snipping Tool application.
                "SnippingTool.exe".to_string(),
            ]
            .into_iter()
            .collect(),
            enable_multithreading: false, // Default to false for better system responsiveness
        }
    }
}

impl WorkflowRecorderConfig {}

/// The workflow recorder
pub struct WorkflowRecorder {
    /// The recorded workflow
    pub workflow: Arc<Mutex<RecordedWorkflow>>,

    /// The event sender
    event_tx: broadcast::Sender<WorkflowEvent>,

    /// The configuration
    config: WorkflowRecorderConfig,

    /// The MCP converter for generating tool sequences
    mcp_converter: Option<McpConverter>,

    /// The platform-specific recorder
    #[cfg(target_os = "windows")]
    windows_recorder: Option<WindowsRecorder>,
}

impl WorkflowRecorder {
    /// Create a new workflow recorder
    pub fn new(name: String, config: WorkflowRecorderConfig) -> Self {
        let workflow = Arc::new(Mutex::new(RecordedWorkflow::new(name)));
        let (event_tx, _) = broadcast::channel(100); // Buffer size of 100 events

<<<<<<< HEAD
        // Create MCP converter with default settings
=======
        // Initialize MCP converter with default configuration so that
        // we can enrich recorded events with MCP-ready sequences.
>>>>>>> 2c7b68c (recorder: restore core features; migrate ButtonClick->ClickEvent; add resolver; retain performance modes; docs: update record_workflow; mcp_converter: remove unused; tests/examples updated; .gitignore: recorder logs, scripts/local/**)
        let conversion_config = ConversionConfig::default();
        let mcp_converter = Some(McpConverter::with_config(conversion_config));

        Self {
            workflow,
            event_tx,
            config,
            mcp_converter,
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
            let windows_recorder = WindowsRecorder::new(self.config.clone(), event_tx).await?;
            self.windows_recorder = Some(windows_recorder);

            // Start the event processing task
            let event_rx = self.event_tx.subscribe();
            let mcp_converter = self.mcp_converter.clone();
            tokio::spawn(async move {
                Self::process_events(workflow, event_rx, mcp_converter).await;
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
            WorkflowRecorderError::SaveError(format!("Failed to lock workflow: {e}"))
        })?;

        workflow.save_to_file(path).map_err(|e| {
            WorkflowRecorderError::SaveError(format!("Failed to save workflow: {e}"))
        })?;

        Ok(())
    }

    /// Process events from the event receiver
    async fn process_events(
        workflow: Arc<Mutex<RecordedWorkflow>>,
        mut event_rx: broadcast::Receiver<WorkflowEvent>,
        mcp_converter: Option<McpConverter>,
    ) {
        while let Ok(event) = event_rx.recv().await {
<<<<<<< HEAD
            // If MCP conversion is enabled, enhance the event
=======
            // If MCP conversion is enabled, try to enrich the event. Fall back to
            // recording the original event on failure to preserve behavior.
>>>>>>> 2c7b68c (recorder: restore core features; migrate ButtonClick->ClickEvent; add resolver; retain performance modes; docs: update record_workflow; mcp_converter: remove unused; tests/examples updated; .gitignore: recorder logs, scripts/local/**)
            let recorded_event = if let Some(ref converter) = mcp_converter {
                match converter.convert_event(&event, None).await {
                    Ok(conversion_result) => {
                        let timestamp = event.timestamp().unwrap_or_else(|| {
                            std::time::SystemTime::now()
                                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as u64
                        });

                        crate::events::RecordedEvent {
                            timestamp,
                            event,
                            mcp_sequence: Some(conversion_result.primary_sequence),
                            semantic_action: Some(conversion_result.semantic_action),
                            fallback_sequences: Some(conversion_result.fallback_sequences),
<<<<<<< HEAD
                            enhanced_ui_context: None, // TODO: Add UI context analysis
                        }
                    }
                    Err(e) => {
                        // Log conversion error but still record the original event
=======
                            enhanced_ui_context: None,
                        }
                    }
                    Err(e) => {
>>>>>>> 2c7b68c (recorder: restore core features; migrate ButtonClick->ClickEvent; add resolver; retain performance modes; docs: update record_workflow; mcp_converter: remove unused; tests/examples updated; .gitignore: recorder logs, scripts/local/**)
                        tracing::warn!("MCP conversion failed: {}", e);
                        let timestamp = event.timestamp().unwrap_or_else(|| {
                            std::time::SystemTime::now()
                                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as u64
                        });

                        crate::events::RecordedEvent {
                            timestamp,
                            event,
                            mcp_sequence: None,
                            semantic_action: None,
                            fallback_sequences: None,
                            enhanced_ui_context: None,
                        }
                    }
                }
            } else {
<<<<<<< HEAD
                // No MCP conversion, create basic recorded event
=======
>>>>>>> 2c7b68c (recorder: restore core features; migrate ButtonClick->ClickEvent; add resolver; retain performance modes; docs: update record_workflow; mcp_converter: remove unused; tests/examples updated; .gitignore: recorder logs, scripts/local/**)
                let timestamp = event.timestamp().unwrap_or_else(|| {
                    std::time::SystemTime::now()
                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64
                });

                crate::events::RecordedEvent {
                    timestamp,
                    event,
                    mcp_sequence: None,
                    semantic_action: None,
                    fallback_sequences: None,
                    enhanced_ui_context: None,
                }
            };

<<<<<<< HEAD
            // Add the event to the workflow (keep lock scope minimal)
=======
>>>>>>> 2c7b68c (recorder: restore core features; migrate ButtonClick->ClickEvent; add resolver; retain performance modes; docs: update record_workflow; mcp_converter: remove unused; tests/examples updated; .gitignore: recorder logs, scripts/local/**)
            if let Ok(mut workflow_guard) = workflow.lock() {
                workflow_guard.add_enhanced_event(recorded_event);
            }
        }
    }
}
