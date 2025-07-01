use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_VP8};
use webrtc::api::setting_engine::SettingEngine;
use webrtc::api::API;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::media::Sample;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_local::TrackLocal;
use webrtc::util::Marshal;

pub mod capture;
pub mod encoder;
pub mod signaling;

use capture::ScreenCapture;
use encoder::VideoEncoder;

// Re-export the create_signaling_backend function
pub use signaling::create_signaling_backend;

/// Signaling messages that need to be exchanged between peers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SignalingMessage {
    /// SDP offer from the streaming agent
    Offer { sdp: String },
    /// SDP answer from the viewer
    Answer { sdp: String },
    /// ICE candidate from either party
    IceCandidate {
        candidate: String,
        sdp_mid: Option<String>,
        sdp_mline_index: Option<u16>,
    },
    /// Signal to start streaming
    StartStream,
    /// Signal to stop streaming
    StopStream,
}

/// Trait for implementing custom signaling backends
/// This allows users to plug in any signaling mechanism (WebSocket, Supabase, Firebase, etc.)
#[async_trait]
pub trait SignalingBackend: Send + Sync {
    /// Send a signaling message
    async fn send(&self, message: SignalingMessage) -> Result<()>;

    /// Receive the next signaling message
    async fn receive(&mut self) -> Result<SignalingMessage>;

    /// Get a unique session/channel ID for this connection
    fn session_id(&self) -> &str;
}

/// Configuration for the WebRTC streamer
#[derive(Debug, Clone)]
pub struct StreamerConfig {
    /// Video width in pixels
    pub width: u32,
    /// Video height in pixels  
    pub height: u32,
    /// Frames per second
    pub fps: u32,
    /// Video bitrate in bits per second
    pub bitrate: u32,
    /// STUN/TURN servers for NAT traversal
    pub ice_servers: Vec<RTCIceServer>,
    /// Whether to use hardware acceleration if available
    pub use_hardware_acceleration: bool,
}

impl Default for StreamerConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 15,
            bitrate: 2_000_000, // 2 Mbps
            ice_servers: vec![RTCIceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_string()],
                ..Default::default()
            }],
            use_hardware_acceleration: true,
        }
    }
}

/// Main WebRTC streamer that handles screen capture and streaming
pub struct WebRTCStreamer {
    config: StreamerConfig,
    peer_connection: Arc<RTCPeerConnection>,
    video_track: Arc<TrackLocalStaticRTP>,
    encoder: Arc<RwLock<VideoEncoder>>,
    capture: Arc<RwLock<ScreenCapture>>,
    signaling: Arc<RwLock<Box<dyn SignalingBackend>>>,
    is_streaming: Arc<RwLock<bool>>,
}

impl WebRTCStreamer {
    /// Create a new WebRTC streamer with the given configuration and signaling backend
    pub async fn new(config: StreamerConfig, signaling: Box<dyn SignalingBackend>) -> Result<Self> {
        // Create a MediaEngine with VP8 codec
        let mut media_engine = MediaEngine::default();
        media_engine.register_default_codecs()?;

        // Create the WebRTC API
        let mut setting_engine = SettingEngine::default();
        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut media_engine)?;

        let api = API::new_with_media_engine(media_engine, Some(setting_engine), Some(registry));

        // Create peer connection
        let rtc_config = RTCConfiguration {
            ice_servers: config.ice_servers.clone(),
            ..Default::default()
        };

        let peer_connection = Arc::new(api.new_peer_connection(rtc_config).await?);

        // Create video track
        let video_track = Arc::new(TrackLocalStaticRTP::new(
            RTCRtpCodecCapability {
                mime_type: MIME_TYPE_VP8.to_string(),
                ..Default::default()
            },
            "video".to_string(),
            "terminator-stream".to_string(),
        ));

