use crate::events::{ButtonClickEvent, ButtonInteractionType};
use crate::{
    ApplicationSwitchEvent, ApplicationSwitchMethod, BrowserTabNavigationEvent, ClipboardAction,
    ClipboardEvent, EventMetadata, HotkeyEvent, KeyboardEvent, MouseButton, MouseEvent,
    MouseEventType, Position, Result, TabAction, TabNavigationMethod, TextInputCompletedEvent,
    TextInputMethod, WorkflowEvent, WorkflowRecorderConfig,
};
use arboard::Clipboard;
use rdev::{Button, EventType, Key};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant, SystemTime},
};
use terminator::{convert_uiautomation_element_to_terminator, UIElement};
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};
use uiautomation::UIAutomation;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Com::{
    CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED,
};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, PostThreadMessageW, TranslateMessage, MSG, WM_QUIT,
};

/// The Windows-specific recorder
pub struct WindowsRecorder {
    /// The event sender
    event_tx: broadcast::Sender<WorkflowEvent>,

    /// The configuration
    config: WorkflowRecorderConfig,

    /// The last mouse position
    last_mouse_pos: Arc<Mutex<Option<(i32, i32)>>>,

    /// Signal to stop the listener thread
    stop_indicator: Arc<AtomicBool>,

    /// Modifier key states
    modifier_states: Arc<Mutex<ModifierStates>>,

    /// Last clipboard content hash for change detection
    last_clipboard_hash: Arc<Mutex<Option<u64>>>,

    /// Last mouse move time for throttling
    last_mouse_move_time: Arc<Mutex<Instant>>,

    /// Known hotkey patterns
    hotkey_patterns: Arc<Vec<HotkeyPattern>>,

    /// UI Automation thread ID for proper cleanup
    ui_automation_thread_id: Arc<Mutex<Option<u32>>>,

    /// Current typing session tracking
    current_typing_session: Arc<AtomicTypingSession>,

    /// Current application tracking for switch detection
    current_application: Arc<Mutex<Option<ApplicationState>>>,

    /// Browser tab navigation tracking
    browser_tab_tracker: Arc<Mutex<BrowserTabTracker>>,
}

#[derive(Debug, Clone)]
struct ModifierStates {
    ctrl: bool,
    alt: bool,
    shift: bool,
    win: bool,
}

#[derive(Debug, Clone)]
struct HotkeyPattern {
    action: String,
    keys: Vec<u32>,
}

/// Tracks the current application state for switch detection
#[derive(Debug, Clone)]
struct ApplicationState {
    /// Application name/title
    name: String,
    /// Process ID
    process_id: u32,
    /// When the application became active
    start_time: Instant,
}

/// Tracks browser tab navigation state
#[derive(Debug, Clone)]
struct BrowserTabTracker {
    /// Current browser application
    current_browser: Option<String>,
    /// Current URL (best effort detection)
    current_url: Option<String>,
    /// Previous URL for navigation tracking
    previous_url: Option<String>,
    /// Current page title
    current_title: Option<String>,
    /// When the current page was last accessed
    last_navigation_time: Option<Instant>,
    /// Known browser process names
    known_browsers: Vec<String>,
}

impl Default for BrowserTabTracker {
    fn default() -> Self {
        Self {
            current_browser: None,
            current_url: None,
            previous_url: None,
            current_title: None,
            last_navigation_time: None,
            known_browsers: vec![
                // Executable names
                "chrome.exe".to_string(),
                "firefox.exe".to_string(),
                "msedge.exe".to_string(),
                "brave.exe".to_string(),
                "opera.exe".to_string(),
                "vivaldi.exe".to_string(),
                "iexplore.exe".to_string(),
                // Display names
                "google chrome".to_string(),
                "chrome".to_string(),
                "firefox".to_string(),
                "mozilla firefox".to_string(),
                "microsoft edge".to_string(),
                "edge".to_string(),
                "brave".to_string(),
                "opera".to_string(),
                "vivaldi".to_string(),
                "internet explorer".to_string(),
            ],
        }
    }
}

/// Simple atomic-based typing session for zero-contention keystroke processing
struct AtomicTypingSession {
    /// Whether a typing session is currently active
    is_active: AtomicBool,
    /// Number of keystrokes in current session
    keystroke_count: AtomicU32,
    /// Start time of session (nanoseconds since epoch)
    start_time_nanos: AtomicU64,
    /// Last keystroke time (nanoseconds since epoch)  
    last_keystroke_nanos: AtomicU64,
}

impl Default for AtomicTypingSession {
    fn default() -> Self {
        Self {
            is_active: AtomicBool::new(false),
            keystroke_count: AtomicU32::new(0),
            start_time_nanos: AtomicU64::new(0),
            last_keystroke_nanos: AtomicU64::new(0),
        }
    }
}

impl AtomicTypingSession {
    /// Record a keystroke - completely lock-free and fast
    fn record_keystroke(&self) {
        let now_nanos = self.nanos_since_epoch();

        if !self.is_active.load(Ordering::Relaxed) {
            // Start new session
            self.start_time_nanos.store(now_nanos, Ordering::Relaxed);
            self.keystroke_count.store(1, Ordering::Relaxed);
            self.is_active.store(true, Ordering::Relaxed);
        } else {
            // Update existing session
            self.keystroke_count.fetch_add(1, Ordering::Relaxed);
        }

        self.last_keystroke_nanos
            .store(now_nanos, Ordering::Relaxed);
    }

    /// Check if session should timeout (lock-free read)
    fn should_timeout(&self, timeout_ms: u64) -> bool {
        if !self.is_active.load(Ordering::Relaxed) {
            return false;
        }

        let last_keystroke = self.last_keystroke_nanos.load(Ordering::Relaxed);
        let now = self.nanos_since_epoch();
        let elapsed_ms = (now - last_keystroke) / 1_000_000; // Convert nanos to millis

        elapsed_ms > timeout_ms
    }

    /// Complete and clear the session atomically
    fn complete_session(&self, force: bool, timeout_ms: u64) -> Option<(u32, u64)> {
        if !self.is_active.load(Ordering::Relaxed) {
            return None;
        }

        if !force && !self.should_timeout(timeout_ms) {
            return None;
        }

        // Atomically extract session data and mark inactive
        let keystroke_count = self.keystroke_count.load(Ordering::Relaxed);
        let start_time = self.start_time_nanos.load(Ordering::Relaxed);
        let end_time = self.last_keystroke_nanos.load(Ordering::Relaxed);

        // Mark session as inactive
        self.is_active.store(false, Ordering::Relaxed);

        let duration_ms = (end_time - start_time) / 1_000_000;
        Some((keystroke_count, duration_ms))
    }

    fn nanos_since_epoch(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }
}

impl WindowsRecorder {
    /// Capture the current timestamp in milliseconds since epoch
    fn capture_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// Creates a UIAutomation instance with the configured threading model for a new thread.
    fn create_configured_automation_instance(
        config: &WorkflowRecorderConfig,
    ) -> std::result::Result<UIAutomation, String> {
        unsafe {
            let threading_model = if config.enable_multithreading {
                COINIT_MULTITHREADED
            } else {
                COINIT_APARTMENTTHREADED
            };
            let hr = CoInitializeEx(None, threading_model);
            if hr.is_err() && hr != windows::Win32::Foundation::RPC_E_CHANGED_MODE {
                let err_msg = format!("Failed to initialize COM for new thread: {:?}", hr);
                error!("{}", err_msg);
                return Err(err_msg);
            }
        }
        UIAutomation::new_direct().map_err(|e| {
            let err_msg = format!("Failed to create UIAutomation instance directly: {}", e);
            error!("{}", err_msg);
            err_msg
        })
    }

