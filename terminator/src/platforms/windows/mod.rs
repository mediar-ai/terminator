//! Windows platform implementation for UI automation
//!
//! This module provides Windows-specific UI automation functionality using
//! the Windows UI Automation API through the uiautomation crate.

pub mod applications;
pub mod element;
pub mod engine;
pub mod highlighting;
pub mod input;
pub mod tree_builder;
pub mod types;
pub mod utils;

// Re-export the main types that external code needs
pub use element::WindowsUIElement;
pub use engine::WindowsEngine;
pub use types::{FontStyle, HighlightHandle, TextPosition};

// Re-export utility functions that might be needed externally
pub use utils::{convert_uiautomation_element_to_terminator, generate_element_id};

// Re-export from applications module
pub use applications::get_process_name_by_pid;
