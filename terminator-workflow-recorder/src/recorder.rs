use crate::{RecordedWorkflow, Result, WorkflowEvent, WorkflowRecorderError};
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

// Optional network recorder available when compiled with the "network_capture" feature
#[cfg(feature = "network_capture")]
pub mod network;

#[cfg(target_os = "windows")]
pub use self::windows::*;

#[cfg(feature = "network_capture")]
pub use self::network::*;

/// Performance mode for the workflow recorder
#[derive(Debug, Clone, PartialEq, Default)]
pub enum PerformanceMode {
    /// Default behavior - captures all events with full detail
    #[default]
    Normal,
    /// Moderate optimizations - some filtering and reduced capture frequency
    Balanced,
    /// Aggressive optimizations for weak computers - minimal overhead
    LowEnergy,
}

impl PerformanceMode {
    /// Create a configuration optimized for low-end computers
    ///
    /// # Examples
    ///
    /// ```rust
    /// use terminator_workflow_recorder::{WorkflowRecorderConfig, PerformanceMode};
    ///
    /// let mut config = WorkflowRecorderConfig::default();
    /// config.performance_mode = PerformanceMode::LowEnergy;
    ///
    /// // Or use the helper method for a complete low-energy setup
    /// let low_energy_config = PerformanceMode::low_energy_config();
    /// ```
    pub fn low_energy_config() -> WorkflowRecorderConfig {
        WorkflowRecorderConfig {
            performance_mode: PerformanceMode::LowEnergy,
            max_events_per_second: Some(5),       // Very conservative
            event_processing_delay_ms: Some(100), // 100ms delays
            filter_mouse_noise: true,
            filter_keyboard_noise: true,
            reduce_ui_element_capture: true,
            record_text_input_completion: false, // Disable high-overhead feature
            mouse_move_throttle_ms: 500,         // Very slow mouse tracking
            ..WorkflowRecorderConfig::default()
        }
    }

    /// Create a configuration with balanced performance optimizations
    ///
    /// # Examples
    ///
    /// ```rust
    /// use terminator_workflow_recorder::PerformanceMode;
    ///
    /// let balanced_config = PerformanceMode::balanced_config();
    /// ```
    pub fn balanced_config() -> WorkflowRecorderConfig {
        WorkflowRecorderConfig {
            performance_mode: PerformanceMode::Balanced,
            filter_mouse_noise: true,    // Skip mouse moves/scrolls
            mouse_move_throttle_ms: 200, // Moderate mouse tracking
            ..WorkflowRecorderConfig::default()
        }
    }
}

/// Configuration for the workflow recorder
#[derive(Debug, Clone)]
pub struct WorkflowRecorderConfig {
    /// Whether to record mouse events
    pub record_mouse: bool,

    /// Whether to record keyboard events
    pub record_keyboard: bool,

    /// Whether to capture UI element information
    pub capture_ui_elements: bool,

    /// Whether to record clipboard operations
    pub record_clipboard: bool,

    /// Whether to record hotkey/shortcut events
    pub record_hotkeys: bool,

    /// Whether to record network traffic (requires the "network_capture" cargo feature)
    pub record_network: bool,

    pub record_text_input_completion: bool,

    /// Whether to record high-level application switch events
    pub record_application_switches: bool,

    /// Whether to record high-level browser tab navigation events  
    pub record_browser_tab_navigation: bool,

    /// Minimum time between application switches to consider them separate (milliseconds)
    pub app_switch_dwell_time_threshold_ms: u64,

    /// Timeout for browser URL/title detection after tab actions (milliseconds)
    pub browser_detection_timeout_ms: u64,

    /// Maximum clipboard content length to record (longer content will be truncated)
    pub max_clipboard_content_length: usize,

    /// Whether to track modifier key states accurately
    pub track_modifier_states: bool,

    /// Minimum time between mouse move events to reduce noise (milliseconds)
    pub mouse_move_throttle_ms: u64,

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

    // Performance optimization options
    /// Performance mode controlling overall resource usage and event filtering
    pub performance_mode: PerformanceMode,

    /// Custom delay between event processing cycles (milliseconds)
    /// None uses the performance_mode default
    pub event_processing_delay_ms: Option<u64>,

    /// Rate limiting for events per second
    /// None uses the performance_mode default  
    pub max_events_per_second: Option<u32>,

    /// Skip mouse move and scroll events to reduce noise (keeps clicks)
    pub filter_mouse_noise: bool,