    /// Create a new Windows recorder
    pub async fn new(
        config: WorkflowRecorderConfig,
        event_tx: broadcast::Sender<WorkflowEvent>,
    ) -> Result<Self> {
        info!("Initializing comprehensive Windows recorder");
        debug!("Recorder config: {:?}", config);

        let last_mouse_pos = Arc::new(Mutex::new(None));
        let stop_indicator = Arc::new(AtomicBool::new(false));
        let modifier_states = Arc::new(Mutex::new(ModifierStates {
            ctrl: false,
            alt: false,
            shift: false,
            win: false,
        }));
        let last_clipboard_hash = Arc::new(Mutex::new(None));
        let last_mouse_move_time = Arc::new(Mutex::new(Instant::now()));

        // Initialize hotkey patterns
        let hotkey_patterns = Arc::new(Self::initialize_hotkey_patterns());

        let mut recorder = Self {
            event_tx,
            config,
            last_mouse_pos,
            stop_indicator,
            modifier_states,
            last_clipboard_hash,
            last_mouse_move_time,
            hotkey_patterns,
            ui_automation_thread_id: Arc::new(Mutex::new(None)),
            current_typing_session: Arc::new(AtomicTypingSession::default()),
            current_application: Arc::new(Mutex::new(None)),
            browser_tab_tracker: Arc::new(Mutex::new(BrowserTabTracker::default())),
        };

        let handle = tokio::runtime::Handle::current();

        // Set up comprehensive event listeners
        recorder.setup_comprehensive_listeners(handle).await?;

        // Start background typing session completion timer for reliable text completion
        if recorder.config.record_text_input_completion {
            recorder.start_typing_completion_timer();
        }

        Ok(recorder)
    }

    /// Format UI Automation property values properly for JSON output
    fn format_property_value(value: &uiautomation::variants::Variant) -> Option<String> {
        // First try to get as string
        if let Ok(s) = value.get_string() {
            if !s.is_empty() {
                return Some(s);
            } else {
                return None; // Empty string - don't include
            }
        }

        // Try to handle other important types without using debug format
        // Note: We avoid using try_into or debug format to prevent artifacts like "BOOL(false)"

        // For boolean values, we'll skip them for now to avoid clutter
        // since most boolean property changes (like HasKeyboardFocus) create noise

        // For numeric values, we could add handling here if needed in the future
        // but for now we'll keep it simple and only include meaningful strings

        // If we can't get a meaningful string value, skip this property
        None
    }

    /// Check if this keystroke should count towards typing (printable characters and common editing keys)
    /// Note: Tab (0x09) is not included here as it's handled separately as a session completion trigger
    fn is_typing_keystroke(key_code: u32, character: Option<char>) -> bool {
        // Printable characters
        if character.is_some() && character != Some('\0') {
            return true;
        }

        // Common editing keys
        matches!(
            key_code,
            0x08 | // Backspace
            0x2E | // Delete
            0x20 | // Space
            0x0D // Enter
        )
    }

    /// Determine the likely input method based on session characteristics
    fn determine_input_method(
        keystroke_count: u32,
        duration_ms: u64,
        had_paste: bool,
    ) -> TextInputMethod {
        if had_paste {
            if keystroke_count > 5 {
                TextInputMethod::Mixed
            } else {
                TextInputMethod::Pasted
            }
        } else if duration_ms < 100 && keystroke_count > 10 {
            // Very fast typing of many characters suggests auto-fill
            TextInputMethod::AutoFilled
        } else {
            TextInputMethod::Typed
        }
    }

    /// Check for application switch and emit event if detected
    fn check_and_emit_application_switch(
        current_app: &Arc<Mutex<Option<ApplicationState>>>,
        event_tx: &broadcast::Sender<WorkflowEvent>,
        new_element: &Option<UIElement>,
        switch_method: ApplicationSwitchMethod,
        config: &WorkflowRecorderConfig,
    ) {
        if !config.record_application_switches {
            return;
        }

        if let Some(element) = new_element {
            let app_name = element.application_name();
            if let Ok(process_id) = element.process_id() {
                if !app_name.is_empty() {
                    let mut current = current_app.lock().unwrap();

                    // Check if this is a new application
                    let is_switch = if let Some(ref current_state) = *current {
                        current_state.process_id != process_id || current_state.name != app_name
                    } else {
                        true // First app detection
                    };

                    if is_switch {
                        let now = Instant::now();

                        // Calculate dwell time for previous app
                        let dwell_time = if let Some(ref current_state) = *current {
                            let duration = now.duration_since(current_state.start_time);
                            if duration.as_millis()
                                >= config.app_switch_dwell_time_threshold_ms as u128
                            {
                                Some(duration.as_millis() as u64)
                            } else {
                                None // Too short, probably just UI noise
                            }
                        } else {
                            None
                        };

                        // Only emit if we have meaningful dwell time or this is first app
                        if dwell_time.is_some() || current.is_none() {
                            let switch_event = ApplicationSwitchEvent {
                                from_application: current.as_ref().map(|s| s.name.clone()),
                                to_application: app_name.clone(),
                                from_process_id: current.as_ref().map(|s| s.process_id),
                                to_process_id: process_id,
                                switch_method,
                                dwell_time_ms: dwell_time,
                                switch_count: None, // TODO: Track Alt+Tab cycles
                                metadata: EventMetadata::with_ui_element_and_timestamp(Some(
                                    element.clone(),
                                )),
                            };

                            if let Err(e) =
                                event_tx.send(WorkflowEvent::ApplicationSwitch(switch_event))
                            {
                                debug!("Failed to send application switch event: {}", e);
                            }
                        }

                        // Update current application state
                        *current = Some(ApplicationState {
                            name: app_name,
                            process_id,
                            start_time: now,
                        });
                    }
                }
            }
        }
    }

    /// Check and emit browser navigation events with improved filtering
    fn check_and_emit_browser_navigation_with_url(
        tracker: &Arc<Mutex<BrowserTabTracker>>,
        event_tx: &broadcast::Sender<WorkflowEvent>,
        ui_element: &Option<UIElement>,
        method: TabNavigationMethod,
        config: &WorkflowRecorderConfig,
        url_override: Option<String>,
    ) {
        if !config.record_browser_tab_navigation {
            return;
        }

        if let Some(ref element) = ui_element {
            let app_name = element.application_name();

            // Check if this is a browser
            let tracker_guard = tracker.lock().unwrap();
            let is_browser = tracker_guard
                .known_browsers
                .iter()
                .any(|browser| app_name.to_lowercase().contains(&browser.to_lowercase()));
            drop(tracker_guard);

            if is_browser {
                let window_title = element.window_title();
                let element_name = element.name_or_empty();

                // Try to extract URL and title from various UI elements
                let (mut detected_url, detected_title) =
                    Self::extract_browser_info(&window_title, &element_name, &element.role());

                // Use URL override if provided
                if let Some(override_url) = url_override {
                    detected_url = Some(override_url);
                }

                // Additional check: if element_name looks like a URL, use it directly
                if detected_url.is_none()
                    && element_name.starts_with("http")
                    && element_name.len() > 10
                {
                    detected_url = Some(element_name.clone());
                }

                // Apply improved filtering
                if !Self::should_emit_browser_navigation(ui_element, &detected_url, &detected_title)
                {
                    debug!("Filtering out browser navigation event - not a real navigation");
                    return;
                }

                let mut tracker_guard = tracker.lock().unwrap();

                // Check if this is the first time we're detecting this browser or if there's a meaningful change
                let is_first_detection = tracker_guard.current_browser.is_none()
                    || tracker_guard.current_browser.as_ref() != Some(&app_name);

                let has_url_change = detected_url.is_some()
                    && detected_url.as_ref() != tracker_guard.current_url.as_ref();

                let has_title_change = detected_title.is_some()
                    && detected_title.as_ref() != tracker_guard.current_title.as_ref();

                // Only emit event if we have meaningful data and either:
                // 1. This is the first detection of a browser
                // 2. There's a URL change
                // 3. There's a title change (new page)
                if is_first_detection || has_url_change || has_title_change {
                    // Calculate time spent on previous page
                    let page_dwell_time_ms = tracker_guard
                        .last_navigation_time
                        .map(|last_time| last_time.elapsed().as_millis() as u64);

                    // Determine the action based on context
                    let action = if is_first_detection || tracker_guard.current_browser.is_none() {
                        TabAction::Created
                    } else {
                        TabAction::Switched // Default for other cases
                    };

                    let event = BrowserTabNavigationEvent {
                        action,
                        method: method.clone(),
                        url: detected_url.clone(),
                        previous_url: tracker_guard.current_url.clone(),
                        title: detected_title.clone(),
                        browser: app_name.clone(),
                        tab_index: None, // Tab index detection would require more complex logic
                        total_tabs: None, // Total tabs detection would require more complex logic
                        page_dwell_time_ms,
                        is_back_forward: false, // Back/forward detection would require more complex logic
                        metadata: EventMetadata::with_ui_element_and_timestamp(ui_element.clone()),
                    };

                    // Update tracker state
                    tracker_guard.current_browser = Some(app_name);
                    if let Some(url) = detected_url {
                        tracker_guard.previous_url = tracker_guard.current_url.clone();
                        tracker_guard.current_url = Some(url);
                    }
                    if let Some(title) = detected_title {
                        tracker_guard.current_title = Some(title);
                    }
                    tracker_guard.last_navigation_time = Some(Instant::now());

                    drop(tracker_guard);

                    // Send the event
                    if let Err(e) = event_tx.send(WorkflowEvent::BrowserTabNavigation(event)) {
                        debug!("Failed to send browser navigation event: {}", e);
                    }
                } else {
                    debug!("Skipping browser navigation event - no meaningful change detected");
                }
            }
        }
    }

