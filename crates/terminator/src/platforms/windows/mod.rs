//! Windows platform implementation for UI automation
//!
//! This module provides Windows-specific UI automation functionality using
//! the Windows UI Automation API through the uiautomation crate.

pub mod applications;
pub mod element;
pub mod engine;
pub mod health;
pub mod highlighting;
pub mod input;
pub mod inspect_overlay;
pub mod tree_builder;
pub mod types;
pub mod utils;
pub mod virtual_display;

// Re-export the main types that external code needs
pub use element::WindowsUIElement;
pub use engine::WindowsEngine;
pub use types::{FontStyle, HighlightHandle, TextPosition};

// Re-export utility functions that might be needed externally
pub use utils::{convert_uiautomation_element_to_terminator, generate_element_id};

// Re-export from applications module
pub use applications::get_process_name_by_pid;

// Re-export highlighting control functions
pub use highlighting::set_recording_mode;

// Re-export inspect overlay functions
pub use inspect_overlay::{hide_inspect_overlay, show_inspect_overlay, InspectElement, InspectOverlayHandle, OverlayDisplayMode};

// Re-export virtual display support
pub use virtual_display::{
    is_headless_environment, HeadlessConfig, VirtualDisplayConfig, VirtualDisplayManager,
};
