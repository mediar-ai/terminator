use crate::{
    KeyboardEvent, MouseButton, MouseEvent, MouseEventType, Position, UiElement,
    WorkflowEvent, WorkflowRecorderConfig, ClipboardEvent, ClipboardAction,
    FileEvent, FileAction, ScrollEvent, ScrollDirection,
    DragDropEvent, HotkeyEvent, Rect, Result, TextSelectionEvent, SelectionMethod
};
use std::{
    sync::{Arc, Mutex, mpsc},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
    collections::{HashMap, HashSet},
    thread,
    path::PathBuf,
};
use tokio::sync::broadcast;
use tracing::{debug, info, error, warn};
use rdev::{EventType, Button, Key};
use uiautomation::{UIAutomation, UIElement as WinUIElement};
use uiautomation::types::{Point, UIProperty};
use windows::Win32::Foundation::POINT;
use dashmap::DashMap;
use arboard::Clipboard;
use notify::{Watcher, RecursiveMode, Event, EventKind};
use regex::Regex;

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
    
    /// Window tracking for geometry changes
    window_tracker: Arc<Mutex<HashMap<isize, WindowInfo>>>,
    
    /// Drag state tracking
    drag_state: Arc<Mutex<Option<DragState>>>,
    
    /// Text input buffer for batching character events
    text_buffer: Arc<Mutex<TextBuffer>>,
    
    /// Last mouse move time for throttling
    last_mouse_move_time: Arc<Mutex<Instant>>,
    
    /// Known hotkey patterns
    hotkey_patterns: Arc<Vec<HotkeyPattern>>,
    
    /// File system watcher
    _file_watcher: Option<notify::RecommendedWatcher>,
    
    /// Active windows set for tracking
    active_windows: Arc<Mutex<HashSet<isize>>>,
}

#[derive(Debug, Clone)]
struct ModifierStates {
    ctrl: bool,
    alt: bool,
    shift: bool,
    win: bool,
}

#[derive(Debug, Clone)]
struct WindowInfo {
    title: String,
    bounds: Rect,
    state: String,
    process_id: u32,
}

#[derive(Debug)]
struct DragState {
    start_pos: Position,
    start_element: Option<UiElement>,
    is_dragging: bool,
}

#[derive(Debug)]
struct TextBuffer {
    content: String,
    target_element: Option<UiElement>,
    last_update: Instant,
    selection_start: Option<u32>,
    selection_end: Option<u32>,
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
        let window_tracker = Arc::new(Mutex::new(HashMap::new()));
        let drag_state = Arc::new(Mutex::new(None));
        let text_buffer = Arc::new(Mutex::new(TextBuffer {
            content: String::new(),
            target_element: None,
            last_update: Instant::now(),
            selection_start: None,
            selection_end: None,
        }));
        let last_mouse_move_time = Arc::new(Mutex::new(Instant::now()));
        
        // Initialize hotkey patterns
        let hotkey_patterns = Arc::new(Self::initialize_hotkey_patterns());
        
        let active_windows = Arc::new(Mutex::new(HashSet::new()));
        
        // Set up file system watcher if enabled
        let file_watcher = if config.monitor_file_system {
            Some(Self::setup_file_watcher(&config, event_tx.clone())?)
        } else {
            None
        };
        