    /// Extract URL and title from browser UI elements (best effort)
    fn extract_browser_info(
        window_title: &str,
        element_name: &str,
        element_role: &str,
    ) -> (Option<String>, Option<String>) {
        let mut url = None;
        let mut title = None;

        // Try to extract URL from element name if it looks like a URL
        if element_name.starts_with("http") && element_name.len() > 10 {
            url = Some(element_name.to_string());
        }

        // Try to extract URL from address bar elements
        if (element_role.to_lowercase().contains("address")
            || element_role.to_lowercase().contains("location")
            || element_role.to_lowercase().contains("edit"))
            && element_name.starts_with("http")
            && element_name.len() > 10
        {
            url = Some(element_name.to_string());
        }

        // Extract title from window title (format: "Page Title - Browser Name")
        if !window_title.is_empty() {
            // Common browser title patterns
            let separators = [" - ", " — ", " – ", " | "];
            for sep in &separators {
                if let Some(pos) = window_title.rfind(sep) {
                    let potential_title = &window_title[..pos];
                    if !potential_title.is_empty() && potential_title.len() > 3 {
                        title = Some(potential_title.to_string());
                        break;
                    }
                }
            }

            // If no separator found, use the whole window title if it's reasonable
            if title.is_none() && window_title.len() > 3 && window_title.len() < 200 {
                title = Some(window_title.to_string());
            }
        }

        // If we still don't have a title, try using element_name as title (for tab titles)
        if title.is_none() && !element_name.is_empty() && !element_name.starts_with("http") {
            // Remove common browser suffixes
            let cleaned_name = element_name
                .replace(" - Google Chrome", "")
                .replace(" - Microsoft Edge", "")
                .replace(" - Mozilla Firefox", "")
                .replace(" - Chrome", "")
                .replace(" - Edge", "")
                .replace(" - Firefox", "");

            if !cleaned_name.trim().is_empty() && cleaned_name.len() > 3 {
                title = Some(cleaned_name.trim().to_string());
            }
        }

        (url, title)
    }

    /// Process potential typing session completion with UI element capture
    fn check_and_complete_typing_session(
        automation: &UIAutomation,
        current_session: &Arc<AtomicTypingSession>,
        event_tx: &broadcast::Sender<WorkflowEvent>,
        force_complete: bool,
        timeout_ms: u64,
    ) {
        let session_data = current_session.complete_session(force_complete, timeout_ms);

        if let Some((keystroke_count, duration_ms)) = session_data {
            // NOW capture the UI element and text - only when session completes (no contention!)
            let (ui_element, text_value, field_name, field_type) = if let Some(focused_element) =
                Self::get_focused_ui_element_with_timeout(automation)
            {
                // Try to get the current text value from the UI element
                let text = focused_element.text(1).unwrap_or_else(|_| {
                    // Fallback: try to get name if text fails
                    focused_element.name().unwrap_or_default()
                });

                let field_name = focused_element.name().unwrap_or_default();
                let field_type = focused_element.role();

                (Some(focused_element), text, Some(field_name), field_type)
            } else {
                (None, String::new(), None, "unknown".to_string())
            };

            // Only emit event if we got meaningful text or have multiple keystrokes
            if !text_value.trim().is_empty() || keystroke_count > 1 {
                let input_method = Self::determine_input_method(
                    keystroke_count,
                    duration_ms,
                    false, // had_paste is not available in the AtomicTypingSession
                );

                let text_input_event = TextInputCompletedEvent {
                    text_value,
                    field_name,
                    field_type,
                    input_method,
                    typing_duration_ms: duration_ms,
                    keystroke_count,
                    metadata: EventMetadata {
                        ui_element,
                        timestamp: Some(Self::capture_timestamp()),
                    },
                };

                let _ = event_tx.send(WorkflowEvent::TextInputCompleted(text_input_event));
            }
        }
    }

    /// Initialize common hotkey patterns
    fn initialize_hotkey_patterns() -> Vec<HotkeyPattern> {
        vec![
            HotkeyPattern {
                action: "Copy".to_string(),
                keys: vec![162, 67], // Ctrl + C
            },
            HotkeyPattern {
                action: "Paste".to_string(),
                keys: vec![162, 86], // Ctrl + V
            },
            HotkeyPattern {
                action: "Cut".to_string(),
                keys: vec![162, 88], // Ctrl + X
            },
            HotkeyPattern {
                action: "Undo".to_string(),
                keys: vec![162, 90], // Ctrl + Z
            },
            HotkeyPattern {
                action: "Redo".to_string(),
                keys: vec![162, 89], // Ctrl + Y
            },
            HotkeyPattern {
                action: "Save".to_string(),
                keys: vec![162, 83], // Ctrl + S
            },
            HotkeyPattern {
                action: "Switch Window".to_string(),
                keys: vec![164, 9], // Alt + Tab
            },
            HotkeyPattern {
                action: "Show Desktop".to_string(),
                keys: vec![91, 68], // Win + D
            },
            HotkeyPattern {
                action: "Task Manager".to_string(),
                keys: vec![162, 160, 27], // Ctrl + Shift + Esc
            },
        ]
    }

    /// Set up comprehensive event listeners
    async fn setup_comprehensive_listeners(
        &mut self,
        handle: tokio::runtime::Handle,
    ) -> Result<()> {
        // Main input event listener (enhanced from original)
        self.setup_enhanced_input_listener().await?;

        // Clipboard monitoring
        if self.config.record_clipboard {
            self.setup_clipboard_monitor()?;
        }

        // UI Automation event monitoring
        self.setup_ui_automation_events(
            Arc::clone(&self.current_application),
            Arc::clone(&self.browser_tab_tracker),
            handle,
        )?;

        Ok(())
    }

