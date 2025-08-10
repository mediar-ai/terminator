//! Common types used across platforms for UI automation

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

/// Position options for text overlays in highlighting
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TextPosition {
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
    TopLeft,
    Inside,
}

/// Font styling options for text overlays
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontStyle {
    pub size: u32,
    pub bold: bool,
    pub color: u32, // BGR format
}

impl Default for FontStyle {
    fn default() -> Self {
        Self {
            size: 12,
            bold: false,
            color: 0x000000, // Black
        }
    }
}

/// Handle for managing active highlights with cleanup
pub struct HighlightHandle {
    pub(crate) should_close: Arc<AtomicBool>,
    pub(crate) handle: Option<thread::JoinHandle<()>>,
}

impl HighlightHandle {
    /// Manually close the highlight
    pub fn close(mut self) {
        self.should_close.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for HighlightHandle {
    fn drop(&mut self) {
        self.should_close.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
