//! Workflow Recorder crate for Windows
//!
//! This crate provides functionality to record user interactions with the Windows UI,
//! including mouse clicks, keyboard input, and window focus changes.
//! The recorded workflow can be saved as a JSON file for later playback or analysis.

#![cfg_attr(not(target_os = "windows"), allow(unused))]

pub mod events;
pub mod recorder;
pub mod error;

pub use events::{
    Position, Rect, MouseButton, MouseEventType, KeyboardEvent, MouseEvent,
    ClipboardAction, ClipboardEvent, TextSelectionEvent, SelectionMethod, DragDropEvent,
    HotkeyEvent, WorkflowEvent, RecordedEvent, RecordedWorkflow, StructureChangeType,
    UiStructureChangedEvent, UiPropertyChangedEvent, UiFocusChangedEvent, EventMetadata,
};
pub use recorder::*;
pub use error::*;