    /// Set up enhanced input event listener
    async fn setup_enhanced_input_listener(&mut self) -> Result<()> {
        let event_tx = self.event_tx.clone();
        let last_mouse_pos = Arc::clone(&self.last_mouse_pos);
        let capture_ui_elements = self.config.capture_ui_elements;
        let stop_indicator_clone = Arc::clone(&self.stop_indicator);
        let modifier_states = Arc::clone(&self.modifier_states);
        let last_mouse_move_time = Arc::clone(&self.last_mouse_move_time);
        let hotkey_patterns = Arc::clone(&self.hotkey_patterns);
        let config = self.config.clone();
        let current_typing_session = Arc::clone(&self.current_typing_session);

        thread::spawn(move || {
            let track_modifiers = config.track_modifier_states;
            let record_hotkeys = config.record_hotkeys;
            let record_text_input_completion = config.record_text_input_completion;
            let mouse_move_throttle = config.mouse_move_throttle_ms;
            // PERFORMANCE: Create UIAutomation instance once outside the event loop
            let automation = if capture_ui_elements {
                match Self::create_configured_automation_instance(&config) {
                    Ok(auto) => {
                        info!("✅ UIAutomation instance created for input events with configured threading model");
                        Some(auto)
                    }
                    Err(e) => {
                        warn!("⚠️  Failed to create UIAutomation for input events: {}. UI context will be missing.", e);
                        None
                    }
                }
            } else {
                None
            };

            let mut active_keys: HashMap<u32, bool> = HashMap::new();

            if let Err(error) = rdev::listen(move |event: rdev::Event| {
                if stop_indicator_clone.load(Ordering::SeqCst) {
                    return;
                }

                match event.event_type {
                    EventType::KeyPress(key) => {
                        let key_code = key_to_u32(&key);
                        active_keys.insert(key_code, true);

                        // Update modifier states
                        if track_modifiers {
                            Self::update_modifier_states(&modifier_states, key_code, true);
                        }

                        // Check for hotkeys
                        if record_hotkeys {
                            if let Some(hotkey) =
                                Self::detect_hotkey(&hotkey_patterns, &active_keys)
                            {
                                let _ = event_tx.send(WorkflowEvent::Hotkey(hotkey));
                            }
                        }

                        let modifiers = if track_modifiers {
                            modifier_states.lock().unwrap().clone()
                        } else {
                            ModifierStates {
                                ctrl: false,
                                alt: false,
                                shift: false,
                                win: false,
                            }
                        };

                        let character = if (32..=126).contains(&key_code) {
                            Some(key_code as u8 as char)
                        } else {
                            None
                        };

                        // PERFORMANCE OPTIMIZATION: Only capture UI elements for high-level keyboard events
                        // Skip UI element capture for individual typing keystrokes - only capture for:
                        // 1. Non-typing keys (function keys, shortcuts, etc.)
                        // 2. Completed typing sessions (handled in TextInputCompletedEvent)
                        let mut ui_element = None;
                        if capture_ui_elements {
                            let is_typing_keystroke =
                                Self::is_typing_keystroke(key_code, character);

                            // Only capture UI element for non-typing keystrokes (shortcuts, function keys, etc.)
                            if !is_typing_keystroke {
                                ui_element = Self::get_focused_ui_element_with_timeout(
                                    automation.as_ref().unwrap(),
                                );
                            }
                            // For typing keystrokes: UI element will be captured when typing session completes
                        }

                        // Handle typing session tracking for text input completion
                        if record_text_input_completion
                            && Self::is_typing_keystroke(key_code, character)
                        {
                            // PERFORMANCE OPTIMIZATION: ZERO-CONTENTION keystroke tracking
                            // Uses atomic operations instead of mutex - no blocking!
                            current_typing_session.record_keystroke();

                            // Special case: Enter key should immediately complete the typing session
                            // This captures common form submission patterns
                            if key_code == 0x0D {
                                if let Some(ref auto) = automation {
                                    // Enter key
                                    Self::check_and_complete_typing_session(
                                        auto,
                                        &current_typing_session,
                                        &event_tx,
                                        true, // force complete on Enter
                                        config.text_input_completion_timeout_ms,
                                    );
                                }
                            }
                        }

                        // Special case: Tab key should also complete typing session
                        // This captures text input when user tabs to next field
                        if record_text_input_completion && key_code == 0x09 {
                            if let Some(ref auto) = automation {
                                // Tab key - complete session before focus changes
                                Self::check_and_complete_typing_session(
                                    auto,
                                    &current_typing_session,
                                    &event_tx,
                                    true, // force complete on Tab
                                    config.text_input_completion_timeout_ms,
                                );
                            }
                        }

                        let keyboard_event = KeyboardEvent {
                            key_code,
                            is_key_down: true,
                            ctrl_pressed: modifiers.ctrl,
                            alt_pressed: modifiers.alt,
                            shift_pressed: modifiers.shift,
                            win_pressed: modifiers.win,
                            character,
                            scan_code: None, // TODO: Get actual scan code
                            metadata: EventMetadata {
                                ui_element,
                                timestamp: Some(Self::capture_timestamp()),
                            },
                        };

                        let _ = event_tx.send(WorkflowEvent::Keyboard(keyboard_event));
                    }
                    EventType::KeyRelease(key) => {
                        let key_code = key_to_u32(&key);
                        active_keys.remove(&key_code);

                        if track_modifiers {
                            Self::update_modifier_states(&modifier_states, key_code, false);
                        }

                        let modifiers = if track_modifiers {
                            modifier_states.lock().unwrap().clone()
                        } else {
                            ModifierStates {
                                ctrl: false,
                                alt: false,
                                shift: false,
                                win: false,
                            }
                        };

                        // PERFORMANCE OPTIMIZATION: Only capture UI elements for high-level keyboard events
                        // Skip UI element capture for typing keystrokes during key release
                        let mut ui_element = None;
                        if capture_ui_elements {
                            let character = if (32..=126).contains(&key_code) {
                                Some(key_code as u8 as char)
                            } else {
                                None
                            };
                            let is_typing_keystroke =
                                Self::is_typing_keystroke(key_code, character);

                            // Only capture UI element for non-typing keystrokes
                            if !is_typing_keystroke {
                                ui_element = Self::get_focused_ui_element_with_timeout(
                                    automation.as_ref().unwrap(),
                                );
                            }
                        }

                        let keyboard_event = KeyboardEvent {
                            key_code,
                            is_key_down: false,
                            ctrl_pressed: modifiers.ctrl,
                            alt_pressed: modifiers.alt,
                            shift_pressed: modifiers.shift,
                            win_pressed: modifiers.win,
                            character: None,
                            scan_code: None,
                            metadata: EventMetadata {
                                ui_element,
                                timestamp: Some(Self::capture_timestamp()),
                            },
                        };
                        let _ = event_tx.send(WorkflowEvent::Keyboard(keyboard_event));
                    }
                    EventType::ButtonPress(button) => {
                        if let Some((x, y)) = *last_mouse_pos.lock().unwrap() {
                            let mouse_button = match button {
                                Button::Left => MouseButton::Left,
                                Button::Right => MouseButton::Right,
                                Button::Middle => MouseButton::Middle,
                                _ => return,
                            };

                            // Complete any active typing session on mouse click (likely focus change)
                            if record_text_input_completion {
                                if let Some(ref auto) = automation {
                                    Self::check_and_complete_typing_session(
                                        auto,
                                        &current_typing_session,
                                        &event_tx,
                                        true, // force complete on click
                                        config.text_input_completion_timeout_ms,
                                    );
                                }
                            }

                            let mut ui_element = None;
                            if capture_ui_elements {
                                ui_element = Self::get_focused_ui_element_with_timeout(
                                    automation.as_ref().unwrap(),
                                );

                                // Debug: Log what UI element we captured at mouse down
                                if let Some(ref element) = ui_element {
                                    debug!(
                                        "Mouse down captured element: name='{}', role='{}', position=({}, {})",
                                        element.name_or_empty(),
                                        element.role(),
                                        x, y
                                    );
                                } else {
                                    debug!(
                                        "Mouse down: No UI element captured at position ({}, {})",
                                        x, y
                                    );
                                }
                            }

                            // No need to store pending clicks - we'll use focus events

                            let mouse_event = MouseEvent {
                                event_type: MouseEventType::Down,
                                button: mouse_button,
                                position: Position { x, y },
                                scroll_delta: None,
                                drag_start: None,
                                metadata: EventMetadata {
                                    ui_element,
                                    timestamp: Some(Self::capture_timestamp()),
                                },
                            };
                            let _ = event_tx.send(WorkflowEvent::Mouse(mouse_event));
                        }
                    }
                    EventType::ButtonRelease(button) => {
                        if let Some((x, y)) = *last_mouse_pos.lock().unwrap() {
                            let mouse_button = match button {
                                Button::Left => MouseButton::Left,
                                Button::Right => MouseButton::Right,
                                Button::Middle => MouseButton::Middle,
                                _ => return,
                            };

                            let mut ui_element = None;
                            if capture_ui_elements {
                                ui_element = Self::get_focused_ui_element_with_timeout(
                                    automation.as_ref().unwrap(),
                                );
                            }

                            let mouse_event = MouseEvent {
                                event_type: MouseEventType::Up,
                                button: mouse_button,
                                position: Position { x, y },
                                scroll_delta: None,
                                drag_start: None,
                                metadata: EventMetadata {
                                    ui_element,
                                    timestamp: Some(Self::capture_timestamp()),
                                },
                            };
                            let _ = event_tx.send(WorkflowEvent::Mouse(mouse_event));
                        }
                    }
                    EventType::MouseMove { x, y } => {
                        let x = x as i32;
                        let y = y as i32;

                        // Throttle mouse moves
                        let now = Instant::now();
                        let should_record = {
                            let mut last_time = last_mouse_move_time.lock().unwrap();
                            if now.duration_since(*last_time).as_millis()
                                >= mouse_move_throttle as u128
                            {
                                *last_time = now;
                                true
                            } else {
                                false
                            }
                        };
                        let mut ui_element = None;
                        if capture_ui_elements {
                            ui_element = Self::get_focused_ui_element_with_timeout(
                                automation.as_ref().unwrap(),
                            );
                        }

                        *last_mouse_pos.lock().unwrap() = Some((x, y));

                        if should_record {
                            let mouse_event = MouseEvent {
                                event_type: MouseEventType::Move,
                                button: MouseButton::Left,
                                position: Position { x, y },
                                scroll_delta: None,
                                drag_start: None,
                                metadata: EventMetadata {
                                    ui_element,
                                    timestamp: Some(Self::capture_timestamp()),
                                },
                            };
                            let _ = event_tx.send(WorkflowEvent::Mouse(mouse_event));
                        }
                    }
                    EventType::Wheel { delta_x, delta_y } => {
                        if let Some((x, y)) = *last_mouse_pos.lock().unwrap() {
                            let mut ui_element = None;
                            if capture_ui_elements {
                                ui_element = Self::get_focused_ui_element_with_timeout(
                                    automation.as_ref().unwrap(),
                                );
                            }

                            let mouse_event = MouseEvent {
                                event_type: MouseEventType::Wheel,
                                button: MouseButton::Middle,
                                position: Position { x, y },
                                scroll_delta: Some((delta_x as i32, delta_y as i32)),
                                drag_start: None,
                                metadata: EventMetadata {
                                    ui_element,
                                    timestamp: Some(Self::capture_timestamp()),
                                },
                            };
                            let _ = event_tx.send(WorkflowEvent::Mouse(mouse_event));
                        }
                    }
                }
            }) {
                error!("Failed to listen for events: {:?}", error);
            }
        });

        Ok(())
    }

