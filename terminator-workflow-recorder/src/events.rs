use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Represents a position on the screen
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Represents the type of mouse event
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MouseEventType {
    Click,
    DoubleClick,
    RightClick,
    Down,
    Up,
    Move,
    Wheel,
    DragStart,
    DragEnd,
    Drop,
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
    
    /// Character representation of the key (if printable)
    pub character: Option<char>,
    
    /// Raw scan code
    pub scan_code: Option<u32>,
    
    /// Event metadata (UI element, application, etc.)
    pub metadata: EventMetadata,
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
    
    /// Scroll delta for wheel events
    pub scroll_delta: Option<(i32, i32)>,
    
    /// Drag start position (for drag events)
    pub drag_start: Option<Position>,
    
    /// Event metadata (UI element, application, etc.)
    pub metadata: EventMetadata,
}

/// Represents clipboard actions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ClipboardAction {
    Copy,
    Cut,
    Paste,
    Clear,
}

/// Represents a clipboard event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardEvent {
    /// The clipboard action
    pub action: ClipboardAction,
    
    /// The content that was copied/cut/pasted (truncated if too long)
    pub content: Option<String>,
    
    /// The size of the content in bytes
    pub content_size: Option<usize>,
    
    /// The format of the clipboard data
    pub format: Option<String>,
    
    /// Whether the content was truncated due to size
    pub truncated: bool,
    
    /// Event metadata (UI element, application, etc.)
    pub metadata: EventMetadata,
}

/// Represents text selection events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextSelectionEvent {
    /// The selected text content
    pub selected_text: String,
    
    /// The start position of the selection (screen coordinates)
    pub start_position: Position,
    
    /// The end position of the selection (screen coordinates)
    pub end_position: Position,
    
    /// The selection method (mouse drag, keyboard shortcuts, etc.)
    pub selection_method: SelectionMethod,
    
    /// The length of the selection in characters
    pub selection_length: usize,
    
    /// Whether this is a partial selection within a larger text block
    pub is_partial_selection: bool,
    
    /// Event metadata (UI element, application, etc.)
    pub metadata: EventMetadata,
}

/// Represents how text was selected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SelectionMethod {
    MouseDrag,
    DoubleClick,    // Word selection
    TripleClick,    // Line/paragraph selection
    KeyboardShortcut, // Ctrl+A, Shift+arrows, etc.
    ContextMenu,
}

/// Represents drag and drop operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DragDropEvent {
    /// The start position of the drag
    pub start_position: Position,
    
    /// The end position of the drop
    pub end_position: Position,
    
    /// The UI element being dragged (source)
    pub source_element: Option<UiElement>,
    
    /// The type of data being dragged
    pub data_type: Option<String>,
    
    /// The dragged content (if text)
    pub content: Option<String>,
    
    /// Whether the drag was successful
    pub success: bool,
    
    /// Event metadata (target UI element, application, etc.)
    pub metadata: EventMetadata,
}

/// Represents hotkey/shortcut events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyEvent {
    /// The key combination (e.g., "Ctrl+C", "Alt+Tab")
    pub combination: String,
    
    /// The action performed by the hotkey
    pub action: Option<String>,
    
    /// Whether this was a global or application-specific hotkey
    pub is_global: bool,
    
    /// Event metadata (UI element, application, etc.)
    pub metadata: EventMetadata,
}

/// Represents a workflow event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowEvent {
    /// A mouse event
    Mouse(MouseEvent),
    
    /// A keyboard event
    Keyboard(KeyboardEvent),
    
    /// A clipboard event
    Clipboard(ClipboardEvent),
    
    /// A text selection event
    TextSelection(TextSelectionEvent),
    
    /// A drag and drop event
    DragDrop(DragDropEvent),
    
    /// A hotkey event
    Hotkey(HotkeyEvent),
    
    /// A UI Automation property change event
    UiPropertyChanged(UiPropertyChangedEvent),
    
    /// A UI Automation focus change event
    UiFocusChanged(UiFocusChangedEvent),
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

/// Represents UI Automation structure change types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StructureChangeType {
    ChildAdded,
    ChildRemoved,
    ChildrenInvalidated,
    ChildrenBulkAdded,
    ChildrenBulkRemoved,
    ChildrenReordered,
}

/// Represents a UI Automation structure change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiStructureChangedEvent {
    /// The type of structure change
    pub change_type: StructureChangeType,
    
    /// The element where the structure change occurred
    pub element: Option<UiElement>,
    
    /// Runtime IDs of affected children (if applicable)
    pub runtime_ids: Option<Vec<i32>>,
    
    /// The application where the change occurred
    pub application: Option<String>,
    
    /// Additional details about the change
    pub details: Option<String>,
}

/// Represents a UI Automation property change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiPropertyChangedEvent {
    /// The property that changed (as string for serialization)
    pub property_name: String,
    
    /// The property ID
    pub property_id: u32,
    
    /// The old value (if available)
    pub old_value: Option<String>,
    
    /// The new value
    pub new_value: Option<String>,
    
    /// Event metadata (UI element, application, etc.)
    pub metadata: EventMetadata,
}

/// Represents a UI Automation focus change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiFocusChangedEvent {
    /// The previous element that had focus (if available)
    pub previous_element: Option<UiElement>,
    
    /// Event metadata (current focused UI element, application, etc.)
    pub metadata: EventMetadata,
}

/// Unified metadata for all workflow events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    /// The UI element associated with this event (if available)
    pub ui_element: Option<UiElement>,
    
    /// The application where this event occurred
    pub application: Option<String>,
    
    /// The window title where this event occurred
    pub window_title: Option<String>,
    
    /// The process ID of the application
    pub process_id: Option<u32>,
}

impl EventMetadata {
    /// Create new metadata from a UI element
    pub fn from_ui_element(ui_element: Option<UiElement>) -> Self {
        let (application, window_title, process_id) = if let Some(ref elem) = ui_element {
            (
                elem.application_name.clone(),
                elem.window_title.clone(),
                elem.process_id,
            )
        } else {
            (None, None, None)
        };

        Self {
            ui_element,
            application,
            window_title,
            process_id,
        }
    }

    /// Create empty metadata
    pub fn empty() -> Self {
        Self {
            ui_element: None,
            application: None,
            window_title: None,
            process_id: None,
        }
    }
} 