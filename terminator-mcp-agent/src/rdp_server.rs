#[cfg(feature = "rdp")]
use anyhow::{Context, Result};
use ironrdp::connector::ConnectorResult;
use ironrdp_acceptor::{accept_begin, accept_finalize, Acceptor, AcceptorResult, BeginResult};
use ironrdp_async::Framed;
use ironrdp_pdu::gcc::KeyboardType;
use ironrdp_pdu::input::fast_path::{FastPathInput, FastPathInputEvent};
use ironrdp_pdu::input::mouse::{PointerButton, PointerEvent, PointerFlags};
use ironrdp_pdu::input::scancode::KeyboardFlags;
use ironrdp_pdu::{decode, encode_vec, mcs, nego, PduParsing};
use ironrdp_server::{
    BitmapUpdate, DesktopSize, DisplayUpdate, Framebuffer, RdpServer, RdpServerDisplay,
    RdpServerInputHandler, RdpServerOptions,
};
use std::net::SocketAddr;
use std::sync::Arc;
use terminator::Desktop;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// RDP server configuration
#[derive(Debug, Clone)]
pub struct RdpServerConfig {
    /// TCP bind address (default: 127.0.0.1:3389)
    pub bind_address: SocketAddr,
    /// Screen refresh rate in FPS (default: 15)
    pub fps: u32,
    /// Enable input control (mouse/keyboard)
    pub enable_input_control: bool,
}

impl Default for RdpServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1:3389".parse().unwrap(),
            fps: 15,
            enable_input_control: true,
        }
    }
}

/// Input handler implementation that forwards events to Desktop API
struct DesktopInputHandler {
    desktop: Arc<Desktop>,
}

impl RdpServerInputHandler for DesktopInputHandler {
    fn keyboard(&mut self, code: u16, flags: KeyboardFlags) -> Result<()> {
        debug!("Keyboard event: scancode={}, flags={:?}", code, flags);

        // TODO: Map scancode to key name and call desktop.press_key()
        // For now, just log the event

        Ok(())
    }

    fn mouse(&mut self, event: PointerEvent) -> Result<()> {
        debug!(
            "Mouse event: x={}, y={}, flags={:?}",
            event.x_position, event.y_position, event.flags
        );

        // Convert RDP coordinates to screen coordinates
        let x = event.x_position as f64;
        let y = event.y_position as f64;

        // Handle different mouse actions
        if event.flags.contains(PointerFlags::DOWN) {
            if event.flags.contains(PointerFlags::BUTTON1) {
                debug!("Left mouse button down at ({}, {})", x, y);
                // TODO: Call desktop.click() or similar method
            } else if event.flags.contains(PointerFlags::BUTTON2) {
                debug!("Right mouse button down at ({}, {})", x, y);
            }
        }

        Ok(())
    }
}

/// Display handler implementation that captures screen and sends updates
struct DesktopDisplay {
    desktop: Arc<Desktop>,
    fps: u32,
}

impl RdpServerDisplay for DesktopDisplay {
    async fn size(&mut self) -> Result<DesktopSize> {
        // Capture screen to get dimensions
        match self.desktop.capture_all_monitors().await {
            Ok(screenshots) => {
                if let Some(screenshot) = screenshots.first() {
                    Ok(DesktopSize {
                        width: screenshot.width as u16,
                        height: screenshot.height as u16,
                    })
                } else {
                    Ok(DesktopSize {
                        width: 1920,
                        height: 1080,
                    })
                }
            }
            Err(e) => {
                warn!("Failed to capture screen for size: {}", e);
                Ok(DesktopSize {
                    width: 1920,
                    height: 1080,
                })
            }
        }
    }

    async fn updates(&mut self) -> Result<Vec<DisplayUpdate>> {
        // Capture all monitors
        match self.desktop.capture_all_monitors().await {
            Ok(screenshots) => {
                let mut updates = Vec::new();

                for (monitor_idx, screenshot) in screenshots.iter().enumerate() {
                    debug!(
                        "Captured monitor {} ({}x{})",
                        monitor_idx, screenshot.width, screenshot.height
                    );

                    // Convert RGBA to RGB (RDP uses RGB24)
                    let rgb_data: Vec<u8> = screenshot
                        .rgba_data
                        .chunks(4)
                        .flat_map(|rgba| vec![rgba[2], rgba[1], rgba[0]]) // BGR order for RDP
                        .collect();

                    // Create bitmap update
                    let framebuffer = Framebuffer {
                        width: screenshot.width as u16,
                        height: screenshot.height as u16,
                        data: rgb_data,
                    };

                    updates.push(DisplayUpdate::Bitmap(BitmapUpdate {
                        top: 0,
                        left: 0,
                        framebuffer,
                    }));
                }

                Ok(updates)
            }
            Err(e) => {
                warn!("Failed to capture screens: {}", e);
                Ok(Vec::new())
            }
        }
    }
}

/// RDP server for remote viewing and control of the MCP session
pub struct RdpServerRunner {
    config: RdpServerConfig,
    desktop: Arc<Desktop>,
}

impl RdpServerRunner {
    /// Create a new RDP server instance
    pub fn new(config: RdpServerConfig, desktop: Arc<Desktop>) -> Self {
        Self { config, desktop }
    }

    /// Start the RDP server and accept connections
    pub async fn run(self) -> Result<()> {
        let listener = TcpListener::bind(self.config.bind_address)
            .await
            .context("Failed to bind RDP server")?;

        info!(
            "RDP server listening on {} (fps: {}, input control: {})",
            self.config.bind_address, self.config.fps, self.config.enable_input_control
        );

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("RDP client connected from {}", addr);
                    let server = self.clone();
                    tokio::spawn(async move {
                        if let Err(e) = server.handle_client(stream, addr).await {
                            error!("RDP client {} error: {:#}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept RDP connection: {}", e);
                }
            }
        }
    }

    /// Handle a single RDP client connection
    async fn handle_client(&self, stream: TcpStream, addr: SocketAddr) -> Result<()> {
        info!("Handling RDP client from {}", addr);

        // Create input and display handlers
        let input_handler = DesktopInputHandler {
            desktop: Arc::clone(&self.desktop),
        };

        let display_handler = DesktopDisplay {
            desktop: Arc::clone(&self.desktop),
            fps: self.config.fps,
        };

        // Create RDP server options
        let options = RdpServerOptions::default();

        // Create and run RDP server for this connection
        let mut server = RdpServer::builder()
            .with_addr(self.config.bind_address)
            .with_input_handler(input_handler)
            .with_display_handler(display_handler)
            .build()?;

        // Run the connection
        server.run_connection(stream).await?;

        info!("RDP client {} disconnected", addr);
        Ok(())
    }
}

impl Clone for RdpServerRunner {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            desktop: Arc::clone(&self.desktop),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rdp_config_default() {
        let config = RdpServerConfig::default();
        assert_eq!(config.bind_address.to_string(), "127.0.0.1:3389");
        assert_eq!(config.fps, 15);
        assert!(config.enable_input_control);
    }
}
