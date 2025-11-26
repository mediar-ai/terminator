use crate::Monitor;
use image::ImageFormat;
use std::io::{Cursor, Seek, Write};

/// Holds the screenshot data
#[derive(Debug, Clone)]
pub struct ScreenshotResult {
    /// Raw image data (e.g., RGBA)
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
}
