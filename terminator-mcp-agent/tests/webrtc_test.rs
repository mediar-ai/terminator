#[cfg(feature = "webrtc-streaming")]
mod webrtc_tests {
    use terminator_mcp_agent::streaming::{SignalingMessage, StreamerConfig};

    #[test]
    fn test_streamer_config_creation() {
        let config = StreamerConfig::default();
        assert_eq!(config.width, 1920);
        assert_eq!(config.height, 1080);
        assert_eq!(config.fps, 15);
        assert_eq!(config.bitrate, 2_000_000);
    }

    #[test]
    fn test_signaling_message_serialization() {
        let offer = SignalingMessage::Offer {
            sdp: "test-sdp".to_string(),
        };

        let json = serde_json::to_string(&offer).unwrap();
        assert!(json.contains("\"type\":\"offer\""));
        assert!(json.contains("\"sdp\":\"test-sdp\""));

        let deserialized: SignalingMessage = serde_json::from_str(&json).unwrap();
        match deserialized {
            SignalingMessage::Offer { sdp } => assert_eq!(sdp, "test-sdp"),
            _ => panic!("Wrong message type"),
        }
    }
}