    /// Update modifier key states
    fn update_modifier_states(states: &Arc<Mutex<ModifierStates>>, key_code: u32, pressed: bool) {
        let mut states = states.lock().unwrap();
        match key_code {
            162 | 163 => states.ctrl = pressed,  // Left/Right Ctrl
            164 | 165 => states.alt = pressed,   // Left/Right Alt
            160 | 161 => states.shift = pressed, // Left/Right Shift
            91 | 92 => states.win = pressed,     // Left/Right Win
            _ => {}
        }
    }

    /// Detect hotkey combinations
    fn detect_hotkey(
        patterns: &[HotkeyPattern],
        active_keys: &HashMap<u32, bool>,
    ) -> Option<HotkeyEvent> {
        for pattern in patterns {
            if pattern
                .keys
                .iter()
                .all(|&key| active_keys.get(&key).copied().unwrap_or(false))
            {
                return Some(HotkeyEvent {
                    combination: format!("{:?}", pattern.keys), // TODO: Format properly
                    action: Some(pattern.action.clone()),
                    is_global: true,
                    metadata: EventMetadata {
                        ui_element: None,
                        timestamp: Some(Self::capture_timestamp()),
                    }, // TODO: Pass UI element context from caller
                });
            }
        }
        None
    }

    /// Set up clipboard monitoring
    fn setup_clipboard_monitor(&self) -> Result<()> {
        let event_tx = self.event_tx.clone();
        let stop_indicator = Arc::clone(&self.stop_indicator);
        let last_hash = Arc::clone(&self.last_clipboard_hash);
        let config = self.config.clone();
        let capture_ui_elements = self.config.capture_ui_elements;

        thread::spawn(move || {
            let mut clipboard = match Clipboard::new() {
                Ok(cb) => cb,
                Err(e) => {
                    error!("Failed to initialize clipboard: {}", e);
                    return;
                }
            };

            // Use configured automation instance
            let automation = if capture_ui_elements {
                match Self::create_configured_automation_instance(&config) {
                    Ok(auto) => Some(auto),
                    Err(e) => {
                        warn!("⚠️ Failed to create UIAutomation for clipboard monitor: {}. UI context will be missing.", e);
                        None
                    }
                }
            } else {
                None
            };

            // Initialize the clipboard hash with current content to avoid false initial events
            if let Ok(initial_content) = clipboard.get_text() {
                let initial_hash = Self::calculate_hash(&initial_content);
                *last_hash.lock().unwrap() = Some(initial_hash);
                debug!("Initialized clipboard monitoring with existing content hash");
            }

            while !stop_indicator.load(Ordering::SeqCst) {
                if let Ok(content) = clipboard.get_text() {
                    let hash = Self::calculate_hash(&content);
                    let mut last_hash_guard = last_hash.lock().unwrap();

                    if last_hash_guard.as_ref() != Some(&hash) {
                        *last_hash_guard = Some(hash);
                        drop(last_hash_guard);

                        let (truncated_content, truncated) =
                            if content.len() > config.max_clipboard_content_length {
                                (
                                    content[..config.max_clipboard_content_length].to_string(),
                                    true,
                                )
                            } else {
                                (content.clone(), false)
                            };

                        // Capture UI element if enabled
                        let ui_element = if capture_ui_elements {
                            automation
                                .as_ref()
                                .and_then(Self::get_focused_ui_element_with_timeout)
                        } else {
                            None
                        };

                        let clipboard_event = ClipboardEvent {
                            action: ClipboardAction::Copy, // Assume copy for content changes
                            content: Some(truncated_content),
                            content_size: Some(content.len()),
                            format: Some("text".to_string()),
                            truncated,
                            metadata: EventMetadata {
                                ui_element,
                                timestamp: Some(Self::capture_timestamp()),
                            },
                        };

                        let _ = event_tx.send(WorkflowEvent::Clipboard(clipboard_event));
                    }
                }

                thread::sleep(Duration::from_millis(200)); // PERFORMANCE: Check clipboard every 200ms (was 100ms)
            }
        });

        Ok(())
    }

