use anyhow::Result;
use image::{DynamicImage, ImageBuffer, Rgba};
use std::sync::Arc;
use tokio::sync::Mutex;

use super::StreamerConfig;

/// Encoded frame data
pub struct EncodedFrame {
    pub data: Vec<u8>,
    pub timestamp: webrtc::util::Marshal::Unmarshal,
    pub is_keyframe: bool,
}

/// Video encoder that converts raw frames to VP8
pub struct VideoEncoder {
    config: StreamerConfig,
    // In a real implementation, this would use libvpx or similar
    // For now, we'll use a placeholder that converts to raw format
    frame_count: Arc<Mutex<u64>>,
}

impl VideoEncoder {
    pub fn new(config: StreamerConfig) -> Result<Self> {
        Ok(Self {
            config,
            frame_count: Arc::new(Mutex::new(0)),
        })
    }

    pub async fn encode_frame(&self, frame: DynamicImage) -> Result<EncodedFrame> {
        let mut count = self.frame_count.lock().await;
        *count += 1;
        let frame_num = *count;

        // Resize frame to target dimensions if needed
        let frame = if frame.width() != self.config.width || frame.height() != self.config.height {
            frame.resize_exact(
                self.config.width,
                self.config.height,
                image::imageops::FilterType::Lanczos3,
            )
        } else {
            frame
        };

        // Convert to RGBA8
        let rgba = frame.to_rgba8();

        // In a real implementation, this would encode to VP8
        // For now, we'll create a simple placeholder format
        // that demonstrates the structure
        let mut encoded_data = Vec::new();

        // Simple header: frame number (8 bytes) + dimensions (8 bytes)
        encoded_data.extend_from_slice(&frame_num.to_le_bytes());
        encoded_data.extend_from_slice(&self.config.width.to_le_bytes());
        encoded_data.extend_from_slice(&self.config.height.to_le_bytes());

        // For demonstration, we'll compress using a simple RLE-like approach
        // In production, this would use actual VP8 encoding
        let raw_data = rgba.as_raw();
        encoded_data.extend_from_slice(&compress_simple(raw_data));

        // Calculate timestamp based on frame rate
        let timestamp =
            webrtc::util::Marshal::Unmarshal((frame_num * 90000 / self.config.fps as u64) as i64);

        // Keyframe every 30 frames
        let is_keyframe = frame_num % 30 == 1;

        Ok(EncodedFrame {
            data: encoded_data,
            timestamp,
            is_keyframe,
        })
    }
}

/// Simple compression for demonstration
/// In production, use proper VP8 encoding
fn compress_simple(data: &[u8]) -> Vec<u8> {
    // This is a placeholder - real implementation would use libvpx
    // For now, just return a subset of the data to simulate compression
    let sample_rate = 4; // Take every 4th byte as a simple "compression"
    data.iter().step_by(sample_rate).copied().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_encoder_creation() {
        let config = StreamerConfig::default();
        let encoder = VideoEncoder::new(config).unwrap();
        assert!(encoder.frame_count.lock().await.eq(&0));
    }

    #[tokio::test]
    async fn test_frame_encoding() {
        let config = StreamerConfig {
            width: 640,
            height: 480,
            ..Default::default()
        };

        let encoder = VideoEncoder::new(config).unwrap();

        // Create a test image
        let img = DynamicImage::ImageRgba8(ImageBuffer::from_fn(640, 480, |x, y| {
            Rgba([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8, 255])
        }));

        let encoded = encoder.encode_frame(img).await.unwrap();
        assert!(!encoded.data.is_empty());
        assert!(encoded.is_keyframe); // First frame should be keyframe
    }
}
