use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Represents a position on the screen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

/// Represents a UI element
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiElement {
    /// The name of the UI element
    pub name: Option<String>,
    
    /// The automation ID of the UI element
    pub automation_id: Option<String>,
    
    /// The class name of the UI element
    pub class_name: Option<String>,
    
    /// The control type of the UI element
    pub control_type: Option<String>,
    
    /// The process ID of the application that owns the UI element
    pub process_id: Option<u32>,
    
    /// The application name
    pub application_name: Option<String>,
    
    /// The window title
    pub window_title: Option<String>,
    
    /// The bounding rectangle of the UI element
    pub bounding_rect: Option<Rect>,
    
    /// Whether the UI element is enabled
    pub is_enabled: Option<bool>,
    
    /// Whether the UI element has keyboard focus
    pub has_keyboard_focus: Option<bool>,
    
    /// The hierarchy path to this element (from root)
    pub hierarchy_path: Option<String>,
    
    /// The value of the UI element (for input fields, etc.)
    pub value: Option<String>,
}

/// Represents a rectangle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Represents the type of mouse button
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Represents the type of mouse event
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MouseEventType {
    Click,
    DoubleClick,
    RightClick,
    Down,
    Up,
    Move,
    Wheel,
}

/// Represents a keyboard event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardEvent {
    /// The key code
    pub key_code: u32,
    
    /// Whether the key was pressed or released
    pub is_key_down: bool,
    
    /// Whether the Ctrl key was pressed
    pub ctrl_pressed: bool,
    
    /// Whether the Alt key was pressed
    pub alt_pressed: bool,
    
    /// Whether the Shift key was pressed
    pub shift_pressed: bool,
    
    /// Whether the Win key was pressed
    pub win_pressed: bool,
}

/// Represents a mouse event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseEvent {
    /// The type of mouse event
    pub event_type: MouseEventType,
    
    /// The mouse button
    pub button: MouseButton,
    
    /// The position of the mouse
    pub position: Position,
    
    /// The UI element under the mouse
    pub ui_element: Option<UiElement>,
}

/// Represents a window event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowEvent {
    /// The window title
    pub title: Option<String>,
    
    /// The window class name
    pub class_name: Option<String>,
    
    /// The process ID of the application that owns the window
    pub process_id: Option<u32>,
    
    /// The application name
    pub application_name: Option<String>,
}

/// Represents a workflow event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowEvent {
    /// A mouse event
    Mouse(MouseEvent),
    
    /// A keyboard event
    Keyboard(KeyboardEvent),
    
    /// A window focus changed event
    WindowFocusChanged(WindowEvent),
    
    /// A window created event
    WindowCreated(WindowEvent),
    
    /// A window closed event
    WindowClosed(WindowEvent),
}

/// Represents a recorded event with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedEvent {
    /// The timestamp of the event (milliseconds since epoch)
    pub timestamp: u64,
    
    /// The event
    pub event: WorkflowEvent,
}

/// Represents a recorded workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedWorkflow {
    /// The name of the workflow
    pub name: String,
    
    /// The timestamp when the recording started
    pub start_time: u64,
    
    /// The timestamp when the recording ended
    pub end_time: Option<u64>,
    
    /// The recorded events
    pub events: Vec<RecordedEvent>,
}

impl RecordedWorkflow {
    /// Create a new recorded workflow
    pub fn new(name: String) -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        
        Self {
            name,
            start_time: now,
            end_time: None,
            events: Vec::new(),
        }
    }
    
    /// Add an event to the workflow
    pub fn add_event(&mut self, event: WorkflowEvent) {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        
        self.events.push(RecordedEvent {
            timestamp: now,
            event,
        });
    }
    
    /// Finish the recording
    pub fn finish(&mut self) {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        
        self.end_time = Some(now);
    }
} 