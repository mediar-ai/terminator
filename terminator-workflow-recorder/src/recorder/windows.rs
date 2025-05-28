use crate::{
    KeyboardEvent, MouseButton, MouseEvent, MouseEventType, Position, UiElement,
    WorkflowEvent, WorkflowRecorderConfig, ClipboardEvent, ClipboardAction,
    HotkeyEvent, Result, EventMetadata,
    UiPropertyChangedEvent, UiFocusChangedEvent
};
use std::{
    sync::{Arc, Mutex, mpsc},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
    collections::{HashMap},
    thread,
};
use tokio::sync::broadcast;
use tracing::{debug, info, error, warn};
use rdev::{EventType, Button, Key};
use uiautomation::{UIAutomation, UIElement as WinUIElement};
use uiautomation::types::{Point, UIProperty};
use windows::Win32::Foundation::POINT;
use dashmap::DashMap;
use arboard::Clipboard;
use regex::Regex;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::Foundation::{CloseHandle, HANDLE};

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

    /// Cache for process information (PID -> (Window Title, App Name))
    process_info_cache: Arc<DashMap<u32, (Option<String>, Option<String>)>>,
    
    /// Modifier key states
    modifier_states: Arc<Mutex<ModifierStates>>,
    
    /// Last clipboard content hash for change detection
    last_clipboard_hash: Arc<Mutex<Option<u64>>>,
    
    /// Last mouse move time for throttling
    last_mouse_move_time: Arc<Mutex<Instant>>,
    
    /// Known hotkey patterns
    hotkey_patterns: Arc<Vec<HotkeyPattern>>,
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
    pub fn new(
        config: WorkflowRecorderConfig,
        event_tx: broadcast::Sender<WorkflowEvent>,
    ) -> Result<Self> {
        info!("Initializing comprehensive Windows recorder");
        debug!("Recorder config: {:?}", config);
        
        let last_mouse_pos = Arc::new(Mutex::new(None));
        let stop_indicator = Arc::new(AtomicBool::new(false));
        let process_info_cache = Arc::new(DashMap::new());
        let modifier_states = Arc::new(Mutex::new(ModifierStates {
            ctrl: false, alt: false, shift: false, win: false,
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
            process_info_cache,
            modifier_states,
            last_clipboard_hash,
            last_mouse_move_time,
            hotkey_patterns,
        };
        
        // Set up comprehensive event listeners
        recorder.setup_comprehensive_listeners()?;
        
        Ok(recorder)
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
    fn setup_comprehensive_listeners(&mut self) -> Result<()> {
        // Main input event listener (enhanced from original)
        self.setup_enhanced_input_listener()?;
        
        // Clipboard monitoring
        if self.config.record_clipboard {
            self.setup_clipboard_monitor()?;
        }
        
        // UI Automation event monitoring
        self.setup_ui_automation_events()?;
        
        Ok(())
    }
    
    /// Set up enhanced input event listener
    fn setup_enhanced_input_listener(&mut self) -> Result<()> {
        let event_tx = self.event_tx.clone();
        let last_mouse_pos = Arc::clone(&self.last_mouse_pos);
        let capture_ui_elements = self.config.capture_ui_elements;
        let stop_indicator_clone = Arc::clone(&self.stop_indicator);
        let process_info_cache_clone = Arc::clone(&self.process_info_cache);
        let modifier_states = Arc::clone(&self.modifier_states);
        let last_mouse_move_time = Arc::clone(&self.last_mouse_move_time);
        let hotkey_patterns = Arc::clone(&self.hotkey_patterns);
        let mouse_move_throttle = self.config.mouse_move_throttle_ms;
        let track_modifiers = self.config.track_modifier_states;
        let record_hotkeys = self.config.record_hotkeys;

        
        thread::spawn(move || {
            let automation = match UIAutomation::new() {
                Ok(auto) => auto,
                Err(e) => {
                    error!("Failed to create UIAutomation instance: {}", e);
                    return;
                }
            };
            
            let mut active_keys: HashMap<u32, bool> = HashMap::new();
            
            if let Err(error) = rdev::listen(move |event| {
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
                            if let Some(hotkey) = Self::detect_hotkey(&hotkey_patterns, &active_keys) {
                                let _ = event_tx.send(WorkflowEvent::Hotkey(hotkey));
                            }
                        }
                        
                        let modifiers = if track_modifiers {
                            modifier_states.lock().unwrap().clone()
                        } else {
                            ModifierStates { ctrl: false, alt: false, shift: false, win: false }
                        };
                        
                        let character = if key_code >= 32 && key_code <= 126 {
                            Some(key_code as u8 as char)
                        } else {
                            None
                        };
                        
                        // Capture UI element for keyboard events if enabled
                        let mut ui_element = None;
                        if capture_ui_elements {
                            // Try to get the focused element first
                            ui_element = Self::get_focused_ui_element(&automation, Arc::clone(&process_info_cache_clone));
                            
                            // If no focused element, fall back to element at mouse position
                            if ui_element.is_none() {
                                if let Some((x, y)) = *last_mouse_pos.lock().unwrap() {
                                    ui_element = get_ui_element_at_point(&automation, x, y, Arc::clone(&process_info_cache_clone));
                                }
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
                            metadata: EventMetadata::from_ui_element(ui_element),
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
                            ModifierStates { ctrl: false, alt: false, shift: false, win: false }
                        };
                        
                        // Capture UI element for keyboard events if enabled
                        let mut ui_element = None;
                        if capture_ui_elements {
                            // Try to get the focused element first
                            ui_element = Self::get_focused_ui_element(&automation, Arc::clone(&process_info_cache_clone));
                            
                            // If no focused element, fall back to element at mouse position
                            if ui_element.is_none() {
                                if let Some((x, y)) = *last_mouse_pos.lock().unwrap() {
                                    ui_element = get_ui_element_at_point(&automation, x, y, Arc::clone(&process_info_cache_clone));
                                }
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
                            metadata: EventMetadata::from_ui_element(ui_element),
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
                                ui_element = get_ui_element_at_point(&automation, x, y, Arc::clone(&process_info_cache_clone));
                            }
                            
                            let mouse_event = MouseEvent {
                                event_type: MouseEventType::Down,
                                button: mouse_button,
                                position: Position { x, y },
                                scroll_delta: None,
                                drag_start: None,
                                metadata: EventMetadata::from_ui_element(ui_element),
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
                                ui_element = get_ui_element_at_point(&automation, x, y, Arc::clone(&process_info_cache_clone));
                            }
                            
                            let mouse_event = MouseEvent {
                                event_type: MouseEventType::Up,
                                button: mouse_button,
                                position: Position { x, y },
                                scroll_delta: None,
                                drag_start: None,
                                metadata: EventMetadata::from_ui_element(ui_element),
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
                            if now.duration_since(*last_time).as_millis() >= mouse_move_throttle as u128 {
                                *last_time = now;
                                true
                            } else {
                                false
                            }
                        };
                        
                        *last_mouse_pos.lock().unwrap() = Some((x, y));
                        
                        if should_record {
                            let mouse_event = MouseEvent {
                                event_type: MouseEventType::Move,
                                button: MouseButton::Left,
                                position: Position { x, y },
                                scroll_delta: None,
                                drag_start: None,
                                metadata: EventMetadata::from_ui_element(None),
                            };
                            let _ = event_tx.send(WorkflowEvent::Mouse(mouse_event));
                        }
                    }
                    EventType::Wheel { delta_x, delta_y } => {
                        if let Some((x, y)) = *last_mouse_pos.lock().unwrap() {
                            let mut ui_element = None;
                            if capture_ui_elements {
                                ui_element = get_ui_element_at_point(&automation, x, y, Arc::clone(&process_info_cache_clone));
                            }
                            
                            let mouse_event = MouseEvent {
                                event_type: MouseEventType::Wheel,
                                button: MouseButton::Middle,
                                position: Position { x, y },
                                scroll_delta: Some((delta_x as i32, delta_y as i32)),
                                drag_start: None,
                                metadata: EventMetadata::from_ui_element(ui_element),
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
    fn detect_hotkey(patterns: &[HotkeyPattern], active_keys: &HashMap<u32, bool>) -> Option<HotkeyEvent> {
        for pattern in patterns {
            if pattern.keys.iter().all(|&key| active_keys.get(&key).copied().unwrap_or(false)) {
                return Some(HotkeyEvent {
                    combination: format!("{:?}", pattern.keys), // TODO: Format properly
                    action: Some(pattern.action.clone()),
                    is_global: true,
                    metadata: EventMetadata::empty(), // TODO: Pass UI element context from caller
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
        let process_info_cache = Arc::clone(&self.process_info_cache);
        let capture_ui_elements = self.config.capture_ui_elements;
        
        thread::spawn(move || {
            let mut clipboard = match Clipboard::new() {
                Ok(cb) => cb,
                Err(e) => {
                    error!("Failed to initialize clipboard: {}", e);
                    return;
                }
            };
            
            let automation = if capture_ui_elements {
                match UIAutomation::new() {
                    Ok(auto) => Some(auto),
                    Err(e) => {
                        error!("Failed to create UIAutomation instance for clipboard monitoring: {}", e);
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
                        
                        let (truncated_content, truncated) = if content.len() > max_content_length {
                            (content[..max_content_length].to_string(), true)
                        } else {
                            (content.clone(), false)
                        };
                        
                        // Capture UI element if enabled
                        let ui_element = if let Some(ref automation) = automation {
                            Self::get_focused_ui_element(automation, Arc::clone(&process_info_cache))
                        } else {
                            None
                        };
                        
                        let clipboard_event = ClipboardEvent {
                            action: ClipboardAction::Copy, // Assume copy for content changes
                            content: Some(truncated_content),
                            content_size: Some(content.len()),
                            format: Some("text".to_string()),
                            truncated,
                            metadata: EventMetadata::from_ui_element(ui_element),
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
    
    /// Set up UI Automation event handlers
    fn setup_ui_automation_events(&self) -> Result<()> {
        let event_tx = self.event_tx.clone();
        let stop_indicator = Arc::clone(&self.stop_indicator);
        let process_info_cache = Arc::clone(&self.process_info_cache);
        let record_structure_changes = self.config.record_ui_structure_changes;
        let record_property_changes = self.config.record_ui_property_changes;
        let record_focus_changes = self.config.record_ui_focus_changes;
        
        if !record_structure_changes && !record_property_changes && !record_focus_changes {
            debug!("No UI Automation events enabled, skipping setup");
            return Ok(());
        }
        
        thread::spawn(move || {
            let automation = match UIAutomation::new() {
                Ok(auto) => auto,
                Err(e) => {
                    error!("Failed to create UIAutomation instance for events: {}", e);
                    return;
                }
            };
            
            info!("UI Automation event monitoring started");
            
            // Set up focus change event handler if enabled
            if record_focus_changes {
                let focus_event_tx = event_tx.clone();
                let focus_process_cache = Arc::clone(&process_info_cache);
                
                // Create a focus changed event handler struct
                struct FocusHandler {
                    event_tx: broadcast::Sender<WorkflowEvent>,
                    process_cache: Arc<DashMap<u32, (Option<String>, Option<String>)>>,
                }
                
                impl uiautomation::events::CustomFocusChangedEventHandler for FocusHandler {
                    fn handle(&self, sender: &uiautomation::UIElement) -> uiautomation::Result<()> {
                        let ui_element = WindowsRecorder::convert_win_element_to_ui_element(sender, Arc::clone(&self.process_cache));
                        
                        let focus_event = UiFocusChangedEvent {
                            previous_element: None, // TODO: Track previous element
                            metadata: EventMetadata::from_ui_element(ui_element),
                        };
                        
                        let _ = self.event_tx.send(WorkflowEvent::UiFocusChanged(focus_event));
                        Ok(())
                    }
                }
                
                let focus_handler = FocusHandler {
                    event_tx: focus_event_tx,
                    process_cache: focus_process_cache,
                };
                
                let focus_event_handler = uiautomation::events::UIFocusChangedEventHandler::from(focus_handler);
                
                // Register the focus change event handler
                if let Err(e) = automation.add_focus_changed_event_handler(None, &focus_event_handler) {
                    error!("Failed to register focus change event handler: {}", e);
                } else {
                    info!("Focus change event handler registered");
                }
            }
            
            // Set up property change event handler if enabled
            if record_property_changes {
                let property_event_tx = event_tx.clone();
                let property_process_cache = Arc::clone(&process_info_cache);
                
                // Create a property changed event handler using the proper closure type
                let property_handler: Box<uiautomation::events::CustomPropertyChangedEventHandlerFn> = Box::new(move |sender, property, value| {
                    // Only process certain properties to reduce noise
                    match property {
                        uiautomation::types::UIProperty::ValueValue |
                        uiautomation::types::UIProperty::Name |
                        uiautomation::types::UIProperty::HasKeyboardFocus => {
                            // Continue processing
                        }
                        _ => {
                            // Skip other properties to reduce noise
                            return Ok(());
                        }
                    }
                    
                    let ui_element = WindowsRecorder::convert_win_element_to_ui_element(sender, Arc::clone(&property_process_cache));
                    
                    // Only process text-related controls
                    let is_text_control = ui_element.as_ref()
                        .and_then(|elem| elem.control_type.as_ref())
                        .map(|ct| {
                            let ct_lower = ct.to_lowercase();
                            ct_lower.contains("edit") || 
                            ct_lower.contains("text") || 
                            ct_lower.contains("document") ||
                            ct_lower.contains("input")
                        })
                        .unwrap_or(false);
                    
                    if !is_text_control && property != uiautomation::types::UIProperty::HasKeyboardFocus {
                        return Ok(());
                    }
                    
                    // Try to get the actual current value from the element instead of relying on the event value
                    let actual_value = match property {
                        uiautomation::types::UIProperty::ValueValue => {
                            // For text controls, try multiple approaches to get the actual text
                            let mut text_content = None;
                            
                            // First, try the value from the event parameter
                            let event_value_str = value.to_string();
                            if !event_value_str.is_empty() && event_value_str != "EMPTY" && event_value_str != "null" {
                                text_content = Some(event_value_str);
                            }
                            
                            // If event value is not useful, try ValueValue property
                            if text_content.is_none() {
                                if let Ok(prop_value) = sender.get_property_value(uiautomation::types::UIProperty::ValueValue) {
                                    let value_str = prop_value.to_string();
                                    if !value_str.is_empty() && value_str != "EMPTY" && value_str != "null" {
                                        text_content = Some(value_str);
                                    }
                                }
                            }
                            
                            // If that didn't work, try getting the name
                            if text_content.is_none() {
                                if let Ok(name) = sender.get_name() {
                                    if !name.is_empty() {
                                        text_content = Some(name);
                                    }
                                }
                            }
                            
                            // Try getting the legacy accessible value
                            if text_content.is_none() {
                                if let Ok(legacy_value) = sender.get_property_value(uiautomation::types::UIProperty::LegacyIAccessibleValue) {
                                    let legacy_str = legacy_value.to_string();
                                    if !legacy_str.is_empty() && legacy_str != "EMPTY" && legacy_str != "null" {
                                        text_content = Some(legacy_str);
                                    }
                                }
                            }
                            
                            // Final fallback - indicate that we detected a change
                            if text_content.is_none() {
                                text_content = Some("(text changed - content not accessible via automation)".to_string());
                            }
                            
                            text_content
                        }
                        uiautomation::types::UIProperty::Name => {
                            let event_value_str = value.to_string();
                            if !event_value_str.is_empty() && event_value_str != "null" {
                                Some(event_value_str)
                            } else {
                                sender.get_name().ok()
                            }
                        }
                        uiautomation::types::UIProperty::HasKeyboardFocus => {
                            Some(value.to_string())
                        }
                        _ => {
                            Some(value.to_string())
                        }
                    };
                    
                    let property_event = UiPropertyChangedEvent {
                        property_name: format!("{:?}", property),
                        property_id: property as u32,
                        old_value: None, // TODO: Track old values
                        new_value: actual_value,
                        metadata: EventMetadata::from_ui_element(ui_element),
                    };
                    
                    let _ = property_event_tx.send(WorkflowEvent::UiPropertyChanged(property_event));
                    Ok(())
                });
                
                let property_event_handler = uiautomation::events::UIPropertyChangedEventHandler::from(property_handler);
                
                // Register property change event handler for common properties on the root element
                let root = automation.get_root_element().unwrap();
                let properties = vec![
                    uiautomation::types::UIProperty::ValueValue,
                    uiautomation::types::UIProperty::Name,
                    uiautomation::types::UIProperty::HasKeyboardFocus,
                ];
                
                if let Err(e) = automation.add_property_changed_event_handler(
                    &root,
                    uiautomation::types::TreeScope::Subtree,
                    None,
                    &property_event_handler,
                    &properties
                ) {
                    error!("Failed to register property change event handler: {}", e);
                } else {
                    info!("Property change event handler registered for ValueValue, Name, and HasKeyboardFocus");
                }
            }
            
            // Note: Structure change events are not yet implemented in the current version
            // of the uiautomation crate, so we'll keep the polling approach for now
            if record_structure_changes {
                warn!("Structure change events are not yet fully supported, using polling fallback");
                // TODO: Implement when structure change events become available
            }
            
            // Keep the thread alive while monitoring
            while !stop_indicator.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(100));
            }
            
            info!("UI Automation event monitoring stopped");
        });
        
        Ok(())
    }
    
    /// Convert a Windows UI element to our UiElement struct
    fn convert_win_element_to_ui_element(
        element: &uiautomation::UIElement,
        process_info_cache: Arc<DashMap<u32, (Option<String>, Option<String>)>>,
    ) -> Option<UiElement> {
        let name = element.get_name().ok();
        let automation_id = element.get_automation_id().ok();
        let class_name = element.get_classname().ok();
        let control_type = element.get_control_type().ok().map(|ct| ct.to_string());
        let process_id = element.get_process_id().ok().map(|pid| pid as u32);
        
        let is_enabled = element.is_enabled().ok();
        let has_keyboard_focus = element.has_keyboard_focus().ok();
        let value = element.get_property_value(uiautomation::types::UIProperty::Name)
            .ok().map(|v| v.to_string());
        
        let bounding_rect = element.get_bounding_rectangle().ok().map(|rect| {
            crate::events::Rect {
                x: rect.get_left() as i32,
                y: rect.get_top() as i32,
                width: rect.get_width() as i32,
                height: rect.get_height() as i32,
            }
        });
        
        let hierarchy_path = get_element_hierarchy_path(element);
        
        let (window_title, application_name) = if let Some(pid) = process_id {
            get_window_info_for_process(pid, Arc::clone(&process_info_cache))
        } else {
            (None, None)
        };
        
        Some(UiElement {
            name,
            automation_id,
            class_name,
            control_type,
            process_id,
            application_name,
            window_title,
            bounding_rect,
            is_enabled,
            has_keyboard_focus,
            hierarchy_path,
            value,
        })
    }
    
    /// Get the currently focused UI element
    fn get_focused_ui_element(
        automation: &UIAutomation,
        process_info_cache: Arc<DashMap<u32, (Option<String>, Option<String>)>>,
    ) -> Option<UiElement> {
        debug!("Getting focused UI element");
        
        match automation.get_focused_element() {
            Ok(element) => {
                debug!("Found focused UI element, gathering properties...");
                
                let name = element.get_name().ok();
                let automation_id = element.get_automation_id().ok();
                let class_name = element.get_classname().ok();
                let control_type = element.get_control_type().ok().map(|ct| ct.to_string());
                let process_id = element.get_process_id().ok().map(|pid| pid as u32);
                
                let is_enabled = element.is_enabled().ok();
                let has_keyboard_focus = Some(true); // This element has focus by definition
                let value = element.get_property_value(UIProperty::Name).ok().map(|v| v.to_string());
                
                let bounding_rect = element.get_bounding_rectangle().ok().map(|rect| {
                    crate::events::Rect {
                        x: rect.get_left() as i32,
                        y: rect.get_top() as i32,
                        width: rect.get_width() as i32,
                        height: rect.get_height() as i32,
                    }
                });
                
                let hierarchy_path = get_element_hierarchy_path(&element);
                
                let (window_title, application_name) = if let Some(pid) = process_id {
                    get_window_info_for_process(pid, Arc::clone(&process_info_cache))
                } else {
                    (None, None)
                };
                
                debug!(
                    "Focused UI element properties: name={:?}, automation_id={:?}, class={:?}, type={:?}, process={:?}, app={:?}, window={:?}",
                    name, automation_id, class_name, control_type, process_id, application_name, window_title
                );
                
                Some(UiElement {
                    name,
                    automation_id,
                    class_name,
                    control_type,
                    process_id,
                    application_name,
                    window_title,
                    bounding_rect,
                    is_enabled,
                    has_keyboard_focus,
                    hierarchy_path,
                    value,
                })
            }
            Err(e) => {
                debug!("Failed to get focused UI element: {}", e);
                None
            }
        }
    }
    
    /// Stop recording
    pub fn stop(&self) -> Result<()> {
        debug!("Stopping comprehensive Windows recorder...");
        self.stop_indicator.store(true, Ordering::SeqCst);
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

/// Get the UI element at the given point
#[cfg(target_os = "windows")]
fn get_ui_element_at_point(
    automation: &UIAutomation,
    x: i32,
    y: i32,
    process_info_cache: Arc<DashMap<u32, (Option<String>, Option<String>)>>,
) -> Option<UiElement> {
    debug!("Getting UI element at point ({}, {})", x, y);
    
    match automation.element_from_point(Point::from(POINT { x, y })) {
        Ok(element) => {
            
            let name = element.get_name().ok();
            let automation_id = element.get_automation_id().ok();
            let class_name = element.get_classname().ok();
            let control_type = element.get_control_type().ok().map(|ct| ct.to_string());
            let process_id = element.get_process_id().ok().map(|pid| pid as u32);
            
            let is_enabled = element.is_enabled().ok();
            let has_keyboard_focus = element.has_keyboard_focus().ok();
            let value = element.get_property_value(UIProperty::Name).ok().map(|v| v.to_string());
            
            let bounding_rect = element.get_bounding_rectangle().ok().map(|rect| {
                crate::events::Rect {
                    x: rect.get_left() as i32,
                    y: rect.get_top() as i32,
                    width: rect.get_width() as i32,
                    height: rect.get_height() as i32,
                }
            });
            
            let hierarchy_path = get_element_hierarchy_path(&element);
            
            let (window_title, application_name) = if let Some(pid) = process_id {
                get_window_info_for_process(pid, Arc::clone(&process_info_cache))
            } else {
                (None, None)
            };
            
            Some(UiElement {
                name,
                automation_id,
                class_name,
                control_type,
                process_id,
                application_name,
                window_title,
                bounding_rect,
                is_enabled,
                has_keyboard_focus,
                hierarchy_path,
                value,
            })
        }
        Err(e) => {
            debug!("Failed to get UI element: {}", e);
            None
        }
    }
}

/// Get the hierarchy path for an element
#[cfg(target_os = "windows")]
fn get_element_hierarchy_path(element: &WinUIElement) -> Option<String> {
    let mut path = Vec::new();
    let mut current = Some(element.clone());
    
    while let Some(elem) = current {
        let name = elem.get_name().ok().unwrap_or_default();
        let control_type = elem.get_control_type().ok().map(|ct| ct.to_string()).unwrap_or_default();
        let automation_id = elem.get_automation_id().ok().unwrap_or_default();
        
        let identifier = if !automation_id.is_empty() {
            format!("{}[{}]", control_type, automation_id)
        } else if !name.is_empty() {
            format!("{}[{}]", control_type, name)
        } else {
            control_type
        };
        
        path.push(identifier);
        
        current = elem.get_cached_parent().ok();
    }
    
    path.reverse();
    
    if path.is_empty() {
        None
    } else {
        Some(path.join("/"))
    }
}

/// Get window information for the given process ID
#[cfg(target_os = "windows")]
fn get_window_info_for_process(
    process_id: u32,
    cache: Arc<DashMap<u32, (Option<String>, Option<String>)>>,
) -> (Option<String>, Option<String>) {
    // Check cache first
    if let Some(cached_info) = cache.get(&process_id) {
        return cached_info.clone();
    }

    let (tx, rx) = mpsc::channel();
    let pid_clone = process_id;

    std::thread::spawn(move || {
        let app_name = match get_process_name_by_pid_recorder(pid_clone as i32) {
            Ok(name) => Some(name),
            Err(e) => {
                error!("Failed to get process name for PID {} (within get_window_info_for_process thread): {}", pid_clone, e);
                None
            }
        };

        let window_title = match UIAutomation::new() {
            Ok(automation) => {
                match automation.get_root_element() {
                    Ok(root) => {
                        let condition = match automation.create_property_condition(
                            UIProperty::ProcessId,
                            (pid_clone as i32).into(),
                            None,
                        ) {
                            Ok(cond) => cond,
                            Err(e) => {
                                error!("Failed to create process ID condition for PID {}: {}", pid_clone, e);
                                let _ = tx.send((None, app_name));
                                return;
                            }
                        };

                        match root.find_first(uiautomation::types::TreeScope::Children, &condition)
                            .or_else(|e| {
                                debug!("Could not find window for PID {} in children (error: {}), trying subtree.", pid_clone, e);
                                root.find_first(uiautomation::types::TreeScope::Subtree, &condition)
                            }) {
                            Ok(app_element) => app_element.get_name().ok(),
                            Err(e) => {
                                error!(
                                    "Failed to find app element for PID {}: {}",
                                    pid_clone,
                                    e
                                );
                                None
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to get root UI element for PID {}: {}", pid_clone, e);
                        None
                    }
                }
            }
            Err(e) => {
                error!("Failed to create UIAutomation instance for PID {}: {}", pid_clone, e);
                None
            }
        };
        let _ = tx.send((window_title, app_name));
    });

    // Keep a reasonable timeout for the native API call (should be much faster than PowerShell)
    let timeout_duration = Duration::from_secs(3);

    match rx.recv_timeout(timeout_duration) {
        Ok(result) => {
            // Cache the result before returning
            debug!("Caching result for PID {}: ({:?}, {:?})", process_id, result.0, result.1);
            cache.insert(process_id, result.clone());
            result
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            error!("Timeout getting window info for PID {}", process_id);
            // Fallback: try to get app_name directly if the combined operation timed out.
            // This call to get_process_name_by_pid_recorder also has its own timeout.
            let app_name_fallback = match get_process_name_by_pid_recorder(process_id as i32) {
                Ok(name) => Some(name),
                Err(e) => {
                    error!("Fallback failed to get process name for PID {} after timeout: {}", process_id, e);
                    None
                }
            };
            // Cache the fallback result as well, to avoid re-running the timeout logic immediately.
            // It's possible the UI automation part is what's consistently failing/timing out.
            let fallback_result = (None, app_name_fallback);
            debug!("Caching fallback result for PID {}: ({:?}, {:?})", process_id, fallback_result.0, fallback_result.1);
            cache.insert(process_id, fallback_result.clone());
            fallback_result
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            error!("Channel disconnected while getting window info for PID {}. This might indicate the spawned thread panicked.", process_id);
            // Fallback: attempt to get the app_name as a last resort.
            let app_name_fallback = match get_process_name_by_pid_recorder(process_id as i32) {
                Ok(name) => Some(name),
                Err(e) => {
                    error!("Fallback failed to get process name for PID {} after disconnect: {}", process_id, e);
                    None
                }
            };
            // Cache this fallback result too.
            let fallback_result = (None, app_name_fallback);
            debug!("Caching fallback result (after disconnect) for PID {}: ({:?}, {:?})", process_id, fallback_result.0, fallback_result.1);
            cache.insert(process_id, fallback_result.clone());
            fallback_result
        }
    }
}

// Helper function to get process name by PID using native Windows API, with a timeout
fn get_process_name_by_pid_recorder(pid: i32) -> std::result::Result<String, String> {
    let (tx, rx) = mpsc::channel();
    
    std::thread::spawn(move || {
        let _result = unsafe {
            // Create a snapshot of all processes
            let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
                Ok(handle) => handle,
                Err(e) => {
                    let _ = tx.send(Err(format!("Failed to create process snapshot: {}", e)));
                    return;
                }
            };
            
            if snapshot.is_invalid() {
                let _ = tx.send(Err("Invalid snapshot handle".to_string()));
                return;
            }
            
            // Ensure we close the handle when done
            let _guard = HandleGuard(snapshot);
            
            let mut process_entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            
            // Get the first process
            if Process32FirstW(snapshot, &mut process_entry).is_err() {
                let _ = tx.send(Err("Failed to get first process".to_string()));
                return;
            }
            
            // Iterate through processes to find the one with matching PID
            loop {
                if process_entry.th32ProcessID == pid as u32 {
                    // Convert the process name from wide string to String
                    let name_slice = &process_entry.szExeFile;
                    let name_len = name_slice.iter().position(|&c| c == 0).unwrap_or(name_slice.len());
                    let process_name = String::from_utf16_lossy(&name_slice[..name_len]);
                    
                    // Remove .exe extension if present
                    let clean_name = process_name
                        .strip_suffix(".exe")
                        .or_else(|| process_name.strip_suffix(".EXE"))
                        .unwrap_or(&process_name);
                    
                    let _ = tx.send(Ok(clean_name.to_string()));
                    return;
                }
                
                // Get the next process
                if Process32NextW(snapshot, &mut process_entry).is_err() {
                    break;
                }
            }
            
            let _ = tx.send(Err(format!("Process with PID {} not found", pid)));
        };
    });

    // Keep a reasonable timeout for the native API call (should be much faster than PowerShell)
    let timeout_duration = Duration::from_secs(2);

    match rx.recv_timeout(timeout_duration) {
        Ok(result_from_thread) => result_from_thread,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(format!("Timeout getting process name for PID {} via Windows API", pid)),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(format!("Channel disconnected while getting process name for PID {} via Windows API. Thread might have panicked.", pid)),
    }
}

// RAII guard to ensure handle is closed
struct HandleGuard(HANDLE);

impl Drop for HandleGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn test_get_process_name_by_pid_recorder_current_process() {
        // Test with the current process PID
        let current_pid = process::id() as i32;
        let result = get_process_name_by_pid_recorder(current_pid);
        
        assert!(result.is_ok(), "Should be able to get current process name");
        let process_name = result.unwrap();
        
        // The process name should be a valid non-empty string
        assert!(!process_name.is_empty(), "Process name should not be empty");
        
        // Should not contain .exe extension
        assert!(!process_name.ends_with(".exe"), "Process name should not contain .exe extension");
        assert!(!process_name.ends_with(".EXE"), "Process name should not contain .EXE extension");
        
        // Should be a reasonable process name (alphanumeric, hyphens, underscores)
        assert!(process_name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_'), 
               "Process name should contain only alphanumeric characters, hyphens, and underscores: {}", process_name);
        
        println!("Current process name: {}", process_name);
    }

    #[test]
    fn test_get_process_name_by_pid_recorder_invalid_pid() {
        // Test with an invalid PID (very high number unlikely to exist)
        let invalid_pid = 999999;
        let result = get_process_name_by_pid_recorder(invalid_pid);
        
        assert!(result.is_err(), "Should fail for invalid PID");
        let error_msg = result.unwrap_err();
        assert!(error_msg.contains("Process with PID"), "Error should mention the PID");
    }

    #[test]
    fn test_get_process_name_by_pid_recorder_timeout() {
        // Test that the function respects timeout
        let start_time = std::time::Instant::now();
        
        // Use an invalid PID to trigger the "not found" path
        let invalid_pid = 999999;
        let result = get_process_name_by_pid_recorder(invalid_pid);
        
        let elapsed = start_time.elapsed();
        
        // Should complete within reasonable time (much less than the 2-second timeout)
        assert!(elapsed < Duration::from_millis(1000), 
               "Function should complete quickly for invalid PID, took: {:?}", elapsed);
        
        assert!(result.is_err(), "Should fail for invalid PID");
    }

    #[test]
    fn test_get_process_name_by_pid_recorder_system_process() {
        // Test with a known system process (PID 4 is usually System on Windows)
        let system_pid = 4;
        let result = get_process_name_by_pid_recorder(system_pid);
        
        // This might succeed or fail depending on permissions, but shouldn't panic
        match result {
            Ok(name) => {
                assert!(!name.is_empty(), "Process name should not be empty");
                assert!(!name.ends_with(".exe"), "Process name should not contain .exe extension");
                println!("System process name: {}", name);
            }
            Err(e) => {
                println!("Expected: Could not access system process: {}", e);
            }
        }
    }

    #[test]
    fn test_handle_guard_recorder() {
        // Test that HandleGuard properly cleans up handles in recorder context
        use windows::Win32::System::Diagnostics::ToolHelp::CreateToolhelp32Snapshot;
        
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).unwrap();
            assert!(!snapshot.is_invalid(), "Snapshot should be valid");
            
            // Create and immediately drop the guard
            {
                let _guard = HandleGuard(snapshot);
                // Guard should be alive here
            }
            // Guard should be dropped and handle closed here
            
            // Test passes if no panic occurs
        }
    }

    #[test]
    fn test_get_window_info_for_process_current() {
        // Test getting window info for current process
        let current_pid = process::id();
        let cache = Arc::new(DashMap::new());
        
        let (window_title, app_name) = get_window_info_for_process(current_pid, cache.clone());
        
        // App name should be available (from our native API)
        assert!(app_name.is_some(), "Should get app name for current process");
        let app_name = app_name.unwrap();
        assert!(!app_name.is_empty(), "App name should not be empty");
        assert!(!app_name.ends_with(".exe"), "App name should not contain .exe extension");
        
        // Window title might or might not be available for test processes
        println!("Current process - App: {:?}, Window: {:?}", app_name, window_title);
        
        // Test caching - second call should be faster and return same result
        let start_time = std::time::Instant::now();
        let (window_title2, app_name2) = get_window_info_for_process(current_pid, cache);
        let elapsed = start_time.elapsed();
        
        assert!(elapsed < Duration::from_millis(100), "Cached call should be very fast");
        assert_eq!(app_name, app_name2.unwrap(), "Cached result should match");
        assert_eq!(window_title, window_title2, "Cached window title should match");
    }

    #[test]
    fn test_get_window_info_for_process_invalid_pid() {
        // Test with invalid PID
        let invalid_pid = 999999;
        let cache = Arc::new(DashMap::new());
        
        let (window_title, app_name) = get_window_info_for_process(invalid_pid, cache);
        
        // Should handle gracefully
        assert!(window_title.is_none(), "Should not get window title for invalid PID");
        assert!(app_name.is_none(), "Should not get app name for invalid PID");
    }

    #[test]
    fn test_get_window_info_for_process_caching() {
        // Test that caching works correctly
        let current_pid = process::id();
        let cache = Arc::new(DashMap::new());
        
        // First call - should populate cache
        assert!(cache.is_empty(), "Cache should start empty");
        
        let (window_title1, app_name1) = get_window_info_for_process(current_pid, cache.clone());
        
        assert!(!cache.is_empty(), "Cache should be populated after first call");
        assert!(cache.contains_key(&current_pid), "Cache should contain current PID");
        
        // Second call - should use cache
        let start_time = std::time::Instant::now();
        let (window_title2, app_name2) = get_window_info_for_process(current_pid, cache.clone());
        let elapsed = start_time.elapsed();
        
        // Cached call should be very fast
        assert!(elapsed < Duration::from_millis(50), "Cached call should be very fast");
        
        // Results should be identical
        assert_eq!(window_title1, window_title2, "Cached window title should match");
        assert_eq!(app_name1, app_name2, "Cached app name should match");
    }

    #[test]
    fn test_multiple_concurrent_process_lookups() {
        // Test that multiple concurrent lookups work correctly
        let current_pid = process::id() as i32;
        let (tx, rx) = mpsc::channel();
        
        // Spawn multiple threads doing lookups
        for i in 0..5 {
            let tx_clone = tx.clone();
            std::thread::spawn(move || {
                let result = get_process_name_by_pid_recorder(current_pid);
                tx_clone.send((i, result)).unwrap();
            });
        }
        
        // Collect results
        let mut results = Vec::new();
        for _ in 0..5 {
            let (thread_id, result) = rx.recv_timeout(Duration::from_secs(5)).unwrap();
            results.push((thread_id, result));
        }
        
        // All should succeed and return the same name
        let mut process_names = Vec::new();
        for (thread_id, result) in results {
            assert!(result.is_ok(), "Thread {} should succeed", thread_id);
            process_names.push(result.unwrap());
        }
        
        // All names should be identical
        let first_name = &process_names[0];
        for name in &process_names[1..] {
            assert_eq!(first_name, name, "All concurrent lookups should return same name");
        }
        
        println!("All {} threads returned: {}", process_names.len(), first_name);
    }

    #[test]
    fn test_process_name_consistency_with_main_lib() {
        // Test that the recorder's implementation is consistent
        let current_pid = process::id() as i32;
        
        // Get name using recorder function multiple times to ensure consistency
        let result1 = get_process_name_by_pid_recorder(current_pid);
        let result2 = get_process_name_by_pid_recorder(current_pid);
        
        match (result1, result2) {
            (Ok(name1), Ok(name2)) => {
                assert_eq!(name1, name2, 
                          "Multiple calls should return same process name");
                println!("Both calls returned: {}", name1);
            }
            (Ok(name), Err(err)) => {
                panic!("Inconsistent results: first succeeded ({}), second failed ({})", name, err);
            }
            (Err(err), Ok(name)) => {
                panic!("Inconsistent results: first failed ({}), second succeeded ({})", err, name);
            }
            (Err(err1), Err(err2)) => {
                println!("Both calls failed consistently - Err1: {}, Err2: {}", err1, err2);
            }
        }
    }

    #[cfg(test)]
    mod performance_tests {
        use super::*;
        use std::time::Instant;

        #[test]
        fn test_performance_vs_timeout() {
            // Test that native API is much faster than the timeout
            let current_pid = process::id() as i32;
            let start_time = Instant::now();
            
            let result = get_process_name_by_pid_recorder(current_pid);
            let elapsed = start_time.elapsed();
            
            assert!(result.is_ok(), "Should succeed for current process");
            
            // Should complete in much less than the 2-second timeout
            assert!(elapsed < Duration::from_millis(500), 
                   "Native API should be fast, took: {:?}", elapsed);
            
            println!("Process lookup took: {:?}", elapsed);
        }

        #[test]
        fn test_batch_performance() {
            // Test performance of multiple lookups
            let current_pid = process::id() as i32;
            let iterations = 10;
            
            let start_time = Instant::now();
            
            for _ in 0..iterations {
                let result = get_process_name_by_pid_recorder(current_pid);
                assert!(result.is_ok(), "Each lookup should succeed");
            }
            
            let elapsed = start_time.elapsed();
            let avg_time = elapsed / iterations;
            
            println!("{} lookups took {:?}, average: {:?}", iterations, elapsed, avg_time);
            
            // Each lookup should be very fast
            assert!(avg_time < Duration::from_millis(100), 
                   "Average lookup time should be fast: {:?}", avg_time);
        }
    }
} 