    /// Calculate hash for content comparison
    fn calculate_hash(content: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    /// Get focused UI element with timeout protection to prevent hanging
    fn get_focused_ui_element_with_timeout(automation: &UIAutomation) -> Option<UIElement> {
        // Use panic catching to handle any COM/threading issues gracefully
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            automation.get_focused_element()
        })) {
            Ok(Ok(element)) => {
                // Successfully got element, now safely convert it
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    convert_uiautomation_element_to_terminator(element)
                })) {
                    Ok(ui_element) => Some(ui_element),
                    Err(_) => {
                        debug!("Failed to convert UI element safely");
                        None
                    }
                }
            }
            Ok(Err(e)) => {
                debug!("UI Automation call failed: {}", e);
                None
            }
            Err(_) => {
                debug!("UI Automation call panicked, handled gracefully");
                None
            }
        }
    }

    /// Set up UI Automation event handlers
    fn setup_ui_automation_events(
        &self,
        current_application: Arc<Mutex<Option<ApplicationState>>>,
        browser_tab_tracker: Arc<Mutex<BrowserTabTracker>>,
        handle: tokio::runtime::Handle,
    ) -> Result<()> {
        let event_tx = self.event_tx.clone();
        let stop_indicator = Arc::clone(&self.stop_indicator);
        let ui_automation_thread_id = Arc::clone(&self.ui_automation_thread_id);

        // Clone filtering configuration
        let ignore_focus_patterns = self.config.ignore_focus_patterns.clone();
        let ignore_property_patterns = self.config.ignore_property_patterns.clone();
        let ignore_window_titles = self.config.ignore_window_titles.clone();
        let ignore_applications = self.config.ignore_applications.clone();
        let config_clone = self.config.clone();
        let property_config = self.config.clone();

        // Capture typing session for autofill detection in property handler
        let current_typing_session = Arc::clone(&self.current_typing_session);

        thread::spawn(move || {
            info!("Starting UI Automation event monitoring thread");

            // Initialize COM apartment for UI Automation events
            // Threading model controlled by configuration
            let threading_model = if config_clone.enable_multithreading {
                COINIT_MULTITHREADED
            } else {
                COINIT_APARTMENTTHREADED
            };

            let com_initialized = unsafe {
                let hr = CoInitializeEx(None, threading_model);
                if hr.is_ok() {
                    let threading_name = if config_clone.enable_multithreading {
                        "multithreaded (MTA)"
                    } else {
                        "apartment threaded (STA)"
                    };
                    info!(
                        "✅ Successfully initialized COM apartment as {} for UI Automation events",
                        threading_name
                    );
                    true
                } else if hr == windows::Win32::Foundation::RPC_E_CHANGED_MODE {
                    warn!("⚠️  COM apartment already initialized with different threading model");
                    // This is expected if the main process already initialized COM differently
                    false
                } else {
                    error!(
                        "❌ Failed to initialize COM apartment for UI Automation: {:?}",
                        hr
                    );
                    return;
                }
            };

            // Store the thread ID for cleanup
            let thread_id = unsafe { GetCurrentThreadId() };
            *ui_automation_thread_id.lock().unwrap() = Some(thread_id);

            info!(
                "UI Automation event thread started (Thread ID: {})",
                thread_id
            );

            // Use new_direct() to avoid COM initialization conflicts
            // The uiautomation library's new() method tries to initialize COM as MTA which conflicts with our STA setup
            let automation = match uiautomation::UIAutomation::new_direct() {
                Ok(auto) => {
                    info!("✅ Successfully created UIAutomation instance using new_direct()");
                    auto
                }
                Err(e) => {
                    error!("❌ Failed to create UIAutomation instance: {}", e);
                    warn!(
                        "UI Automation events will be disabled, but other recording will continue"
                    );

                    // Still run message pump for potential future use
                    Self::run_message_pump(&stop_indicator);

                    // Clean up COM if we initialized it
                    if com_initialized {
                        unsafe {
                            CoUninitialize();
                        }
                    }
                    return;
                }
            };

            info!("UI Automation instance created successfully, setting up event handlers");

            // Set up focus change event handler if enabled
            info!("Setting up focus change event handler");
            let focus_event_tx = event_tx.clone();

            // Create a channel for thread-safe communication
            let (focus_tx, focus_rx) = std::sync::mpsc::channel::<Option<UIElement>>();

            // Create a focus changed event handler struct
            struct FocusHandler {
                sender: std::sync::mpsc::Sender<Option<UIElement>>,
            }

            impl uiautomation::events::CustomFocusChangedEventHandler for FocusHandler {
                fn handle(&self, sender: &uiautomation::UIElement) -> uiautomation::Result<()> {
                    // Perform the absolute minimum work on this thread.
                    // Convert to our thread-safe UIElement wrapper and send it to the worker thread.
                    let ui_element =
                        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            convert_uiautomation_element_to_terminator(sender.clone())
                        })) {
                            Ok(element) => Some(element),
                            Err(_) => {
                                debug!(
                                "Failed to convert focused UI element safely during event handling"
                            );
                                None
                            }
                        };

                    // Send the wrapped element to the processing thread.
                    if self.sender.send(ui_element).is_err() {
                        debug!(
                            "Failed to send focus change data to worker thread; it may have shut down."
                        );
                    }

                    Ok(())
                }
            }

            let focus_handler = FocusHandler { sender: focus_tx };

            let focus_event_handler =
                uiautomation::events::UIFocusChangedEventHandler::from(focus_handler);

            // Register the focus change event handler
            match automation.add_focus_changed_event_handler(None, &focus_event_handler) {
                Ok(_) => info!("✅ Focus change event handler registered successfully"),
                Err(e) => error!("❌ Failed to register focus change event handler: {}", e),
            }

            // Spawn a thread to process the focus change data safely
            let focus_event_tx_clone = focus_event_tx.clone();
            let focus_current_app = Arc::clone(&current_application);
            let focus_browser_tracker = Arc::clone(&browser_tab_tracker);

            // Clone the necessary variables for the focus-processing thread to take ownership of.
            let focus_processing_config = config_clone.clone();
            let focus_processing_ignore_patterns = ignore_focus_patterns.clone();
            let focus_processing_ignore_window_titles = ignore_window_titles.clone();
            let focus_processing_ignore_applications = ignore_applications.clone();
            let processing_handle = handle;

            std::thread::spawn(move || {
                while let Ok(ui_element) = focus_rx.recv() {
                    if let Some(element) = ui_element {
                        let element_name = element.name_or_empty();
                        let element_role = element.role().to_lowercase();
                        debug!(
                            "Focus event received for element: '{}', role: '{}'",
                            element_name, element_role
                        );

                        // User Request: If a button gets focus, send a ButtonClickEvent
                        if element_role.contains("button")
                            || element_role.contains("menuitem")
                            || element_role.contains("listitem")
                        {
                            let element_desc = element.attributes().description.unwrap_or_default();

                            let interaction_type = Self::determine_button_interaction_type(
                                &element_name,
                                &element_desc,
                                &element_role,
                            );
                            let is_enabled = element.is_enabled().unwrap_or(true);
                            let bounds = element.bounds().unwrap_or_default();

                            let button_event = ButtonClickEvent {
                                button_text: element_name.clone(),
                                interaction_type,
                                button_role: element_role.clone(),
                                was_enabled: is_enabled,
                                click_position: Some(Position {
                                    x: bounds.0 as i32,
                                    y: bounds.1 as i32,
                                }),
                                button_description: if element_desc.is_empty() {
                                    None
                                } else {
                                    Some(element_desc.clone())
                                },
                                metadata: EventMetadata::with_ui_element_and_timestamp(Some(
                                    element.clone(),
                                )),
                            };
                            let result =
                                focus_event_tx_clone.send(WorkflowEvent::ButtonClick(button_event));

                            if result.is_ok() {
                                debug!("Successfully sent ButtonClickEvent for '{}'", element_name);
                            } else {
                                warn!(
                                    "Failed to send ButtonClickEvent for '{}': {:?}",
                                    element_name,
                                    result.err()
                                );
                            }
                        }

                        // Offload slow checks to a separate async task to avoid blocking the queue
                        let task_current_app = Arc::clone(&focus_current_app);
                        let task_browser_tracker = Arc::clone(&focus_browser_tracker);
                        let task_event_tx_clone = focus_event_tx_clone.clone();
                        let task_element_name = element_name.clone();
                        let task_ui_element = Some(element);
                        let task_config_clone = focus_processing_config.clone();
                        let task_ignore_focus_patterns = focus_processing_ignore_patterns.clone();
                        let task_ignore_window_titles =
                            focus_processing_ignore_window_titles.clone();
                        let task_ignore_applications = focus_processing_ignore_applications.clone();

                        processing_handle.spawn(async move {
                            // Re-enable the previously disabled checks inside this non-blocking task
                            if WindowsRecorder::should_ignore_focus_event(
                                &task_element_name,
                                &task_ui_element,
                                &task_ignore_focus_patterns,
                                &task_ignore_window_titles,
                                &task_ignore_applications,
                            ) {
                                debug!("Ignoring focus change event for: {}", task_element_name);
                                return;
                            }

                            // Check for application switch (focus changes often indicate app switches)
                            Self::check_and_emit_application_switch(
                                &task_current_app,
                                &task_event_tx_clone,
                                &task_ui_element,
                                ApplicationSwitchMethod::WindowClick, // Focus change usually means window click
                                &task_config_clone,
                            );

                            // Check for browser tab navigation
                            // Extract URL from focus text if available
                            let focus_url = if let Some(ref element) = task_ui_element {
                                let element_name = element.name_or_empty();
                                if element_name.starts_with("http") && element_name.len() > 10 {
                                    Some(element_name.clone())
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                            Self::check_and_emit_browser_navigation_with_url(
                                &task_browser_tracker,
                                &task_event_tx_clone,
                                &task_ui_element,
                                TabNavigationMethod::TabClick, // Focus change in browser could be tab click
                                &task_config_clone,
                                focus_url,
                            );
                        });
                    }
                }
            });

            // Set up property change event handler if enabled
            info!("Setting up property change event handler");
            let property_event_tx = event_tx.clone();
            let property_ignore_patterns = ignore_property_patterns.clone();
            let property_ignore_window_titles = ignore_window_titles.clone();
            let property_ignore_applications = ignore_applications.clone();

            // Create a channel for thread-safe communication
            let (property_tx, property_rx) =
                std::sync::mpsc::channel::<(String, String, Option<UIElement>)>();

            // Create a property changed event handler using the proper closure type
            let property_handler: Box<uiautomation::events::CustomPropertyChangedEventHandlerFn> =
                Box::new(move |sender, property, value| {
                    let property_name = format!("{:?}", property);

                    // Only proceed if we can extract a meaningful value
                    if let Some(value_string) = Self::format_property_value(&value) {
                        // SAFELY extract UI element information while we're on the correct COM thread
                        let ui_element =
                            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                convert_uiautomation_element_to_terminator(sender.clone())
                            })) {
                                Ok(element) => Some(element),
                                Err(_) => {
                                    debug!(
                                    "Failed to convert UI element safely during property change"
                                );
                                    None
                                }
                            };

                        // Send the extracted data through the channel
                        if property_tx
                            .send((property_name, value_string, ui_element))
                            .is_err()
                        {
                            debug!(
                                "Failed to send property change data to worker thread; it may have shut down."
                            );
                        }
                    }

                    Ok(())
                });

            let property_event_handler =
                uiautomation::events::UIPropertyChangedEventHandler::from(property_handler);

            // Register property change event handler for common properties on the root element
            match automation.get_root_element() {
                Ok(root) => {
                    // PERFORMANCE: Only monitor ValueValue for maximum performance
                    // Name and HasKeyboardFocus create too much noise in most applications
                    let properties = vec![uiautomation::types::UIProperty::ValueValue];

                    match automation.add_property_changed_event_handler(
                            &root,
                            uiautomation::types::TreeScope::Subtree,
                            None,
                            &property_event_handler,
                            &properties
                        ) {
                            Ok(_) => info!("✅ Property change event handler registered for ValueValue only (optimized)"),
                            Err(e) => error!("❌ Failed to register property change event handler: {}", e),
                        }
                }
                Err(e) => error!(
                    "❌ Failed to get root element for property change events: {}",
                    e
                ),
            }

            // Spawn a thread to process the property change data safely
            let property_event_tx_clone = property_event_tx.clone();
            let property_browser_tracker = Arc::clone(&browser_tab_tracker);
            let property_typing_session = Arc::clone(&current_typing_session);
            std::thread::spawn(move || {
                while let Ok((property_name, value_string, ui_element)) = property_rx.recv() {
                    // Extract element name on the worker thread
                    let element_name = ui_element
                        .as_ref()
                        .map(|el| el.name_or_empty())
                        .unwrap_or_default();

                    // Apply filtering
                    if WindowsRecorder::should_ignore_property_event(
                        &element_name,
                        &property_name,
                        &ui_element,
                        &property_ignore_patterns,
                        &property_ignore_window_titles,
                        &property_ignore_applications,
                    ) {
                        debug!(
                            "Ignoring property change event for: {} ({})",
                            element_name, property_name
                        );
                        continue;
                    }

                    // Check for browser tab navigation (property changes often indicate URL/title changes)
                    // We look for URL-like strings in ValueValue property changes
                    if property_name == "ValueValue"
                        && (value_string.starts_with("http")
                            || value_string.contains(".com")
                            || value_string.contains(".org")
                            || value_string.contains(".net"))
                    {
                        // Add http:// prefix if missing for proper URL format
                        let full_url = if value_string.starts_with("http") {
                            value_string.clone()
                        } else {
                            format!("https://{}", value_string)
                        };

                        Self::check_and_emit_browser_navigation_with_url(
                            &property_browser_tracker,
                            &property_event_tx_clone,
                            &ui_element,
                            TabNavigationMethod::AddressBar, // Property change likely means address bar update
                            &property_config,
                            Some(full_url), // Pass the detected URL!
                        );
                    }

                    // === Autofill detection & TextInputCompletedEvent ===
                    if config_clone.record_text_input_completion
                        && property_name == "ValueValue"
                        && !value_string.trim().is_empty()
                    {
                        if let Some(ref element) = ui_element {
                            let role_lower = element.role().to_lowercase();
                            // Basic check: is this an editable field?
                            if role_lower.contains("edit") || role_lower.contains("textbox") || role_lower.contains("input") {
                                // Ignore if we were already in a typing session (keystrokes seen)
                                let keystrokes = property_typing_session.keystroke_count.load(Ordering::Relaxed);
                                let typing_active = property_typing_session.is_active.load(Ordering::Relaxed);

                                if keystrokes == 0 && !typing_active {
                                    // Treat this as autofill
                                    let field_name = element.name_or_empty();
                                    let text_input_event = TextInputCompletedEvent {
                                        text_value: value_string.clone(),
                                        field_name: if field_name.is_empty() { None } else { Some(field_name) },
                                        field_type: element.role(),
                                        input_method: TextInputMethod::AutoFilled,
                                        typing_duration_ms: 0,
                                        keystroke_count: 0,
                                        metadata: EventMetadata::with_ui_element_and_timestamp(ui_element.clone()),
                                    };

                                    // Ensure typing session is marked completed/cleared
                                    let _ = property_typing_session.complete_session(true, 0);

                                    let _ = property_event_tx_clone
                                        .send(WorkflowEvent::TextInputCompleted(text_input_event));
                                }
                            }
                        }
                    }
                }
            });

            info!("✅ UI Automation event handlers setup complete, starting message pump");

            // CRITICAL: Start Windows message pump for COM/UI Automation events
            Self::run_message_pump(&stop_indicator);

            info!("UI Automation event monitoring stopped");

            // Clean up COM if we initialized it
            if com_initialized {
                unsafe {
                    CoUninitialize();
                }
                debug!("COM uninitialized");
            }
        });

        Ok(())
    }

    /// Run the Windows message pump for UI Automation events
    fn run_message_pump(stop_indicator: &Arc<AtomicBool>) {
        info!("Starting Windows message pump for UI Automation events");
        unsafe {
            let mut msg = MSG::default();
            while !stop_indicator.load(Ordering::SeqCst) {
                let result = GetMessageW(&mut msg, None, 0, 0);

                match result.0 {
                    -1 => {
                        // Error occurred
                        error!("Error in message pump: GetMessage failed");
                        break;
                    }
                    0 => {
                        // WM_QUIT received
                        debug!("WM_QUIT received in UI Automation message pump");
                        break;
                    }
                    _ => {
                        // Normal message - process it
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);

                        // Check for quit message
                        if msg.message == WM_QUIT {
                            debug!("WM_QUIT message processed");
                            break;
                        }
                    }
                }

                // Brief yield to check stop condition more frequently
                if msg.message == 0 {
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        }
        info!("Windows message pump stopped");
    }

    /// Check if a focus change event should be ignored based on filtering patterns
    fn should_ignore_focus_event(
        element_name: &str,
        ui_element: &Option<UIElement>,
        ignore_patterns: &std::collections::HashSet<String>,
        ignore_window_titles: &std::collections::HashSet<String>,
        ignore_applications: &std::collections::HashSet<String>,
    ) -> bool {
        let element_name_lower = element_name.to_lowercase();

        // Check against focus-specific ignore patterns
        if ignore_patterns
            .iter()
            .any(|pattern| element_name_lower.contains(pattern))
        {
            return true;
        }

        // Check against window title patterns
        if ignore_window_titles
            .iter()
            .any(|title| element_name_lower.contains(title))
        {
            return true;
        }

        // Check against application patterns
        if let Some(ui_elem) = ui_element {
            let app_name = ui_elem.application_name();
            if !app_name.is_empty() {
                let app_name_lower = app_name.to_lowercase();
                if ignore_applications
                    .iter()
                    .any(|app| app_name_lower.contains(app))
                {
                    return true;
                }
            }
        }

        false
    }

    /// Check if a property change event should be ignored based on filtering patterns
    fn should_ignore_property_event(
        element_name: &str,
        property_name: &str,
        ui_element: &Option<UIElement>,
        ignore_patterns: &std::collections::HashSet<String>,
        ignore_window_titles: &std::collections::HashSet<String>,
        ignore_applications: &std::collections::HashSet<String>,
    ) -> bool {
        let element_name_lower = element_name.to_lowercase();
        let property_name_lower = property_name.to_lowercase();

        // Check against property-specific ignore patterns
        if ignore_patterns.iter().any(|pattern| {
            element_name_lower.contains(pattern) || property_name_lower.contains(pattern)
        }) {
            return true;
        }

        // Check against window title patterns
        if ignore_window_titles
            .iter()
            .any(|title| element_name_lower.contains(title))
        {
            return true;
        }

        // Check against application patterns
        if let Some(ui_elem) = ui_element {
            let app_name = ui_elem.application_name();
            if !app_name.is_empty() {
                let app_name_lower = app_name.to_lowercase();
                if ignore_applications
                    .iter()
                    .any(|app| app_name_lower.contains(app))
                {
                    return true;
                }
            }
        }

        // Ignore frequent time-based property changes that are just noise
        if property_name_lower == "name"
            && (element_name_lower.contains("clock") ||
            element_name_lower.contains("time") ||
            element_name_lower.contains("pm") ||
            element_name_lower.contains("am") ||
            // Check for date patterns like "5/28/2025"
            element_name.matches('/').count() >= 2)
        {
            return true;
        }

        false
    }

    /// Stop recording
    pub fn stop(&self) -> Result<()> {
        debug!("Stopping comprehensive Windows recorder...");
        self.stop_indicator.store(true, Ordering::SeqCst);

        // Signal the UI Automation thread to stop by posting a quit message
        if let Some(thread_id) = *self.ui_automation_thread_id.lock().unwrap() {
            unsafe {
                let result = PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
                if result.is_ok() {
                    debug!(
                        "Posted WM_QUIT message to UI Automation thread {}",
                        thread_id
                    );
                } else {
                    warn!(
                        "Failed to post WM_QUIT message to UI Automation thread {}",
                        thread_id
                    );
                }
            }
        }

        info!("Windows recorder stop signal sent");
        Ok(())
    }

    /// Start typing completion timer to handle session timeouts
    fn start_typing_completion_timer(&self) {
        let current_session = Arc::clone(&self.current_typing_session);
        let event_tx = self.event_tx.clone();
        let stop_indicator = Arc::clone(&self.stop_indicator);
        let timeout_ms = self.config.text_input_completion_timeout_ms;
        let config = self.config.clone();

        thread::spawn(move || {
            // Create a UIAutomation instance once for this thread to reuse, respecting the configured threading model
            let automation = match Self::create_configured_automation_instance(&config) {
                Ok(auto) => Some(auto),
                Err(e) => {
                    warn!(
                        "Failed to create UIAutomation for typing completion timer: {}. Text completion events might be missed.",
                        e
                    );
                    None
                }
            };

            if let Some(auto) = automation {
                while !stop_indicator.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(timeout_ms / 2)); // Check twice per timeout period

                    Self::check_and_complete_typing_session(
                        &auto,
                        &current_session,
                        &event_tx,
                        false, // don't force, let timeout logic decide
                        timeout_ms,
                    );
                }
            }
        });
    }

    /// Determine the type of button interaction based on element characteristics
    fn determine_button_interaction_type(
        name: &str,
        description: &str,
        role: &str,
    ) -> ButtonInteractionType {
        let name_lower = name.to_lowercase();
        let desc_lower = description.to_lowercase();
        let role_lower = role.to_lowercase();

        // Check for dropdown indicators
        if name_lower.contains("dropdown")
            || name_lower.contains("▼")
            || name_lower.contains("⏷")
            || desc_lower.contains("dropdown")
            || desc_lower.contains("expand")
            || desc_lower.contains("collapse")
        {
            return ButtonInteractionType::DropdownToggle;
        }

        // Check for submit buttons
        if name_lower.contains("submit")
            || name_lower.contains("save")
            || name_lower.contains("ok")
            || name_lower.contains("apply")
            || name_lower.contains("confirm")
        {
            return ButtonInteractionType::Submit;
        }

        // Check for cancel buttons
        if name_lower.contains("cancel")
            || name_lower.contains("close")
            || name_lower.contains("×")
            || name_lower.contains("dismiss")
        {
            return ButtonInteractionType::Cancel;
        }

        // Check for toggle buttons
        if role_lower.contains("toggle")
            || name_lower.contains("toggle")
            || desc_lower.contains("toggle")
        {
            return ButtonInteractionType::Toggle;
        }

        // Default to simple click
        ButtonInteractionType::Click
    }

    /// Improved browser navigation detection with better filtering
    fn should_emit_browser_navigation(
        ui_element: &Option<UIElement>,
        detected_url: &Option<String>,
        detected_title: &Option<String>,
    ) -> bool {
        // Only emit if we have meaningful navigation data
        if detected_url.is_none() && detected_title.is_none() {
            return false;
        }

        // Check if this is actually a page navigation vs UI interaction
        if let Some(ref element) = ui_element {
            let element_role = element.role().to_lowercase();
            let element_name = element.name_or_empty().to_lowercase();

            // Don't treat dropdown/list interactions as navigation (main issue to fix)
            if element_role.contains("listitem")
                && (element_name.contains("break")
                    || element_name.contains("ready")
                    || element_name.contains("lunch"))
            {
                debug!(
                    "Ignoring browser navigation for dropdown option: {} (role: {})",
                    element_name, element_role
                );
                return false;
            }

            // Don't treat form controls as navigation
            if element_role.contains("textbox") || element_role.contains("edit") {
                debug!(
                    "Ignoring browser navigation for form control: {}",
                    element_name
                );
                return false;
            }

            // Remove the overly restrictive filtering - let most browser events through
            // Only block the specific dropdown items that were causing issues
        }

        // Only emit for meaningful URL changes
        if let Some(ref url) = detected_url {
            // Filter out non-URLs that might have been falsely detected
            if !url.starts_with("http") || url.len() < 10 {
                return false;
            }
        }

        true
    }
}