    /// Skip key-down events and non-printable keys to reduce noise
    pub filter_keyboard_noise: bool,

    /// Reduce expensive UI element capture operations
    pub reduce_ui_element_capture: bool,
}

impl Default for WorkflowRecorderConfig {
    fn default() -> Self {
        Self {
            record_mouse: true,
            record_keyboard: true, // TODO not used
            capture_ui_elements: true,
            record_clipboard: true,
            record_hotkeys: true,
            record_network: false,
            record_text_input_completion: true,
            record_application_switches: true, // High-level semantic events, enabled by default
            record_browser_tab_navigation: true, // High-level semantic events, enabled by default
            app_switch_dwell_time_threshold_ms: 100, // 100ms minimum dwell time to record
            browser_detection_timeout_ms: 1000, // 1 second to detect URL/title changes
            max_clipboard_content_length: 10240, // 10KB max
            track_modifier_states: true,
            mouse_move_throttle_ms: 100, // PERFORMANCE: Increased from 50ms to 100ms (10 FPS max for mouse moves)
            min_drag_distance: 5.0,      // 5 pixels minimum for drag detection
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
            performance_mode: PerformanceMode::Normal,
            event_processing_delay_ms: None,
            max_events_per_second: None,
            filter_mouse_noise: false,
            filter_keyboard_noise: false,
            reduce_ui_element_capture: false,
            record_network: false,
        }
    }
}

impl WorkflowRecorderConfig {
    /// Get the effective event processing delay based on performance mode
    pub fn effective_processing_delay_ms(&self) -> u64 {
        if let Some(delay) = self.event_processing_delay_ms {
            return delay;
        }

        match self.performance_mode {
            PerformanceMode::Normal => 0,
            PerformanceMode::Balanced => 25,
            PerformanceMode::LowEnergy => 50,
        }
    }

    /// Get the effective max events per second based on performance mode
    pub fn effective_max_events_per_second(&self) -> Option<u32> {
        if let Some(limit) = self.max_events_per_second {
            return Some(limit);
        }

        match self.performance_mode {
            PerformanceMode::Normal => None,
            PerformanceMode::Balanced => Some(20),
            PerformanceMode::LowEnergy => Some(10),
        }
    }

    /// Check if mouse noise filtering should be enabled
    pub fn should_filter_mouse_noise(&self) -> bool {
        self.filter_mouse_noise
            || matches!(
                self.performance_mode,
                PerformanceMode::Balanced | PerformanceMode::LowEnergy
            )
    }

    /// Check if keyboard noise filtering should be enabled  
    pub fn should_filter_keyboard_noise(&self) -> bool {
        self.filter_keyboard_noise || matches!(self.performance_mode, PerformanceMode::LowEnergy)
    }

    /// Check if UI element capture should be reduced
    pub fn should_reduce_ui_capture(&self) -> bool {
        self.reduce_ui_element_capture
            || matches!(
                self.performance_mode,
                PerformanceMode::Balanced | PerformanceMode::LowEnergy
            )
    }
}

/// The workflow recorder
pub struct WorkflowRecorder {
    /// The recorded workflow
    pub workflow: Arc<Mutex<RecordedWorkflow>>,

    /// The event sender
    event_tx: broadcast::Sender<WorkflowEvent>,

    /// The configuration
    config: WorkflowRecorderConfig,

    /// The platform-specific recorder
    #[cfg(target_os = "windows")]
    windows_recorder: Option<WindowsRecorder>,

    /// Optional network recorder (enabled with the "network_capture" feature)
    #[cfg(feature = "network_capture")]
    network_recorder: Option<network::NetworkRecorder>,
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
            #[cfg(feature = "network_capture")]
            network_recorder: None,
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

            // Start network recorder if enabled in config and feature is active
            #[cfg(feature = "network_capture")]
            if self.config.record_network {
                let net_recorder = network::NetworkRecorder::new(self.event_tx.clone()).await?;
                self.network_recorder = Some(net_recorder);
            }

            // Start the event processing task
            let event_rx = self.event_tx.subscribe();
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

            // Stop network recorder if running
            #[cfg(feature = "network_capture")]
            if let Some(net_recorder) = self.network_recorder.take() {
                net_recorder.stop().await?;
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
    ) {
        while let Ok(event) = event_rx.recv().await {
            if let Ok(mut workflow) = workflow.lock() {
                workflow.add_event(event);
            }
        }
    }
}
