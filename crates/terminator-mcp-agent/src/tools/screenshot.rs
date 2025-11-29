//! Screenshot capture and processing for vision models (OCR, Omniparser, Gemini Vision)
//!
//! This module extracts common logic for capturing window screenshots,
//! resizing them for vision models, and converting coordinates.

use base64::{engine::general_purpose, Engine as _};
use image::codecs::png::PngEncoder;
use image::imageops::FilterType;
use image::{ExtendedColorType, ImageBuffer, ImageEncoder, Rgba};
use std::io::Cursor;
use terminator::{Desktop, UIElement};
use tracing::info;

/// Maximum dimension for vision model screenshots
pub const MAX_SCREENSHOT_DIM: u32 = 1920;

/// Result of capturing and preparing a screenshot for vision processing
pub struct PreparedScreenshot {
    /// Base64-encoded PNG image data
    pub base64_image: String,
    /// Final width after any resizing
    pub width: u32,
    /// Final height after any resizing
    pub height: u32,
    /// Window X position (for coordinate conversion)
    pub window_x: f64,
    /// Window Y position (for coordinate conversion)
    pub window_y: f64,
    /// Scale factor applied (1.0 if no resize, <1.0 if resized down)
    pub scale_factor: f64,
}

impl PreparedScreenshot {
    /// Convert vision model coordinates (relative to resized image) to absolute screen coordinates
    /// Input: [x_min, y_min, x_max, y_max] from vision model
    /// Output: [x_min, y_min, x_max, y_max] in screen coordinates
    pub fn to_absolute_coords(&self, box_2d: [f64; 4]) -> [f64; 4] {
        let inv_scale = 1.0 / self.scale_factor;
        [
            (box_2d[0] * inv_scale) + self.window_x,
            (box_2d[1] * inv_scale) + self.window_y,
            (box_2d[2] * inv_scale) + self.window_x,
            (box_2d[3] * inv_scale) + self.window_y,
        ]
    }
}

/// Find a window element by process ID
pub fn find_window_for_pid(desktop: &Desktop, pid: u32) -> Result<UIElement, String> {
    let apps = desktop
        .applications()
        .map_err(|e| format!("Failed to get applications: {e}"))?;

    apps.into_iter()
        .find(|app| app.process_id().unwrap_or(0) == pid)
        .ok_or_else(|| format!("No window found for PID {pid}"))
}

/// Capture a window screenshot and prepare it for vision model processing
///
/// This function:
/// 1. Gets window bounds
/// 2. Captures the screenshot
/// 3. Converts BGRA to RGBA
/// 4. Resizes if larger than MAX_SCREENSHOT_DIM
/// 5. Encodes to PNG and base64
pub fn capture_and_prepare_screenshot(
    window_element: &UIElement,
    model_name: &str,
) -> Result<PreparedScreenshot, String> {
    // Get window bounds (absolute screen coordinates)
    let bounds = window_element
        .bounds()
        .map_err(|e| format!("Failed to get window bounds: {e}"))?;
    let (window_x, window_y, win_w, win_h) = bounds;

    // Capture screenshot of the window
    let screenshot = window_element
        .capture()
        .map_err(|e| format!("Failed to capture window screenshot: {e}"))?;

    let original_width = screenshot.width;
    let original_height = screenshot.height;

    // DPI DEBUG logging
    let dpi_scale_w = original_width as f64 / win_w;
    let dpi_scale_h = original_height as f64 / win_h;
    info!(
        "{} DPI DEBUG: window_bounds(logical)=({:.0},{:.0},{:.0},{:.0}), screenshot(physical)={}x{}, dpi_scale=({:.3},{:.3})",
        model_name, window_x, window_y, win_w, win_h, original_width, original_height, dpi_scale_w, dpi_scale_h
    );

    // Convert BGRA to RGBA (xcap returns BGRA format)
    let rgba_data: Vec<u8> = screenshot
        .image_data
        .chunks_exact(4)
        .flat_map(|bgra| [bgra[2], bgra[1], bgra[0], bgra[3]])
        .collect();

    // Apply resize if needed
    let (final_width, final_height, final_rgba_data, scale_factor) =
        if original_width > MAX_SCREENSHOT_DIM || original_height > MAX_SCREENSHOT_DIM {
            let scale =
                (MAX_SCREENSHOT_DIM as f32 / original_width.max(original_height) as f32).min(1.0);
            let new_width = (original_width as f32 * scale).round() as u32;
            let new_height = (original_height as f32 * scale).round() as u32;

            let img =
                ImageBuffer::<Rgba<u8>, _>::from_raw(original_width, original_height, rgba_data)
                    .ok_or_else(|| {
                        "Failed to create image buffer from screenshot data".to_string()
                    })?;

            let resized = image::imageops::resize(&img, new_width, new_height, FilterType::Lanczos3);

            info!(
                "{}: Resized screenshot from {}x{} to {}x{} (scale: {:.2})",
                model_name, original_width, original_height, new_width, new_height, scale
            );

            (new_width, new_height, resized.into_raw(), scale as f64)
        } else {
            (original_width, original_height, rgba_data, 1.0)
        };

    // Encode to PNG
    let mut png_data = Vec::new();
    let encoder = PngEncoder::new(Cursor::new(&mut png_data));
    encoder
        .write_image(
            &final_rgba_data,
            final_width,
            final_height,
            ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("Failed to encode screenshot to PNG: {e}"))?;

    let base64_image = general_purpose::STANDARD.encode(&png_data);

    info!(
        "{}: Sending {}x{} image ({} KB)",
        model_name,
        final_width,
        final_height,
        png_data.len() / 1024
    );

    Ok(PreparedScreenshot {
        base64_image,
        width: final_width,
        height: final_height,
        window_x,
        window_y,
        scale_factor,
    })
}
