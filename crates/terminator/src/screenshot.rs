use crate::Monitor;
use base64::{engine::general_purpose, Engine};
use image::imageops::FilterType;
use image::{ImageBuffer, ImageFormat, Rgba};
use std::io::{Cursor, Seek, Write};

/// Default maximum dimension for screenshot resizing (width or height)
pub const DEFAULT_MAX_DIMENSION: u32 = 1920;

/// Holds the screenshot data
#[derive(Debug, Clone)]
pub struct ScreenshotResult {
    /// Raw image data in BGRA format (Windows) or RGBA format
    pub image_data: Vec<u8>,
    /// Width of the image
    pub width: u32,
    /// Height of the image
    pub height: u32,
    /// Monitor information if captured from a specific monitor
    pub monitor: Option<Monitor>,
}

impl ScreenshotResult {
    /// Encodes the screenshot data to the specified format and writes it to the provided writer.
    ///
    /// # Arguments
    ///
    /// * `writer` - A mutable reference to a writer (e.g., `Cursor<Vec<u8>>`).
    /// * `format` - The image format to encode to.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if encoding fails.
    pub fn write_to<W: Write + Seek>(
        &self,
        writer: &mut W,
        format: ImageFormat,
    ) -> Result<(), image::ImageError> {
        use image::ImageEncoder;
        match format {
            ImageFormat::Png => {
                let encoder = image::codecs::png::PngEncoder::new(writer);
                encoder.write_image(
                    &self.image_data,
                    self.width,
                    self.height,
                    image::ExtendedColorType::Rgba8,
                )
            }
            // Add other formats as needed, or use DynamicImage for more generic support
            _ => {
                // For other formats, we can construct a DynamicImage and save it
                // This involves copying data, so it might be slightly less efficient
                let img = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(
                    self.width,
                    self.height,
                    self.image_data.clone(),
                )
                .ok_or(image::ImageError::Parameter(
                    image::error::ParameterError::from_kind(
                        image::error::ParameterErrorKind::DimensionMismatch,
                    ),
                ))?;
                let dynamic_image = image::DynamicImage::ImageRgba8(img);
                dynamic_image.write_to(writer, format)
            }
        }
    }

    /// Convert BGRA image data to RGBA format.
    /// Windows captures are typically in BGRA format, this converts to RGBA for standard image processing.
    fn bgra_to_rgba(&self) -> Vec<u8> {
        self.image_data
            .chunks_exact(4)
            .flat_map(|bgra| [bgra[2], bgra[1], bgra[0], bgra[3]])
            .collect()
    }

    /// Encode the screenshot as PNG bytes.
    ///
    /// Converts BGRA to RGBA and encodes as PNG format.
    ///
    /// # Returns
    /// PNG-encoded bytes
    pub fn to_png(&self) -> Result<Vec<u8>, ScreenshotError> {
        let rgba_data = self.bgra_to_rgba();
        encode_rgba_to_png(&rgba_data, self.width, self.height)
    }

    /// Encode the screenshot as PNG bytes with optional resizing.
    ///
    /// If the image exceeds `max_dimension` in either width or height,
    /// it will be resized while maintaining aspect ratio.
    ///
    /// # Arguments
    /// * `max_dimension` - Maximum width or height. If None, uses DEFAULT_MAX_DIMENSION (1920).
    ///
    /// # Returns
    /// PNG-encoded bytes (potentially resized)
    pub fn to_png_resized(&self, max_dimension: Option<u32>) -> Result<Vec<u8>, ScreenshotError> {
        let max_dim = max_dimension.unwrap_or(DEFAULT_MAX_DIMENSION);
        let rgba_data = self.bgra_to_rgba();

        // Check if resize is needed
        if self.width <= max_dim && self.height <= max_dim {
            return encode_rgba_to_png(&rgba_data, self.width, self.height);
        }

        // Calculate new dimensions maintaining aspect ratio
        let scale = (max_dim as f32 / self.width.max(self.height) as f32).min(1.0);
        let new_width = (self.width as f32 * scale).round() as u32;
        let new_height = (self.height as f32 * scale).round() as u32;

        // Create image buffer and resize
        let img = ImageBuffer::<Rgba<u8>, _>::from_raw(self.width, self.height, rgba_data)
            .ok_or_else(|| ScreenshotError::ImageProcessing("Failed to create image buffer".into()))?;

        let resized = image::imageops::resize(&img, new_width, new_height, FilterType::Lanczos3);
        encode_rgba_to_png(&resized.into_raw(), new_width, new_height)
    }

    /// Encode the screenshot as base64-encoded PNG string.
    ///
    /// Useful for embedding in JSON responses or passing to LLMs.
    ///
    /// # Returns
    /// Base64-encoded PNG string
    pub fn to_base64_png(&self) -> Result<String, ScreenshotError> {
        let png_data = self.to_png()?;
        Ok(general_purpose::STANDARD.encode(&png_data))
    }

    /// Encode the screenshot as base64-encoded PNG string with optional resizing.
    ///
    /// If the image exceeds `max_dimension` in either width or height,
    /// it will be resized while maintaining aspect ratio.
    ///
    /// # Arguments
    /// * `max_dimension` - Maximum width or height. If None, uses DEFAULT_MAX_DIMENSION (1920).
    ///
    /// # Returns
    /// Base64-encoded PNG string (potentially resized)
    pub fn to_base64_png_resized(&self, max_dimension: Option<u32>) -> Result<String, ScreenshotError> {
        let png_data = self.to_png_resized(max_dimension)?;
        Ok(general_purpose::STANDARD.encode(&png_data))
    }

    /// Get the dimensions after potential resize operation.
    ///
    /// # Arguments
    /// * `max_dimension` - Maximum width or height.
    ///
    /// # Returns
    /// (width, height) after resize would be applied
    pub fn resized_dimensions(&self, max_dimension: u32) -> (u32, u32) {
        if self.width <= max_dimension && self.height <= max_dimension {
            return (self.width, self.height);
        }
        let scale = (max_dimension as f32 / self.width.max(self.height) as f32).min(1.0);
        let new_width = (self.width as f32 * scale).round() as u32;
        let new_height = (self.height as f32 * scale).round() as u32;
        (new_width, new_height)
    }
}

/// Error type for screenshot operations
#[derive(Debug, thiserror::Error)]
pub enum ScreenshotError {
    #[error("Image processing error: {0}")]
    ImageProcessing(String),
    #[error("PNG encoding error: {0}")]
    PngEncoding(String),
}

/// Helper function to encode RGBA data to PNG bytes
fn encode_rgba_to_png(rgba_data: &[u8], width: u32, height: u32) -> Result<Vec<u8>, ScreenshotError> {
    use image::codecs::png::PngEncoder;
    use image::{ExtendedColorType, ImageEncoder};

    let mut png_data = Vec::new();
    let encoder = PngEncoder::new(Cursor::new(&mut png_data));
    encoder
        .write_image(rgba_data, width, height, ExtendedColorType::Rgba8)
        .map_err(|e| ScreenshotError::PngEncoding(e.to_string()))?;

    Ok(png_data)
}
