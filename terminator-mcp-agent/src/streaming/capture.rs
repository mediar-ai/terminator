use anyhow::Result;
use image::DynamicImage;
use std::sync::Arc;
use terminator::Desktop;

/// Screen capture handler that uses Terminator's built-in screenshot capabilities
pub struct ScreenCapture {
    desktop: Arc<Desktop>,
}

impl ScreenCapture {
    pub fn new() -> Result<Self> {
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        let desktop = Desktop::new(false, false)?;

        #[cfg(target_os = "macos")]
        let desktop = Desktop::new(true, true)?;

        Ok(Self {
            desktop: Arc::new(desktop),
        })
    }

    /// Capture a frame from the primary monitor
    pub async fn capture_frame(&self) -> Result<DynamicImage> {
        // Get primary monitor
        let monitor = self.desktop.get_primary_monitor().await?;

        // Capture screenshot
        let screenshot = self.desktop.capture_monitor(&monitor).await?;

        // Convert to DynamicImage
        let image = DynamicImage::ImageRgba8(
            image::ImageBuffer::from_raw(screenshot.width, screenshot.height, screenshot.data)
                .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?,
        );

        Ok(image)
    }

    /// Capture a specific region of the screen
    pub async fn capture_region(
        &self,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<DynamicImage> {
        // For now, capture full screen and crop
        // In the future, this could be optimized to capture only the specific region
        let full_frame = self.capture_frame().await?;

        // Ensure the region is within bounds
        let x = x.max(0) as u32;
        let y = y.max(0) as u32;
        let width = width.min(full_frame.width() - x);
        let height = height.min(full_frame.height() - y);

        Ok(full_frame.crop_imm(x, y, width, height))
    }

    /// Get the dimensions of the primary monitor
    pub async fn get_screen_dimensions(&self) -> Result<(u32, u32)> {
        let monitor = self.desktop.get_primary_monitor().await?;
        Ok((monitor.width, monitor.height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_screen_capture_creation() {
        let capture = ScreenCapture::new();
        assert!(capture.is_ok());
    }

    #[tokio::test]
    #[ignore] // Ignore in CI as it requires a display
    async fn test_capture_frame() {
        let capture = ScreenCapture::new().unwrap();
        let frame = capture.capture_frame().await;
        assert!(frame.is_ok());

        let frame = frame.unwrap();
        assert!(frame.width() > 0);
        assert!(frame.height() > 0);
    }

    #[tokio::test]
    #[ignore] // Ignore in CI as it requires a display
    async fn test_get_screen_dimensions() {
        let capture = ScreenCapture::new().unwrap();
        let dims = capture.get_screen_dimensions().await;
        assert!(dims.is_ok());

        let (width, height) = dims.unwrap();
        assert!(width > 0);
        assert!(height > 0);
    }
}