        let mut recorder = Self {
            event_tx,
            config,
            last_mouse_pos,
            stop_indicator,
            process_info_cache,
            modifier_states,
            last_clipboard_hash,
            window_tracker,
            drag_state,
            text_buffer,
            last_mouse_move_time,
            hotkey_patterns,
            _file_watcher: file_watcher,
            active_windows,
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
    
    /// Set up file system watcher
    fn setup_file_watcher(
        config: &WorkflowRecorderConfig,
        event_tx: broadcast::Sender<WorkflowEvent>,
    ) -> Result<notify::RecommendedWatcher> {
        let mut watcher = notify::recommended_watcher(move |res: std::result::Result<Event, notify::Error>| {
            match res {
                Ok(event) => {
                    if let Some(workflow_event) = Self::convert_file_event(&event) {
                        let _ = event_tx.send(workflow_event);
                    }
                }
                Err(e) => error!("File watcher error: {:?}", e),
            }
        })?;

        let paths = if config.file_system_watch_paths.is_empty() {
            vec![std::env::current_dir().unwrap_or_else(|_| PathBuf::from("C:\\"))]
        } else {
            config.file_system_watch_paths.iter().map(PathBuf::from).collect()
        };

        for path in paths {
            if let Err(e) = watcher.watch(&path, RecursiveMode::Recursive) {
                warn!("Failed to watch path {:?}: {}", path, e);
            }
        }

        Ok(watcher)
    }
    
    /// Convert notify file event to workflow event
    fn convert_file_event(event: &Event) -> Option<WorkflowEvent> {
        let action = match &event.kind {
            EventKind::Create(_) => FileAction::Created,
            EventKind::Modify(_) => FileAction::Modified,
            EventKind::Remove(_) => FileAction::Deleted,
            _ => return None,
        };

        let path = event.paths.first()?.to_string_lossy().to_string();
        let file_type = path.split('.').last().map(|s| s.to_string());

        Some(WorkflowEvent::File(FileEvent {
            action,
            path,
            destination_path: None,
            size: None,
            file_type,
            source_application: None,
        }))
    }
    
    /// Set up comprehensive event listeners
    fn setup_comprehensive_listeners(&mut self) -> Result<()> {
        // Main input event listener (enhanced from original)
        self.setup_enhanced_input_listener()?;
        
        // Clipboard monitoring
        if self.config.record_clipboard {
            self.setup_clipboard_monitor()?;
        }
        
        // Window event monitoring
        if self.config.record_window {
            self.setup_window_monitor()?;
        }
        
        // System event monitoring
        if self.config.record_system_events {
            self.setup_system_monitor()?;
        }
        
        // Application monitoring
        if self.config.record_applications {
            self.setup_application_monitor()?;
        }
        
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
        let drag_state = Arc::clone(&self.drag_state);
        let text_buffer = Arc::clone(&self.text_buffer);
        let last_mouse_move_time = Arc::clone(&self.last_mouse_move_time);
        let hotkey_patterns = Arc::clone(&self.hotkey_patterns);
        let mouse_move_throttle = self.config.mouse_move_throttle_ms;
        let track_modifiers = self.config.track_modifier_states;
        let record_scroll = self.config.record_scroll;
        let record_drag_drop = self.config.record_drag_drop;
        let record_hotkeys = self.config.record_hotkeys;
        let record_text_selection = self.config.record_text_selection;
        let max_text_selection_length = self.config.max_text_selection_length;
        let min_drag_distance = self.config.min_drag_distance;
        
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
                        
                        let keyboard_event = KeyboardEvent {
                            key_code,
                            is_key_down: true,
                            ctrl_pressed: modifiers.ctrl,
                            alt_pressed: modifiers.alt,
                            shift_pressed: modifiers.shift,
                            win_pressed: modifiers.win,
                            character,
                            scan_code: None, // TODO: Get actual scan code
                        };
                        
                        // Add to text buffer if it's a printable character
                        if let Some(ch) = character {
                            Self::add_to_text_buffer(&text_buffer, ch, &automation, &last_mouse_pos, &process_info_cache_clone);
                        }
                        
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
                        
                        let keyboard_event = KeyboardEvent {
                            key_code,
                            is_key_down: false,
                            ctrl_pressed: modifiers.ctrl,
                            alt_pressed: modifiers.alt,
                            shift_pressed: modifiers.shift,
                            win_pressed: modifiers.win,
                            character: None,
                            scan_code: None,
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
                            
                            // Handle drag start
                            if record_drag_drop && mouse_button == MouseButton::Left {
                                *drag_state.lock().unwrap() = Some(DragState {
                                    start_pos: Position { x, y },
                                    start_element: ui_element.clone(),
                                    is_dragging: false,
                                });
                            }
                            
                            let mouse_event = MouseEvent {
                                event_type: MouseEventType::Down,
                                button: mouse_button,
                                position: Position { x, y },
                                ui_element,
                                scroll_delta: None,
                                drag_start: None,
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
                            
                            // Handle drag end
                            if record_drag_drop && mouse_button == MouseButton::Left {
                                if let Some(drag) = drag_state.lock().unwrap().take() {
                                    let distance = ((x - drag.start_pos.x).pow(2) + (y - drag.start_pos.y).pow(2)) as f64;
                                    let distance_sqrt = distance.sqrt();
                                    
                                    if distance_sqrt > min_drag_distance {
                                        // Check if this is a text selection by examining the UI elements
                                        let is_text_selection = Self::is_text_selection_drag(
                                            &drag.start_element,
                                            &ui_element,
                                            &automation,
                                            drag.start_pos,
                                            Position { x, y }
                                        );
                                        
                                        if is_text_selection && record_text_selection {
                                            // Try to get selected text from the UI automation
                                            if let Some(selected_text) = Self::get_selected_text(&automation) {
                                                let (truncated_text, truncated) = if selected_text.len() > max_text_selection_length {
                                                    (selected_text[..max_text_selection_length].to_string(), true)
                                                } else {
                                                    (selected_text.clone(), false)
                                                };
                                                
                                                let text_selection_event = TextSelectionEvent {
                                                    selected_text: truncated_text,
                                                    start_position: drag.start_pos,
                                                    end_position: Position { x, y },
                                                    target_element: ui_element.clone(),
                                                    selection_method: SelectionMethod::MouseDrag,
                                                    selection_length: selected_text.len(),
                                                    is_partial_selection: truncated,
                                                    application: ui_element.as_ref()
                                                        .and_then(|elem| elem.application_name.clone()),
                                                };
                                                let _ = event_tx.send(WorkflowEvent::TextSelection(text_selection_event));
                                            }
                                        } else if drag.is_dragging {
                                            // Regular drag and drop operation
                                            let drag_drop_event = DragDropEvent {
                                                start_position: drag.start_pos,
                                                end_position: Position { x, y },
                                                source_element: drag.start_element,
                                                target_element: ui_element.clone(),
                                                data_type: None, // TODO: Detect data type
                                                content: None, // TODO: Extract content if text
                                                success: true, // TODO: Detect success
                                            };
                                            let _ = event_tx.send(WorkflowEvent::DragDrop(drag_drop_event));
                                        }
                                    }
                                }
                            }
                            
                            let mouse_event = MouseEvent {
                                event_type: MouseEventType::Up,
                                button: mouse_button,
                                position: Position { x, y },
                                ui_element,
                                scroll_delta: None,
                                drag_start: None,
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
                        
                        // Update drag state
                        if record_drag_drop {
                            if let Some(ref mut drag) = *drag_state.lock().unwrap() {
                                let distance = ((x - drag.start_pos.x).pow(2) + (y - drag.start_pos.y).pow(2)) as f64;
                                if distance.sqrt() > 5.0 { // 5 pixel threshold
                                    drag.is_dragging = true;
                                }
                            }
                        }
                        
                        if should_record {
                            let mouse_event = MouseEvent {
                                event_type: MouseEventType::Move,
                                button: MouseButton::Left,
                                position: Position { x, y },
                                ui_element: None,
                                scroll_delta: None,
                                drag_start: None,
                            };
                            let _ = event_tx.send(WorkflowEvent::Mouse(mouse_event));
                        }
                    }
                    EventType::Wheel { delta_x, delta_y } => {
                        if record_scroll {
                            if let Some((x, y)) = *last_mouse_pos.lock().unwrap() {
                                let mut ui_element = None;
                                if capture_ui_elements {
                                    ui_element = get_ui_element_at_point(&automation, x, y, Arc::clone(&process_info_cache_clone));
                                }
                                
                                let scroll_event = ScrollEvent {
                                    delta: (delta_x as i32, delta_y as i32),
                                    position: Position { x, y },
                                    target_element: ui_element.clone(),
                                    direction: if delta_x != 0 && delta_y != 0 {
                                        ScrollDirection::Both
                                    } else if delta_x != 0 {
                                        ScrollDirection::Horizontal
                                    } else {
                                        ScrollDirection::Vertical
                                    },
                                };
                                let _ = event_tx.send(WorkflowEvent::Scroll(scroll_event));
                                
                                let mouse_event = MouseEvent {
                                    event_type: MouseEventType::Wheel,
                                    button: MouseButton::Middle,
                                    position: Position { x, y },
                                    ui_element,
                                    scroll_delta: Some((delta_x as i32, delta_y as i32)),
                                    drag_start: None,
                                };
                                let _ = event_tx.send(WorkflowEvent::Mouse(mouse_event));
                            }
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
                    application: None, // TODO: Get current application
                    is_global: true,
                });
            }
        }
        None
    }
    
    /// Add character to text buffer
    fn add_to_text_buffer(
        text_buffer: &Arc<Mutex<TextBuffer>>,
        ch: char,
        automation: &UIAutomation,
        last_mouse_pos: &Arc<Mutex<Option<(i32, i32)>>>,
        process_cache: &Arc<DashMap<u32, (Option<String>, Option<String>)>>,
    ) {
        let mut buffer = text_buffer.lock().unwrap();
        buffer.content.push(ch);
        buffer.last_update = Instant::now();
        
        // Get current UI element for context
        if let Some((x, y)) = *last_mouse_pos.lock().unwrap() {
            buffer.target_element = get_ui_element_at_point(automation, x, y, Arc::clone(process_cache));
        }
    }
    
    /// Set up clipboard monitoring
    fn setup_clipboard_monitor(&self) -> Result<()> {
        let event_tx = self.event_tx.clone();
        let stop_indicator = Arc::clone(&self.stop_indicator);
        let last_hash = Arc::clone(&self.last_clipboard_hash);
        let max_content_length = self.config.max_clipboard_content_length;
        
        thread::spawn(move || {
            let mut clipboard = match Clipboard::new() {
                Ok(cb) => cb,
                Err(e) => {
                    error!("Failed to initialize clipboard: {}", e);
                    return;
                }
            };
            
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
                        
                        let clipboard_event = ClipboardEvent {
                            action: ClipboardAction::Copy, // Assume copy for content changes
                            content: Some(truncated_content),
                            content_size: Some(content.len()),
                            format: Some("text".to_string()),
                            source_application: None, // TODO: Detect source app
                            truncated,
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
    
    /// Set up window monitoring
    fn setup_window_monitor(&self) -> Result<()> {
        // Implementation for window events monitoring
        // This would involve Windows API hooks for window messages
        // For now, we'll implement basic window tracking
        
        let _event_tx = self.event_tx.clone();
        let stop_indicator = Arc::clone(&self.stop_indicator);
        let _window_tracker = Arc::clone(&self.window_tracker);
        let _active_windows = Arc::clone(&self.active_windows);
        
        thread::spawn(move || {
            while !stop_indicator.load(Ordering::SeqCst) {
                // TODO: Implement proper window enumeration and change detection
                // This is a placeholder for the comprehensive window monitoring
                thread::sleep(Duration::from_millis(500));
            }
        });
        
        Ok(())
    }
    
    /// Set up system event monitoring
    fn setup_system_monitor(&self) -> Result<()> {
        // Implementation for system events like power, network, etc.
        // This would involve various Windows API event subscriptions
        Ok(())
    }
    
    /// Set up application monitoring
    fn setup_application_monitor(&self) -> Result<()> {
        // Implementation for application lifecycle monitoring
        // This would involve process enumeration and change detection
        Ok(())
    }
    
    /// Check if a drag operation is likely a text selection
    fn is_text_selection_drag(
        start_element: &Option<UiElement>,
        end_element: &Option<UiElement>,
        _automation: &UIAutomation,
        _start_pos: Position,
        _end_pos: Position,
    ) -> bool {
        // Check if both elements are text-related controls
        if let (Some(start_elem), Some(end_elem)) = (start_element, end_element) {
            let text_control_types = ["Text", "Edit", "Document", "Group", "Pane"];
            
            let start_is_text = start_elem.control_type.as_ref()
                .map(|ct| text_control_types.contains(&ct.as_str()))
                .unwrap_or(false);
                
            let end_is_text = end_elem.control_type.as_ref()
                .map(|ct| text_control_types.contains(&ct.as_str()))
                .unwrap_or(false);
            
            // Text selection typically happens within the same element or closely related elements
            let same_application = start_elem.application_name == end_elem.application_name;
            let same_window = start_elem.window_title == end_elem.window_title;
            
            return start_is_text && end_is_text && same_application && same_window;
        }
        
        false
    }
    
    /// Try to get selected text using UI Automation
    fn get_selected_text(automation: &UIAutomation) -> Option<String> {
        // Get the currently focused element
        if let Ok(focused_element) = automation.get_focused_element() {
            // Try to get the element's value - this works for many text controls
            if let Ok(value) = focused_element.get_property_value(UIProperty::Name) {
                let value_str = value.to_string();
                if !value_str.is_empty() && value_str != "null" {
                    return Some(value_str);
                }
            }
            
            // Try to get the element's name as fallback
            if let Ok(name) = focused_element.get_name() {
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
        
        None
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
            debug!("Found UI element, gathering properties...");
            
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
                debug!("Getting window info for process {}", pid);
                get_window_info_for_process(pid, Arc::clone(&process_info_cache))
            } else {
                (None, None)
            };
            
            debug!(
                "UI element properties: name={:?}, automation_id={:?}, class={:?}, type={:?}, process={:?}, app={:?}, window={:?}",
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
        debug!("Cache hit for PID {}: ({:?}, {:?})", process_id, cached_info.0, cached_info.1);
        return cached_info.clone();
    }
    debug!("Cache miss for PID {}", process_id);

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

// Helper function to get process name by PID using PowerShell, with a timeout
fn get_process_name_by_pid_recorder(pid: i32) -> std::result::Result<String, String> {
    let (tx, rx) = mpsc::channel();
    let command_str = format!(
        "Get-Process -Id {} | Select-Object -ExpandProperty ProcessName",
        pid
    );

    std::thread::spawn(move || {
        let result = match std::process::Command::new("powershell")
            .args(["-NoProfile", "-WindowStyle", "hidden", "-Command", &command_str])
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    let process_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if process_name.is_empty() {
                        Err(format!("Process name not found for PID {} (stdout was empty)", pid))
                    } else {
                        Ok(process_name)
                    }
                } else {
                    let err_msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    Err(format!(
                        "PowerShell command failed to get process name for PID {}: {}",
                        pid, err_msg
                    ))
                }
            }
            Err(e) => Err(format!(
                "Failed to execute PowerShell to get process name for PID {}: {}",
                pid, e
            )),
        };
        let _ = tx.send(result);
    });

    let timeout_duration = Duration::from_secs(2);

    match rx.recv_timeout(timeout_duration) {
        Ok(result_from_thread) => result_from_thread,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(format!("Timeout getting process name for PID {} via PowerShell", pid)),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(format!("Channel disconnected while getting process name for PID {} via PowerShell. Thread might have panicked.", pid)),
    }
} 



