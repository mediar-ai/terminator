use crate::{
    KeyboardEvent, MouseButton, MouseEvent, MouseEventType, Position, UiElement,
    WorkflowEvent, Result, WorkflowRecorderConfig
};
use std::{
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;
use tracing::{debug, info, error};
use rdev::{EventType, Button, Key};
use uiautomation::{UIAutomation, UIElement as WinUIElement};
use uiautomation::types::{Point, UIProperty};
use windows::Win32::Foundation::POINT;

/// The Windows-specific recorder
pub struct WindowsRecorder {
    /// The event sender
    event_tx: broadcast::Sender<WorkflowEvent>,
    
    /// The configuration
    config: WorkflowRecorderConfig,
    
    /// The last mouse position
    last_mouse_pos: Arc<Mutex<Option<(i32, i32)>>>,
}

#[cfg(target_os = "windows")]
impl WindowsRecorder {
    /// Create a new Windows recorder
    pub fn new(
        config: WorkflowRecorderConfig,
        event_tx: broadcast::Sender<WorkflowEvent>,
    ) -> Result<Self> {
        info!("[LOG] windows.rs loaded");
        debug!("Initializing Windows recorder with config: {:?}", config);
        
        let last_mouse_pos = Arc::new(Mutex::new(None));
        
        let mut recorder = Self {
            event_tx,
            config,
            last_mouse_pos,
        };
        
        // Set up event listener
        debug!("Setting up event listener...");
        recorder.setup_event_listener()?;
        debug!("Event listener setup completed");
        
        Ok(recorder)
    }
    
    /// Set up the event listener
    fn setup_event_listener(&mut self) -> Result<()> {
        let event_tx = self.event_tx.clone();
        let last_mouse_pos = Arc::clone(&self.last_mouse_pos);
        let capture_ui_elements = self.config.capture_ui_elements;
        
        std::thread::spawn(move || {
            // Create a new UIAutomation instance in this thread
            let automation = match UIAutomation::new() {
                Ok(auto) => auto,
                Err(e) => {
                    error!("Failed to create UIAutomation instance: {}", e);
                    return;
                }
            };
            
            if let Err(error) = rdev::listen(move |event| {
                match event.event_type {
                    EventType::KeyPress(key) => {
                        let keyboard_event = KeyboardEvent {
                            key_code: key_to_u32(&key),
                            is_key_down: true,
                            ctrl_pressed: false, // TODO: Track modifier state
                            alt_pressed: false,
                            shift_pressed: false,
                            win_pressed: false,
                        };
                        let _ = event_tx.send(WorkflowEvent::Keyboard(keyboard_event));
                    }
                    EventType::KeyRelease(key) => {
                        let keyboard_event = KeyboardEvent {
                            key_code: key_to_u32(&key),
                            is_key_down: false,
                            ctrl_pressed: false, // TODO: Track modifier state
                            alt_pressed: false,
                            shift_pressed: false,
                            win_pressed: false,
                        };
                        let _ = event_tx.send(WorkflowEvent::Keyboard(keyboard_event));
                    }
                    EventType::ButtonPress(button) => {
                        if let Some((x, y)) = *last_mouse_pos.lock().unwrap() {
                            let button = match button {
                                Button::Left => MouseButton::Left,
                                Button::Right => MouseButton::Right,
                                Button::Middle => MouseButton::Middle,
                                _ => return,
                            };
                            
                            let mut ui_element = None;
                            if capture_ui_elements {
                                ui_element = get_ui_element_at_point(&automation, x, y);
                            }
                            
                            let mouse_event = MouseEvent {
                                event_type: MouseEventType::Down,
                                button,
                                position: Position { x, y },
                                ui_element,
                            };
                            let _ = event_tx.send(WorkflowEvent::Mouse(mouse_event));
                        }
                    }
                    EventType::ButtonRelease(button) => {
                        if let Some((x, y)) = *last_mouse_pos.lock().unwrap() {
                            let button = match button {
                                Button::Left => MouseButton::Left,
                                Button::Right => MouseButton::Right,
                                Button::Middle => MouseButton::Middle,
                                _ => return,
                            };
                            
                            let mut ui_element = None;
                            if capture_ui_elements {
                                ui_element = get_ui_element_at_point(&automation, x, y);
                            }
                            
                            let mouse_event = MouseEvent {
                                event_type: MouseEventType::Up,
                                button,
                                position: Position { x, y },
                                ui_element,
                            };
                            let _ = event_tx.send(WorkflowEvent::Mouse(mouse_event));
                        }
                    }
                    EventType::MouseMove { x, y } => {
                        let x = x as i32;
                        let y = y as i32;
                        *last_mouse_pos.lock().unwrap() = Some((x, y));
                        
                        let mouse_event = MouseEvent {
                            event_type: MouseEventType::Move,
                            button: MouseButton::Left,
                            position: Position { x, y },
                            ui_element: None,
                        };
                        let _ = event_tx.send(WorkflowEvent::Mouse(mouse_event));
                    }
                    EventType::Wheel { delta_x: _, delta_y: _ } => {
                        if let Some((x, y)) = *last_mouse_pos.lock().unwrap() {
                            let mouse_event = MouseEvent {
                                event_type: MouseEventType::Wheel,
                                button: MouseButton::Middle,
                                position: Position { x, y },
                                ui_element: None,
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
    
    /// Stop recording
    pub fn stop(&self) -> Result<()> {
        debug!("Stopping Windows recorder...");
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
fn get_ui_element_at_point(automation: &UIAutomation, x: i32, y: i32) -> Option<UiElement> {
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
                get_window_info_for_process(pid)
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
fn get_window_info_for_process(_process_id: u32) -> (Option<String>, Option<String>) {
    // TODO: Implement window info gathering using UI Automation
    (None, None)
} 