        // Add track to peer connection
        let rtp_sender = peer_connection
            .add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>)
            .await?;

        // Handle RTCP packets (for congestion control, etc.)
        tokio::spawn(async move {
            let mut rtcp_buf = vec![0u8; 1500];
            while let Ok((_, _)) = rtp_sender.read(&mut rtcp_buf).await {}
        });

        // Create encoder and capture
        let encoder = Arc::new(RwLock::new(VideoEncoder::new(config.clone())?));
        let capture = Arc::new(RwLock::new(ScreenCapture::new()?));

        Ok(Self {
            config,
            peer_connection,
            video_track,
            encoder,
            capture,
            signaling: Arc::new(RwLock::new(signaling)),
            is_streaming: Arc::new(RwLock::new(false)),
        })
    }

    /// Start the WebRTC streaming session
    pub async fn start(&self) -> Result<()> {
        // Set up peer connection handlers
        self.setup_peer_connection_handlers().await?;

        // Create and send offer
        let offer = self.peer_connection.create_offer(None).await?;
        self.peer_connection
            .set_local_description(offer.clone())
            .await?;

        let signaling = self.signaling.read().await;
        signaling
            .send(SignalingMessage::Offer { sdp: offer.sdp })
            .await?;

        // Start handling signaling messages
        let streamer = self.clone_refs();
        tokio::spawn(async move {
            if let Err(e) = streamer.handle_signaling().await {
                tracing::error!("Signaling error: {}", e);
            }
        });

        Ok(())
    }

    /// Stop the streaming session
    pub async fn stop(&self) -> Result<()> {
        *self.is_streaming.write().await = false;

        let signaling = self.signaling.read().await;
        signaling.send(SignalingMessage::StopStream).await?;

        self.peer_connection.close().await?;
        Ok(())
    }

    /// Handle incoming signaling messages
    async fn handle_signaling(&self) -> Result<()> {
        let mut signaling = self.signaling.write().await;

        loop {
            match signaling.receive().await? {
                SignalingMessage::Answer { sdp } => {
                    use webrtc::peer_connection::sdp::sdp_type::RTCSdpType;
                    use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

                    let answer = RTCSessionDescription {
                        sdp_type: RTCSdpType::Answer,
                        sdp,
                    };
                    self.peer_connection.set_remote_description(answer).await?;
                }
                SignalingMessage::IceCandidate {
                    candidate,
                    sdp_mid,
                    sdp_mline_index,
                } => {
                    use webrtc::ice_transport::ice_candidate::{
                        RTCIceCandidate, RTCIceCandidateInit,
                    };

                    let candidate = RTCIceCandidateInit {
                        candidate,
                        sdp_mid,
                        sdp_mline_index,
                        username_fragment: None,
                    };
                    self.peer_connection.add_ice_candidate(candidate).await?;
                }
                SignalingMessage::StartStream => {
                    if !*self.is_streaming.read().await {
                        self.start_capture_loop().await?;
                    }
                }
                SignalingMessage::StopStream => {
                    *self.is_streaming.write().await = false;
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Set up peer connection event handlers
    async fn setup_peer_connection_handlers(&self) -> Result<()> {
        let signaling = Arc::clone(&self.signaling);

        // Handle ICE candidates
        self.peer_connection
            .on_ice_candidate(Box::new(move |candidate| {
                let signaling = Arc::clone(&signaling);
                Box::pin(async move {
                    if let Some(candidate) = candidate {
                        let signaling = signaling.read().await;
                        let _ = signaling
                            .send(SignalingMessage::IceCandidate {
                                candidate: candidate.candidate,
                                sdp_mid: candidate.sdp_mid,
                                sdp_mline_index: candidate.sdp_mline_index,
                            })
                            .await;
                    }
                })
            }));

        // Handle connection state changes
        self.peer_connection
            .on_peer_connection_state_change(Box::new(move |state| {
                Box::pin(async move {
                    tracing::info!("Peer connection state: {:?}", state);
                    if state == RTCPeerConnectionState::Connected {
                        tracing::info!("WebRTC connection established!");
                    }
                })
            }));

        Ok(())
    }

    /// Start the screen capture and encoding loop
    async fn start_capture_loop(&self) -> Result<()> {
        *self.is_streaming.write().await = true;

        let video_track = Arc::clone(&self.video_track);
        let encoder = Arc::clone(&self.encoder);
        let capture = Arc::clone(&self.capture);
        let is_streaming = Arc::clone(&self.is_streaming);
        let fps = self.config.fps;

        tokio::spawn(async move {
            let frame_duration = std::time::Duration::from_millis(1000 / fps as u64);
            let mut interval = tokio::time::interval(frame_duration);

            while *is_streaming.read().await {
                interval.tick().await;

                // Capture screen
                let frame = match capture.read().await.capture_frame().await {
                    Ok(frame) => frame,
                    Err(e) => {
                        tracing::error!("Failed to capture frame: {}", e);
                        continue;
                    }
                };

                // Encode frame
                let encoded = match encoder.write().await.encode_frame(frame).await {
                    Ok(data) => data,
                    Err(e) => {
                        tracing::error!("Failed to encode frame: {}", e);
                        continue;
                    }
                };

                // Send via WebRTC
                let sample = Sample {
                    data: encoded.data.into(),
                    timestamp: encoded.timestamp,
                    duration: frame_duration,
                    ..Default::default()
                };

                if let Err(e) = video_track.write_sample(&sample).await {
                    tracing::error!("Failed to write sample: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Helper to clone Arc references
    fn clone_refs(&self) -> Self {
        Self {
            config: self.config.clone(),
            peer_connection: Arc::clone(&self.peer_connection),
            video_track: Arc::clone(&self.video_track),
            encoder: Arc::clone(&self.encoder),
            capture: Arc::clone(&self.capture),
            signaling: Arc::clone(&self.signaling),
            is_streaming: Arc::clone(&self.is_streaming),
        }
    }
}

/// Example signaling backend using channels (for testing)
pub struct ChannelSignaling {
    session_id: String,
    tx: tokio::sync::mpsc::Sender<SignalingMessage>,
    rx: tokio::sync::mpsc::Receiver<SignalingMessage>,
}

impl ChannelSignaling {
    pub fn new(
        session_id: String,
    ) -> (
        Self,
        tokio::sync::mpsc::Receiver<SignalingMessage>,
        tokio::sync::mpsc::Sender<SignalingMessage>,
    ) {
        let (tx1, rx1) = tokio::sync::mpsc::channel(100);
        let (tx2, rx2) = tokio::sync::mpsc::channel(100);

        (
            Self {
                session_id,
                tx: tx1,
                rx: rx2,
            },
            rx1,
            tx2,
        )
    }
}

#[async_trait]
impl SignalingBackend for ChannelSignaling {
    async fn send(&self, message: SignalingMessage) -> Result<()> {
        self.tx.send(message).await?;
        Ok(())
    }

    async fn receive(&mut self) -> Result<SignalingMessage> {
        self.rx
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("Channel closed"))
    }

    fn session_id(&self) -> &str {
        &self.session_id
    }
}
