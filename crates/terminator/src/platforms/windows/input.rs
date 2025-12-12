//! Input operations (mouse, keyboard, scroll) for Windows
//!
//! This module provides low-level input functions that are shared across
//! engine.rs and element.rs to avoid code duplication.

use crate::{AutomationError, ClickType};
use std::thread;
use std::time::Duration;
use tracing::debug;
use windows::core::BOOL;
use windows::Win32::Foundation::POINT;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern2,
    IUIAutomationTextRange, UIA_TextPattern2Id,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEINPUT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetSystemMetrics, SetCursorPos, SM_CXSCREEN, SM_CYSCREEN,
};

/// Send a mouse click at absolute screen coordinates.
///
/// This is the single source of truth for all mouse click operations.
/// Both engine.rs (Desktop.click_at_coordinates) and element.rs (UIElement.click)
/// should use this function.
///
/// # Arguments
/// * `x` - Absolute screen X coordinate
/// * `y` - Absolute screen Y coordinate
/// * `click_type` - Type of click: Left, Double, or Right
/// * `restore_cursor` - If true, cursor returns to original position after click
pub fn send_mouse_click(
    x: f64,
    y: f64,
    click_type: ClickType,
    restore_cursor: bool,
) -> Result<(), AutomationError> {
    // Save original cursor position if restore is requested
    let original_pos = if restore_cursor {
        let mut pos = POINT { x: 0, y: 0 };
        unsafe {
            let _ = GetCursorPos(&mut pos);
        }
        Some(pos)
    } else {
        None
    };

    unsafe {
        let screen_width = GetSystemMetrics(SM_CXSCREEN) as f64;
        let screen_height = GetSystemMetrics(SM_CYSCREEN) as f64;

        // Convert to normalized coordinates (0-65535 range)
        let abs_x = ((x * 65535.0) / screen_width) as i32;
        let abs_y = ((y * 65535.0) / screen_height) as i32;

        // Determine button flags based on click type
        let (down_flag, up_flag) = match click_type {
            ClickType::Left | ClickType::Double => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
            ClickType::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
        };

        // Move to position first
        let move_input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: abs_x,
                    dy: abs_y,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_MOVE,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        // Mouse down
        let down_input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: abs_x,
                    dy: abs_y,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_ABSOLUTE | down_flag,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        // Mouse up
        let up_input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: abs_x,
                    dy: abs_y,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_ABSOLUTE | up_flag,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        // Send inputs
        SendInput(&[move_input], std::mem::size_of::<INPUT>() as i32);
        SendInput(&[down_input], std::mem::size_of::<INPUT>() as i32);
        SendInput(&[up_input], std::mem::size_of::<INPUT>() as i32);

        // For double-click, send another click sequence
        if click_type == ClickType::Double {
            std::thread::sleep(std::time::Duration::from_millis(50));
            SendInput(&[down_input], std::mem::size_of::<INPUT>() as i32);
            SendInput(&[up_input], std::mem::size_of::<INPUT>() as i32);
        }
    }

    // Restore cursor position if requested
    if let Some(pos) = original_pos {
        unsafe {
            let _ = SetCursorPos(pos.x, pos.y);
        }
    }

    Ok(())
}

/// Send a simple left click at absolute screen coordinates.
/// Convenience wrapper for send_mouse_click with Left click type.
#[inline]
pub fn send_left_click(x: f64, y: f64, restore_cursor: bool) -> Result<(), AutomationError> {
    send_mouse_click(x, y, ClickType::Left, restore_cursor)
}

/// Saved focus state for restoration after automation operations.
///
/// Contains the previously focused element, optional caret position (for text fields),
/// and mouse cursor position. Used to restore user's context after typing/pressing keys.
pub struct FocusState {
    #[allow(dead_code)]
    automation: IUIAutomation,
    focused_element: IUIAutomationElement,
    caret_range: Option<IUIAutomationTextRange>,
    mouse_pos: POINT,
}

/// Save the current focus state including focused element, caret position, and mouse cursor.
///
/// Returns None if focus state cannot be saved (e.g., no focused element).
/// Caret position is only saved if the focused element supports TextPattern2.
pub fn save_focus_state() -> Option<FocusState> {
    unsafe {
        // Create UI Automation instance
        let automation: IUIAutomation =
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;

        // Save mouse position
        let mut mouse_pos = POINT { x: 0, y: 0 };
        let _ = GetCursorPos(&mut mouse_pos);

        // Get focused element
        let focused_element = automation.GetFocusedElement().ok()?;

        // Try to get caret position if element supports TextPattern2
        let caret_range = if let Ok(pattern) =
            focused_element.GetCurrentPatternAs::<IUIAutomationTextPattern2>(UIA_TextPattern2Id)
        {
            let mut is_active = BOOL::default();
            if let Ok(range) = pattern.GetCaretRange(&mut is_active) {
                range.Clone().ok()
            } else {
                None
            }
        } else {
            None
        };

        debug!(
            "Saved focus state: mouse=({}, {}), has_caret={}",
            mouse_pos.x,
            mouse_pos.y,
            caret_range.is_some()
        );

        Some(FocusState {
            automation,
            focused_element,
            caret_range,
            mouse_pos,
        })
    }
}

/// Restore a previously saved focus state.
///
/// Restores focus to the saved element, caret position (if available), and mouse cursor.
/// Silently fails if restoration is not possible (element no longer valid, etc.).
pub fn restore_focus_state(state: FocusState) {
    unsafe {
        // Restore focus to the element
        let _ = state.focused_element.SetFocus();

        // Restore caret position if we have it
        if let Some(ref range) = state.caret_range {
            // Small delay to let focus settle before selecting
            thread::sleep(Duration::from_millis(50));
            let _ = range.Select();
        }

        // Restore mouse cursor position
        let _ = SetCursorPos(state.mouse_pos.x, state.mouse_pos.y);

        debug!(
            "Restored focus state: mouse=({}, {}), had_caret={}",
            state.mouse_pos.x,
            state.mouse_pos.y,
            state.caret_range.is_some()
        );
    }
}
