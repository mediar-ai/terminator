//! Element highlighting functionality for Windows

use super::types::{FontStyle, HighlightHandle, TextPosition};
use crate::AutomationError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tracing::{debug, error};

use uiautomation::UIElement;
// Windows GDI imports
use windows::Win32::Foundation::{COLORREF, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    CreateFontW, CreatePen, DeleteObject, DrawTextW, GetDC, GetStockObject, Rectangle, ReleaseDC,
    SelectObject, SetBkMode, SetTextColor, DT_SINGLELINE, HGDIOBJ, NULL_BRUSH, PS_SOLID,
    TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

/// Implementation of element highlighting for Windows UI elements
pub fn highlight(
    element: Arc<UIElement>,
    color: Option<u32>,
    duration: Option<Duration>,
    text: Option<&str>,
    text_position: Option<TextPosition>,
    font_style: Option<FontStyle>,
) -> Result<HighlightHandle, AutomationError> {
    // Get the element's bounding rectangle (without focusing to avoid changing cursor position)
    let rect = element.get_bounding_rectangle().map_err(|e| {
        AutomationError::PlatformError(format!("Failed to get element bounds: {e}"))
    })?;

    // Debug: Log the rectangle bounds
    debug!(
        "Highlighting element at bounds: left={}, top={}, width={}, height={}",
        rect.get_left(),
        rect.get_top(),
        rect.get_width(),
        rect.get_height()
    );

    // Try to get scale factor from focused window first, fall back to cursor position
    let scale_factor =
        get_scale_factor_from_focused_window().unwrap_or_else(get_scale_factor_from_cursor);

    // Constants for border appearance
    const BORDER_SIZE: i32 = 4;
    const DEFAULT_RED_COLOR: u32 = 0x0000FF; // Pure red in BGR format

    // Use provided color or default to red
    let highlight_color = color.unwrap_or(DEFAULT_RED_COLOR);

    // Scale the coordinates and dimensions
    let x = (rect.get_left() as f64 * scale_factor) as i32;
    let y = (rect.get_top() as f64 * scale_factor) as i32;
    let width = (rect.get_width() as f64 * scale_factor) as i32;
    let height = (rect.get_height() as f64 * scale_factor) as i32;

    // Validate coordinates
    if width <= 0 || height <= 0 {
        return Err(AutomationError::PlatformError(format!(
            "Invalid element dimensions: width={width}, height={height}"
        )));
    }

    debug!(
        "Scaled highlight coordinates: x={}, y={}, width={}, height={}, scale_factor={}",
        x, y, width, height, scale_factor
    );

    // Prepare text overlay data (no truncation for better readability)
    let text_data = text.map(|t| {
        let display_text = if t.len() > 30 {
            format!("{}...", &t[..27]) // Allow longer text (30 chars max)
        } else {
            t.to_string()
        };
        let font_style = font_style.unwrap_or_default();
        let position = text_position.unwrap_or(TextPosition::Top);
        (display_text, font_style, position)
    });

    // Create atomic bool for controlling the highlight thread
    let should_close = Arc::new(AtomicBool::new(false));
    let should_close_clone = should_close.clone();

    // Spawn a thread to handle the highlighting
    let handle = thread::spawn(move || {
        let start_time = Instant::now();
        let duration = duration.unwrap_or(Duration::from_millis(3000)); // Default 3 seconds

        debug!("Starting highlight thread for duration: {:?}", duration);

        // Draw text overlay ONCE at the beginning if provided
        if let Some((ref text, ref font_style, position)) = text_data {
            draw_text_overlay(text, font_style, position, x, y, width, height);
        }

        // Main highlighting loop - just draw border
        debug!("Starting main highlight loop");
        let mut loop_count = 0;
        while start_time.elapsed() < duration && !should_close_clone.load(Ordering::Relaxed) {
            let hdc = unsafe { GetDC(None) };
            if hdc.0.is_null() {
                debug!("Failed to get device context for highlighting");
                return;
            }

            loop_count += 1;

            unsafe {
                // Create a pen for drawing with the specified color
                let hpen = CreatePen(PS_SOLID, BORDER_SIZE, COLORREF(highlight_color));
                if hpen.0.is_null() {
                    ReleaseDC(None, hdc);
                    return;
                }

                // Save current objects
                let old_pen = SelectObject(hdc, HGDIOBJ(hpen.0));
                let null_brush = GetStockObject(NULL_BRUSH);
                let old_brush = SelectObject(hdc, null_brush);

                // Draw only the border rectangle
                let _ = Rectangle(hdc, x, y, x + width, y + height);

                // Restore original objects and clean up
                SelectObject(hdc, old_brush);
                SelectObject(hdc, old_pen);
                let _ = DeleteObject(HGDIOBJ(hpen.0));
                ReleaseDC(None, hdc);
            }

            // Small delay to avoid excessive CPU usage
            thread::sleep(Duration::from_millis(16)); // ~60 FPS
        }

        debug!("Highlight loop completed after {} iterations", loop_count);
    });

    Ok(HighlightHandle {
        should_close,
        handle: Some(handle),
    })
}

/// Helper function to get scale factor from cursor position
fn get_scale_factor_from_cursor() -> f64 {
    let mut point = POINT { x: 0, y: 0 };
    unsafe {
        let _ = GetCursorPos(&mut point);
    }
    match xcap::Monitor::from_point(point.x, point.y) {
        Ok(monitor) => match monitor.scale_factor() {
            Ok(factor) => factor as f64,
            Err(e) => {
                error!("Failed to get scale factor from cursor position: {}", e);
                1.0 // Fallback to default scale factor
            }
        },
        Err(e) => {
            error!("Failed to get monitor from cursor position: {}", e);
            1.0 // Fallback to default scale factor
        }
    }
}

/// Helper function to get scale factor from focused window
fn get_scale_factor_from_focused_window() -> Option<f64> {
    match xcap::Window::all() {
        Ok(windows) => windows
            .iter()
            .find(|w| w.is_focused().unwrap_or(false))
            .and_then(|focused_window| focused_window.current_monitor().ok())
            .and_then(|monitor| monitor.scale_factor().ok().map(|factor| factor as f64)),
        Err(e) => {
            error!("Failed to get windows: {}", e);
            None
        }
    }
}

/// Helper function to draw text overlay
fn draw_text_overlay(
    text: &str,
    font_style: &FontStyle,
    position: TextPosition,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) {
    let hdc = unsafe { GetDC(None) };
    if !hdc.0.is_null() {
        unsafe {
            // Calculate text position with better spacing and sizing for visibility
            let (text_x, text_y, text_width, text_height) = match position {
                TextPosition::Top => (x, y - 35, width.max(250), 40), // Increased width for longer text
                TextPosition::TopRight => (x + width + 15, y - 60, 200, 50),
                TextPosition::Right => (x + width + 15, y + height / 2 - 25, 200, 50),
                TextPosition::BottomRight => (x + width + 15, y + height + 15, 200, 50),
                TextPosition::Bottom => (x, y + height + 15, width.max(200), 50),
                TextPosition::BottomLeft => (x - 215, y + height + 15, 200, 50),
                TextPosition::Left => (x - 215, y + height / 2 - 25, 200, 50),
                TextPosition::TopLeft => (x - 215, y - 60, 200, 50),
                TextPosition::Inside => (x + 15, y + 15, width - 30, height - 30),
            };

            // Create a larger font for better visibility
            let font_size = if font_style.size > 0 {
                font_style.size as i32
            } else {
                18
            };
            let font = CreateFontW(
                font_size,                                               // Height
                0,                                                       // Width (0 = default)
                0,                                                       // Escapement
                0,                                                       // Orientation
                700,                                                     // Weight (bold = 700)
                0,                                                       // Italic
                0,                                                       // Underline
                0,                                                       // StrikeOut
                windows::Win32::Graphics::Gdi::FONT_CHARSET(1), // CharSet (DEFAULT_CHARSET)
                windows::Win32::Graphics::Gdi::FONT_OUTPUT_PRECISION(0), // OutputPrecision
                windows::Win32::Graphics::Gdi::FONT_CLIP_PRECISION(0), // ClipPrecision
                windows::Win32::Graphics::Gdi::FONT_QUALITY(0), // Quality
                0,                                              // PitchAndFamily
                windows::core::PCWSTR::null(),                  // FaceName
            );

            let old_font = SelectObject(hdc, HGDIOBJ(font.0));

            // Set text drawing properties for transparent background
            let text_color = if font_style.color == 0 {
                0x00FF00 // Bright green for visibility
            } else {
                font_style.color
            };
            SetTextColor(hdc, COLORREF(text_color));
            SetBkMode(hdc, TRANSPARENT); // Transparent background

            debug!(
                "Drawing text '{}' at position ({}, {}) with size {}x{}",
                text, text_x, text_y, text_width, text_height
            );

            // Convert text to wide string and null-terminate
            let mut wide_text: Vec<u16> = text.encode_utf16().collect();
            wide_text.push(0);

            let mut text_rect = RECT {
                left: text_x + 5,
                top: text_y + 5,
                right: text_x + text_width - 5,
                bottom: text_y + text_height - 5,
            };

            // Draw text with top-left alignment
            let result = DrawTextW(
                hdc,
                &mut wide_text,
                &mut text_rect,
                DT_SINGLELINE, // Left-aligned, top-aligned text
            );

            debug!("DrawTextW result: {}", result);

            // Restore original font and cleanup
            SelectObject(hdc, old_font);
            let _ = DeleteObject(HGDIOBJ(font.0));

            // Clean up text drawing
            ReleaseDC(None, hdc);
        }
    }
}
