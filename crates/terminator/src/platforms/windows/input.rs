//! Input operations (mouse, keyboard, scroll) for Windows
//!
//! This module provides low-level input functions that are shared across
//! engine.rs and element.rs to avoid code duplication.

use crate::{AutomationError, ClickType};
use windows::Win32::Foundation::POINT;
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