/// Convert a Key to a u32
fn key_to_u32(key: &Key) -> u32 {
    match key {
        Key::KeyA => 0x41,
        Key::KeyB => 0x42,
        Key::KeyC => 0x43,
        Key::KeyD => 0x44,
        Key::KeyE => 0x45,
        Key::KeyF => 0x46,
        Key::KeyG => 0x47,
        Key::KeyH => 0x48,
        Key::KeyI => 0x49,
        Key::KeyJ => 0x4A,
        Key::KeyK => 0x4B,
        Key::KeyL => 0x4C,
        Key::KeyM => 0x4D,
        Key::KeyN => 0x4E,
        Key::KeyO => 0x4F,
        Key::KeyP => 0x50,
        Key::KeyQ => 0x51,
        Key::KeyR => 0x52,
        Key::KeyS => 0x53,
        Key::KeyT => 0x54,
        Key::KeyU => 0x55,
        Key::KeyV => 0x56,
        Key::KeyW => 0x57,
        Key::KeyX => 0x58,
        Key::KeyY => 0x59,
        Key::KeyZ => 0x5A,
        Key::Num0 => 0x30,
        Key::Num1 => 0x31,
        Key::Num2 => 0x32,
        Key::Num3 => 0x33,
        Key::Num4 => 0x34,
        Key::Num5 => 0x35,
        Key::Num6 => 0x36,
        Key::Num7 => 0x37,
        Key::Num8 => 0x38,
        Key::Num9 => 0x39,
        Key::Escape => 0x1B,
        Key::Backspace => 0x08,
        Key::Tab => 0x09,
        Key::Return => 0x0D,
        Key::Space => 0x20,
        Key::LeftArrow => 0x25,
        Key::UpArrow => 0x26,
        Key::RightArrow => 0x27,
        Key::DownArrow => 0x28,
        Key::Delete => 0x2E,
        Key::Home => 0x24,
        Key::End => 0x23,
        Key::PageUp => 0x21,
        Key::PageDown => 0x22,
        Key::F1 => 0x70,
        Key::F2 => 0x71,
        Key::F3 => 0x72,
        Key::F4 => 0x73,
        Key::F5 => 0x74,
        Key::F6 => 0x75,
        Key::F7 => 0x76,
        Key::F8 => 0x77,
        Key::F9 => 0x78,
        Key::F10 => 0x79,
        Key::F11 => 0x7A,
        Key::F12 => 0x7B,
        Key::ShiftLeft => 0xA0,
        Key::ShiftRight => 0xA1,
        Key::ControlLeft => 0xA2,
        Key::ControlRight => 0xA3,
        Key::Alt => 0xA4,
        Key::AltGr => 0xA5,
        Key::MetaLeft => 0x5B,
        Key::MetaRight => 0x5C,
        _ => 0,
    }
}
