use crate::{
    ClipboardAction, ClipboardEvent, EventMetadata, HotkeyEvent, KeyboardEvent, MouseButton,
    MouseEvent, MouseEventType, Position, Result, UiFocusChangedEvent, UiPropertyChangedEvent,
    WorkflowEvent, WorkflowRecorderConfig,
};
use arboard::Clipboard;
use rdev::{Button, EventType, Key};
use regex::Regex;
use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use terminator::{convert_uiautomation_element_to_terminator, UIElement};
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};
use uiautomation::UIAutomation;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
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
    pattern: Regex,
    action: String,
    keys: Vec<u32>,
}

impl WindowsRecorder {
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
        };

        // Set up comprehensive event listeners
        recorder.setup_comprehensive_listeners().await?;

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

    /// Initialize common hotkey patterns
    fn initialize_hotkey_patterns() -> Vec<HotkeyPattern> {
        vec![
            HotkeyPattern {
                pattern: Regex::new(r"Ctrl\+C").unwrap(),
                action: "Copy".to_string(),
                keys: vec![162, 67], // Ctrl + C
            },
            HotkeyPattern {
                pattern: Regex::new(r"Ctrl\+V").unwrap(),
                action: "Paste".to_string(),
                keys: vec![162, 86], // Ctrl + V
            },
            HotkeyPattern {
                pattern: Regex::new(r"Ctrl\+X").unwrap(),
                action: "Cut".to_string(),
                keys: vec![162, 88], // Ctrl + X
            },
            HotkeyPattern {
                pattern: Regex::new(r"Ctrl\+Z").unwrap(),
                action: "Undo".to_string(),
                keys: vec![162, 90], // Ctrl + Z
            },
            HotkeyPattern {
                pattern: Regex::new(r"Ctrl\+Y").unwrap(),
                action: "Redo".to_string(),
                keys: vec![162, 89], // Ctrl + Y
            },
            HotkeyPattern {
                pattern: Regex::new(r"Ctrl\+S").unwrap(),
                action: "Save".to_string(),
                keys: vec![162, 83], // Ctrl + S
            },
            HotkeyPattern {
                pattern: Regex::new(r"Alt\+Tab").unwrap(),
                action: "Switch Window".to_string(),
                keys: vec![164, 9], // Alt + Tab
            },
            HotkeyPattern {
                pattern: Regex::new(r"Win\+D").unwrap(),
                action: "Show Desktop".to_string(),
                keys: vec![91, 68], // Win + D
            },
            HotkeyPattern {
                pattern: Regex::new(r"Ctrl\+Shift\+Esc").unwrap(),
                action: "Task Manager".to_string(),
                keys: vec![162, 160, 27], // Ctrl + Shift + Esc
            },
        ]
    }

    /// Set up comprehensive event listeners
    async fn setup_comprehensive_listeners(&mut self) -> Result<()> {
        // Main input event listener (enhanced from original)
        self.setup_enhanced_input_listener().await?;

        // Clipboard monitoring
        if self.config.record_clipboard {
            self.setup_clipboard_monitor()?;
        }

        // UI Automation event monitoring
        self.setup_ui_automation_events()?;

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
        let mouse_move_throttle = self.config.mouse_move_throttle_ms;
        let track_modifiers = self.config.track_modifier_states;
        let record_hotkeys = self.config.record_hotkeys;

        thread::spawn(move || {
            // PERFORMANCE: Create UIAutomation instance once outside the event loop
            let automation = if capture_ui_elements {
                match UIAutomation::new() {
                    Ok(auto) => {
                        info!("✅ UIAutomation instance created for input events");
                        Some(auto)
                    }
                    Err(e) => {
                        warn!("⚠️  Failed to create UIAutomation for input events: {}", e);
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

                        let character = if key_code >= 32 && key_code <= 126 {
                            Some(key_code as u8 as char)
                        } else {
                            None
                        };

                        // Capture UI element for keyboard events if enabled
                        let mut ui_element = None;
                        if capture_ui_elements {
                            // Use a synchronous approach instead of async to avoid runtime issues
                            ui_element = Self::get_focused_ui_element(automation.as_ref().unwrap());
                            // ui_element = None
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
                                ui_element
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

                        // Capture UI element for keyboard events if enabled
                        let mut ui_element = None;
                        if capture_ui_elements {
                            // ui_element = Self::get_focused_ui_element(automation.as_ref().unwrap());
                            ui_element = None
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
                                ui_element
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

                            let mut ui_element = None;
                            if capture_ui_elements {
                                ui_element =
                                    Self::get_focused_ui_element(automation.as_ref().unwrap());
                            }

                            let mouse_event = MouseEvent {
                                event_type: MouseEventType::Down,
                                button: mouse_button,
                                position: Position { x, y },
                                scroll_delta: None,
                                drag_start: None,
                                metadata: EventMetadata {
                                    ui_element
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
                                ui_element =
                                    Self::get_focused_ui_element(automation.as_ref().unwrap());
                            }

                            let mouse_event = MouseEvent {
                                event_type: MouseEventType::Up,
                                button: mouse_button,
                                position: Position { x, y },
                                scroll_delta: None,
                                drag_start: None,
                                metadata: EventMetadata {
                                    ui_element
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
                            // ui_element = Self::get_focused_ui_element(automation.as_ref().unwrap());
                            ui_element = None
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
                                    ui_element
                                },
                            };
                            let _ = event_tx.send(WorkflowEvent::Mouse(mouse_event));
                        }
                    }
                    EventType::Wheel { delta_x, delta_y } => {
                        if let Some((x, y)) = *last_mouse_pos.lock().unwrap() {
                            let mut ui_element = None;
                            if capture_ui_elements {
                                ui_element = None
                                    // Self::get_focused_ui_element(automation.as_ref().unwrap());
                            }

                            let mouse_event = MouseEvent {
                                event_type: MouseEventType::Wheel,
                                button: MouseButton::Middle,
                                position: Position { x, y },
                                scroll_delta: Some((delta_x as i32, delta_y as i32)),
                                drag_start: None,
                                metadata: EventMetadata {
                                    ui_element
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
                        ui_element: None
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
        let max_content_length = self.config.max_clipboard_content_length;
        let capture_ui_elements = self.config.capture_ui_elements;

        thread::spawn(move || {
            let mut clipboard = match Clipboard::new() {
                Ok(cb) => cb,
                Err(e) => {
                    error!("Failed to initialize clipboard: {}", e);
                    return;
                }
            };
            let automation = UIAutomation::new().unwrap();

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

                        let (truncated_content, truncated) = if content.len() > max_content_length {
                            (content[..max_content_length].to_string(), true)
                        } else {
                            (content.clone(), false)
                        };

                        // Capture UI element if enabled
                        let ui_element = if capture_ui_elements {
                            Self::get_focused_ui_element(&automation)
                            // None
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
                                ui_element
                            },
                        };

                        let _ = event_tx.send(WorkflowEvent::Clipboard(clipboard_event));
                    }
                }

                thread::sleep(Duration::from_millis(100)); // Check clipboard every 100ms
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

    /// Get the currently focused UI element
    fn get_focused_ui_element(automation: &UIAutomation) -> Option<UIElement> {
        debug!("Getting focused UI element");
        match automation.get_focused_element() {
            Ok(element) => Some(convert_uiautomation_element_to_terminator(element)),
            Err(e) => {
                debug!("Failed to get focused element: {}", e);
                None
            }
        }
    }

    /// Set up UI Automation event handlers
    fn setup_ui_automation_events(&self) -> Result<()> {
        let event_tx = self.event_tx.clone();
        let stop_indicator = Arc::clone(&self.stop_indicator);
        let ui_automation_thread_id = Arc::clone(&self.ui_automation_thread_id);
        let record_structure_changes = self.config.record_ui_structure_changes;
        let record_property_changes = self.config.record_ui_property_changes;
        let record_focus_changes = self.config.record_ui_focus_changes;

        // Clone filtering configuration
        let ignore_focus_patterns = self.config.ignore_focus_patterns.clone();
        let ignore_property_patterns = self.config.ignore_property_patterns.clone();
        let ignore_window_titles = self.config.ignore_window_titles.clone();
        let ignore_applications = self.config.ignore_applications.clone();

        if !record_structure_changes && !record_property_changes && !record_focus_changes {
            debug!("No UI Automation events enabled, skipping setup");
            return Ok(());
        }

        thread::spawn(move || {
            info!("Starting UI Automation event monitoring thread");

            // CRITICAL: Initialize COM apartment as STA for UI Automation events
            // This is required because UI Automation events need STA threading
            let com_initialized = unsafe {
                let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                if hr.is_ok() {
                    info!(
                        "✅ Successfully initialized COM apartment as STA for UI Automation events"
                    );
                    true
                } else if hr == windows::Win32::Foundation::RPC_E_CHANGED_MODE.into() {
                    warn!("⚠️  COM apartment already initialized with different threading model");
                    // This is expected if the main process already initialized COM as MTA
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
            if record_focus_changes {
                info!("Setting up focus change event handler");
                let focus_event_tx = event_tx.clone();
                let focus_ignore_patterns = ignore_focus_patterns.clone();
                let focus_ignore_window_titles = ignore_window_titles.clone();
                let focus_ignore_applications = ignore_applications.clone();

                // Create a channel for thread-safe communication
                let (focus_tx, focus_rx) =
                    std::sync::mpsc::channel::<(String, Option<UIElement>)>();

                // Create a focus changed event handler struct
                struct FocusHandler {
                    sender: std::sync::mpsc::Sender<(String, Option<UIElement>)>,
                }

                impl uiautomation::events::CustomFocusChangedEventHandler for FocusHandler {
                    fn handle(&self, sender: &uiautomation::UIElement) -> uiautomation::Result<()> {
                        // Ensure we're handling this on the correct thread
                        let current_thread = unsafe { GetCurrentThreadId() };
                        debug!("Focus change event received on thread {}", current_thread);

                        // Extract basic data that's safe to send across threads
                        let element_name =
                            sender.get_name().unwrap_or_else(|_| "Unknown".to_string());

                        // SAFELY extract UI element information while we're on the correct COM thread
                        let ui_element = match std::panic::catch_unwind(
                            std::panic::AssertUnwindSafe(|| {
                                convert_uiautomation_element_to_terminator(sender.clone())
                            }),
                        ) {
                            Ok(element) => {
                                // Additional safety: verify we can access basic properties
                                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    let name = element.name_or_empty();
                                    let role = element.role();
                                    (name, role)
                                })) {
                                    Ok((name, role)) => {
                                        debug!("Successfully converted focus UI element: name='{}', role='{}'", name, role);
                                        Some(element)
                                    }
                                    Err(e) => {
                                        debug!("UI element converted but properties inaccessible: {:?}", e);
                                        // Return the element anyway since basic conversion worked
                                        Some(element)
                                    }
                                }
                            }
                            Err(e) => {
                                debug!("Failed to convert UI element safely: {:?}", e);
                                None
                            }
                        };

                        // Send the extracted data through the channel
                        if let Err(e) = self.sender.send((element_name, ui_element)) {
                            debug!("Failed to send focus change data through channel: {}", e);
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
                std::thread::spawn(move || {
                    while let Ok((element_name, ui_element)) = focus_rx.recv() {
                        // Apply filtering
                        if WindowsRecorder::should_ignore_focus_event(
                            &element_name,
                            &ui_element,
                            &focus_ignore_patterns,
                            &focus_ignore_window_titles,
                            &focus_ignore_applications,
                        ) {
                            debug!("Ignoring focus change event for: {}", element_name);
                            continue;
                        }

                        // Create a minimal UI element representation
                        let focus_event = UiFocusChangedEvent {
                            previous_element: None,
                            metadata: EventMetadata {
                                ui_element
                            },
                        };

                        if let Err(e) =
                            focus_event_tx_clone.send(WorkflowEvent::UiFocusChanged(focus_event))
                        {
                            debug!("Failed to send focus change event: {}", e);
                            break;
                        }
                    }
                });
            }

            // Set up property change event handler if enabled
            if record_property_changes {
                info!("Setting up property change event handler");
                let property_event_tx = event_tx.clone();
                let property_ignore_patterns = ignore_property_patterns.clone();
                let property_ignore_window_titles = ignore_window_titles.clone();
                let property_ignore_applications = ignore_applications.clone();

                // Create a channel for thread-safe communication
                let (property_tx, property_rx) =
                    std::sync::mpsc::channel::<(String, String, String, Option<UIElement>)>();

                // Create a property changed event handler using the proper closure type
                let property_handler: Box<
                    uiautomation::events::CustomPropertyChangedEventHandlerFn,
                > = Box::new(move |sender, property, value| {
                    // Ensure we're handling this on the correct thread
                    let current_thread = unsafe { GetCurrentThreadId() };

                    // Only process certain properties to reduce noise
                    match property {
                        uiautomation::types::UIProperty::ValueValue
                        | uiautomation::types::UIProperty::Name
                        | uiautomation::types::UIProperty::HasKeyboardFocus => {
                            debug!(
                                "Property change event received for {:?} on thread {}",
                                property, current_thread
                            );
                        }
                        _ => {
                            // Skip other properties to reduce noise
                            return Ok(());
                        }
                    }

                    // Extract basic data that's safe to send across threads
                    let element_name = sender.get_name().unwrap_or_else(|_| "Unknown".to_string());
                    let property_name = format!("{:?}", property);

                    // Only proceed if we can extract a meaningful value
                    if let Some(value_string) = Self::format_property_value(&value) {
                        // SAFELY extract UI element information while we're on the correct COM thread
                        let ui_element = match std::panic::catch_unwind(
                            std::panic::AssertUnwindSafe(|| {
                                convert_uiautomation_element_to_terminator(sender.clone())
                            }),
                        ) {
                            Ok(element) => {
                                // Additional safety: verify we can access basic properties
                                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    let name = element.name_or_empty();
                                    let role = element.role();
                                    (name, role)
                                })) {
                                    Ok((name, role)) => {
                                        debug!("Successfully converted property UI element: name='{}', role='{}'", name, role);
                                        Some(element)
                                    }
                                    Err(e) => {
                                        debug!("UI element converted but properties inaccessible: {:?}", e);
                                        // Return the element anyway since basic conversion worked
                                        Some(element)
                                    }
                                }
                            }
                            Err(e) => {
                                debug!("Failed to convert UI element safely: {:?}", e);
                                None
                            }
                        };

                        // Send the extracted data through the channel
                        if let Err(e) = property_tx.send((
                            element_name,
                            property_name,
                            value_string,
                            ui_element,
                        )) {
                            debug!("Failed to send property change data through channel: {}", e);
                        }
                    } else {
                        debug!(
                            "Skipping property change with empty/null value: {} on {}",
                            property_name, element_name
                        );
                    }

                    Ok(())
                });

                let property_event_handler =
                    uiautomation::events::UIPropertyChangedEventHandler::from(property_handler);

                // Register property change event handler for common properties on the root element
                match automation.get_root_element() {
                    Ok(root) => {
                        let properties = vec![
                            uiautomation::types::UIProperty::ValueValue,
                            uiautomation::types::UIProperty::Name,
                            uiautomation::types::UIProperty::HasKeyboardFocus,
                        ];

                        match automation.add_property_changed_event_handler(
                            &root,
                            uiautomation::types::TreeScope::Subtree,
                            None,
                            &property_event_handler,
                            &properties
                        ) {
                            Ok(_) => info!("✅ Property change event handler registered for ValueValue, Name, and HasKeyboardFocus"),
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
                std::thread::spawn(move || {
                    while let Ok((element_name, property_name, value_string, ui_element)) =
                        property_rx.recv()
                    {
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

                        let property_event = UiPropertyChangedEvent {
                            property_name: property_name.clone(),
                            old_value: None,
                            new_value: Some(value_string),
                            metadata: EventMetadata {
                                ui_element
                            },
                        };

                        if let Err(e) = property_event_tx_clone
                            .send(WorkflowEvent::UiPropertyChanged(property_event))
                        {
                            debug!("Failed to send property change event: {}", e);
                            break;
                        }
                    }
                });
            }

            // Note: Structure change events are not yet implemented in the current version
            // of the uiautomation crate, so we'll keep the polling approach for now
            if record_structure_changes {
                warn!(
                    "Structure change events are not yet fully supported, using polling fallback"
                );
                // TODO: Implement when structure change events become available
            }

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
        ignore_patterns: &[String],
        ignore_window_titles: &[String],
        ignore_applications: &[String],
    ) -> bool {
        let element_name_lower = element_name.to_lowercase();

        // Check against focus-specific ignore patterns
        for pattern in ignore_patterns {
            if element_name_lower.contains(&pattern.to_lowercase()) {
                return true;
            }
        }

        // Check against window title patterns
        for title in ignore_window_titles {
            if element_name_lower.contains(&title.to_lowercase()) {
                return true;
            }
        }

        // Check against application patterns
        if let Some(ui_elem) = ui_element {
            let app_name = ui_elem.application_name();
            if !app_name.is_empty() {
                let app_name_lower = app_name.to_lowercase();
                for app in ignore_applications {
                    if app_name_lower.contains(&app.to_lowercase()) {
                        return true;
                    }
                }
            }
        }

        // Check for common system UI elements that are typically noise
        let common_ignore_patterns = [
            "clock",
            "notification",
            "taskbar",
            "start button",
            "system tray",
            "search",
            "cortana",
            "action center",
            "windows security",
            "microsoft edge webview2",
        ];

        for pattern in &common_ignore_patterns {
            if element_name_lower.contains(pattern) {
                return true;
            }
        }

        false
    }

    /// Check if a property change event should be ignored based on filtering patterns
    fn should_ignore_property_event(
        element_name: &str,
        property_name: &str,
        ui_element: &Option<UIElement>,
        ignore_patterns: &[String],
        ignore_window_titles: &[String],
        ignore_applications: &[String],
    ) -> bool {
        let element_name_lower = element_name.to_lowercase();
        let property_name_lower = property_name.to_lowercase();

        // Check against property-specific ignore patterns
        for pattern in ignore_patterns {
            if element_name_lower.contains(&pattern.to_lowercase())
                || property_name_lower.contains(&pattern.to_lowercase())
            {
                return true;
            }
        }

        // Check against window title patterns
        for title in ignore_window_titles {
            if element_name_lower.contains(&title.to_lowercase()) {
                return true;
            }
        }

        // Check against application patterns
        if let Some(ui_elem) = ui_element {
            let app_name = ui_elem.application_name();
            if !app_name.is_empty() {
                let app_name_lower = app_name.to_lowercase();
                for app in ignore_applications {
                    if app_name_lower.contains(&app.to_lowercase()) {
                        return true;
                    }
                }
            }
        }

        // Check for common system UI elements that are typically noise
        let common_ignore_patterns = [
            "clock",
            "notification",
            "taskbar",
            "start button",
            "system tray",
            "search",
            "cortana",
            "action center",
            "windows security",
            "microsoft edge webview2",
        ];

        for pattern in &common_ignore_patterns {
            if element_name_lower.contains(pattern) {
                return true;
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
