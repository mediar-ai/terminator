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
    
    /// Scroll delta for wheel events
    pub scroll_delta: Option<(i32, i32)>,
    
    /// Drag start position (for drag events)
    pub drag_start: Option<Position>,
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
    
    /// The application that performed the action
    pub source_application: Option<String>,
    
    /// Whether the content was truncated due to size
    pub truncated: bool,
}

/// Represents window actions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WindowAction {
    Created,
    Closed,
    Minimized,
    Maximized,
    Restored,
    Moved,
    Resized,
    FocusGained,
    FocusLost,
    TitleChanged,
}

/// Represents a window event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowEvent {
    /// The window action
    pub action: WindowAction,
    
    /// The window title
    pub title: Option<String>,
    
    /// The window class name
    pub class_name: Option<String>,
    
    /// The process ID of the application that owns the window
    pub process_id: Option<u32>,
    
    /// The application name
    pub application_name: Option<String>,
    
    /// Window position and size
    pub bounds: Option<Rect>,
    
    /// Previous bounds (for move/resize events)
    pub previous_bounds: Option<Rect>,
    
    /// Window handle
    pub handle: Option<String>,
    
    /// Parent window handle
    pub parent_handle: Option<String>,
    
    /// Window state (normal, minimized, maximized)
    pub state: Option<String>,
}

/// Represents text input events (high-level text changes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextInputEvent {
    /// The text that was entered
    pub text: String,
    
    /// The UI element where text was entered
    pub target_element: Option<UiElement>,
    
    /// Whether this was a replacement of existing text
    pub is_replacement: bool,
    
    /// The previous text (if replacement)
    pub previous_text: Option<String>,
    
    /// Selection start position
    pub selection_start: Option<u32>,
    
    /// Selection end position
    pub selection_end: Option<u32>,
}

/// Represents application lifecycle events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApplicationAction {
    Launched,
    Terminated,
    InstallStarted,
    InstallCompleted,
    UpdateStarted,
    UpdateCompleted,
}

/// Represents an application event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationEvent {
    /// The application action
    pub action: ApplicationAction,
    
    /// The application name
    pub application_name: Option<String>,
    
    /// The application path
    pub application_path: Option<String>,
    
    /// The process ID
    pub process_id: Option<u32>,
    
    /// Command line arguments (for launched apps)
    pub command_line: Option<String>,
    
    /// Application version
    pub version: Option<String>,
}

/// Represents file system operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileAction {
    Created,
    Modified,
    Deleted,
    Moved,
    Copied,
    Opened,
    Closed,
    Downloaded,
    Uploaded,
}

/// Represents a file system event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEvent {
    /// The file action
    pub action: FileAction,
    
    /// The file path
    pub path: String,
    
    /// The destination path (for move/copy operations)
    pub destination_path: Option<String>,
    
    /// The file size
    pub size: Option<u64>,
    
    /// The file type/extension
    pub file_type: Option<String>,
    
    /// The application that performed the action
    pub source_application: Option<String>,
}

/// Represents menu and context menu interactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuEvent {
    /// The menu item that was selected
    pub menu_item: String,
    
    /// The menu path (e.g., "File -> Save As")
    pub menu_path: String,
    
    /// The application where the menu was accessed
    pub application: Option<String>,
    
    /// Whether this was a context menu
    pub is_context_menu: bool,
    
    /// The UI element that had focus when menu was opened
    pub context_element: Option<UiElement>,
}

/// Represents dialog interactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogEvent {
    /// The dialog title
    pub title: Option<String>,
    
    /// The dialog type (e.g., "Save", "Open", "Error", "Warning")
    pub dialog_type: Option<String>,
    
    /// The button that was clicked
    pub button_clicked: Option<String>,
    
    /// The dialog text/message
    pub message: Option<String>,
    
    /// Input values (for dialogs with input fields)
    pub input_values: Option<Vec<(String, String)>>,
    
    /// The application that showed the dialog
    pub application: Option<String>,
}

/// Represents scroll events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrollEvent {
    /// The scroll direction and amount
    pub delta: (i32, i32),
    
    /// The position where scrolling occurred
    pub position: Position,
    
    /// The UI element being scrolled
    pub target_element: Option<UiElement>,
    
    /// Whether this was horizontal or vertical scrolling
    pub direction: ScrollDirection,
}

/// Represents scroll direction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ScrollDirection {
    Vertical,
    Horizontal,
    Both,
}

/// Represents system-level events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemAction {
    ScreenLocked,
    ScreenUnlocked,
    UserLoggedIn,
    UserLoggedOut,
    SystemSleep,
    SystemWake,
    DisplayChanged,
    AudioVolumeChanged,
    NetworkConnected,
    NetworkDisconnected,
    UsbDeviceConnected,
    UsbDeviceDisconnected,
}

/// Represents a system event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemEvent {
    /// The system action
    pub action: SystemAction,
    
    /// Additional details about the event
    pub details: Option<String>,
    
    /// Relevant device information (for hardware events)
    pub device_info: Option<String>,
}

/// Represents drag and drop operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DragDropEvent {
    /// The start position of the drag
    pub start_position: Position,
    
    /// The end position of the drop
    pub end_position: Position,
    
    /// The UI element being dragged
    pub source_element: Option<UiElement>,
    
    /// The UI element where it was dropped
    pub target_element: Option<UiElement>,
    
    /// The type of data being dragged
    pub data_type: Option<String>,
    
    /// The dragged content (if text)
    pub content: Option<String>,
    
    /// Whether the drag was successful
    pub success: bool,
}

/// Represents hotkey/shortcut events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyEvent {
    /// The key combination (e.g., "Ctrl+C", "Alt+Tab")
    pub combination: String,
    
    /// The action performed by the hotkey
    pub action: Option<String>,
    
    /// The application where the hotkey was used
    pub application: Option<String>,
    
    /// Whether this was a global or application-specific hotkey
    pub is_global: bool,
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
    
    /// The UI element containing the selected text
    pub target_element: Option<UiElement>,
    
    /// The selection method (mouse drag, keyboard shortcuts, etc.)
    pub selection_method: SelectionMethod,
    
    /// The length of the selection in characters
    pub selection_length: usize,
    
    /// Whether this is a partial selection within a larger text block
    pub is_partial_selection: bool,
    
    /// The application where the selection occurred
    pub application: Option<String>,
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

/// Represents a workflow event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowEvent {
    /// A mouse event
    Mouse(MouseEvent),
    
    /// A keyboard event
    Keyboard(KeyboardEvent),
    
    /// A window event
    Window(WindowEvent),
    
    /// A clipboard event
    Clipboard(ClipboardEvent),
    
    /// A text input event
    TextInput(TextInputEvent),
    
    /// A text selection event
    TextSelection(TextSelectionEvent),
    
    /// An application event
    Application(ApplicationEvent),
    
    /// A file system event
    File(FileEvent),
    
    /// A menu interaction event
    Menu(MenuEvent),
    
    /// A dialog interaction event
    Dialog(DialogEvent),
    
    /// A scroll event
    Scroll(ScrollEvent),
    
    /// A system event
    System(SystemEvent),
    
    /// A drag and drop event
    DragDrop(DragDropEvent),
    
    /// A hotkey event
    Hotkey(HotkeyEvent),
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