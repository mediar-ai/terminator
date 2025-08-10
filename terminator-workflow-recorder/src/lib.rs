//! Workflow Recorder crate for Windows
//!
//! This crate provides functionality to record user interactions with the Windows UI,
//! including mouse clicks, keyboard input, and window focus changes.
//! The recorded workflow can be saved as a JSON file for later playback or analysis.

#![cfg_attr(not(target_os = "windows"), allow(unused))]

mod error;
mod events;
mod mcp_converter;
mod recorder;

pub use error::*;
pub use events::{
    ApplicationSwitchEvent, ApplicationSwitchMethod, BrowserTabNavigationEvent,
    ButtonInteractionType, ClickEvent, ClipboardAction, ClipboardEvent, DragDropEvent,
    EnhancedUIElement, EventMetadata, HotkeyEvent, InteractionContext, KeyboardEvent, McpToolStep,
    MouseButton, MouseEvent, MouseEventType, Position, RecordedEvent, RecordedWorkflow, Rect,
    SelectionMethod, TabAction, TabNavigationMethod, TextInputCompletedEvent, TextInputMethod,
    TextSelectionEvent, UIElementInfo, WorkflowEvent,
};
pub use mcp_converter::{ConversionConfig, ConversionResult, McpConverter};
pub use recorder::*;

#[cfg(target_os = "windows")]
pub mod structs {
    pub use crate::recorder::windows::structs::*;
}
