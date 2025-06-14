//! Workflow Recorder crate for Windows
//!
//! This crate provides functionality to record user interactions with the Windows UI,
//! including mouse clicks, keyboard input, and window focus changes.
//! The recorded workflow can be saved as a JSON file for later playback or analysis.

#![cfg_attr(not(target_os = "windows"), allow(unused))]

pub mod error;
pub mod events;
pub mod recorder;

pub use error::*;
pub use events::{
    ApplicationSwitchEvent, ApplicationSwitchMethod, BrowserTabNavigationEvent, ButtonClickEvent,
    ButtonInteractionType, ClipboardAction, ClipboardEvent, DragDropEvent, DropdownEvent,
    EventMetadata, FormSubmitEvent, HotkeyEvent, KeyboardEvent, LinkClickEvent, MouseButton,
    MouseEvent, MouseEventType, Position, RecordedEvent, RecordedWorkflow, Rect, SelectionMethod,
    TabAction, TabNavigationMethod, TextInputCompletedEvent, TextInputMethod, TextSelectionEvent,
    WorkflowEvent,
};
pub use recorder::*;
