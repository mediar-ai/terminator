//! Example demonstrating WebRTC streaming with Terminator MCP
//!
//! This example shows how to use the optional WebRTC streaming feature
//! to stream the desktop in real-time.
//!
//! To run this example with WebRTC support:
//! ```bash
//! cd terminator-mcp-agent
//! cargo run --features webrtc-streaming --example webrtc_streaming
//! ```

#[cfg(feature = "webrtc-streaming")]
use terminator_mcp_agent::streaming::{
    ChannelSignaling, SignalingBackend, SignalingMessage, StreamerConfig, WebRTCStreamer,
};

#[cfg(not(feature = "webrtc-streaming"))]
fn main() {
    println!("This example requires the 'webrtc-streaming' feature.");
    println!("Run with: cargo run --features webrtc-streaming --example webrtc_streaming");
}

#[cfg(feature = "webrtc-streaming")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    println!("=== Terminator WebRTC Streaming Example ===\n");

    // Create a simple channel-based signaling for demonstration
    // In production, you would use WebSocket, HTTP, or Supabase signaling
    let (signaling, mut rx, tx) = ChannelSignaling::new("demo-session".to_string());

    // Configure the streamer
    let config = StreamerConfig {
        width: 1280,
        height: 720,
        fps: 15,
        bitrate: 1_500_000, // 1.5 Mbps
        ..Default::default()
    };

    println!("Creating WebRTC streamer with config:");
    println!("  Resolution: {}x{}", config.width, config.height);
    println!("  FPS: {}", config.fps);
    println!("  Bitrate: {} bps", config.bitrate);
    println!();

    // Create the streamer
    let streamer = WebRTCStreamer::new(config, Box::new(signaling)).await?;

    // Start streaming
    println!("Starting WebRTC stream...");
    streamer.start().await?;

    // Simulate a viewer connecting
    tokio::spawn(async move {
        println!("\n[Viewer] Waiting for offer...");

        // Wait for the offer from the streamer
        if let Ok(SignalingMessage::Offer { sdp }) = rx.recv().await {
            println!("[Viewer] Received offer, would create answer here");

            // In a real implementation, the viewer would:
            // 1. Create its own RTCPeerConnection
            // 2. Set the remote description with the offer
            // 3. Create an answer
            // 4. Send the answer back

            // Simulate sending an answer
            let _ = tx
                .send(SignalingMessage::Answer {
                    sdp: "mock-answer-sdp".to_string(),
                })
                .await;

            println!("[Viewer] Sent answer back");
        }

        // Handle ICE candidates
        while let Ok(msg) = rx.recv().await {
            match msg {
                SignalingMessage::IceCandidate { candidate, .. } => {
                    println!(
                        "[Viewer] Received ICE candidate: {}",
                        &candidate[..50.min(candidate.len())]
                    );
                }
                SignalingMessage::StopStream => {
                    println!("[Viewer] Stream stopped");
                    break;
                }
                _ => {}
            }
        }
    });

    // Let the stream run for a bit
    println!("\nStreaming desktop for 10 seconds...");
    println!("In a real application, you would exchange signaling messages with a web client.");
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

    // Stop streaming
    println!("\nStopping stream...");
    streamer.stop().await?;

    println!("Done!");
    Ok(())
}

#[cfg(feature = "webrtc-streaming")]
mod web_client_example {
    /// Example HTML/JavaScript code for the web client side
    pub const WEB_CLIENT: &str = r#"
<!DOCTYPE html>
<html>
<head>
    <title>Terminator WebRTC Viewer</title>
</head>
<body>
    <h1>Terminator Desktop Stream</h1>
    <video id="remoteVideo" autoplay playsinline style="width: 100%; max-width: 1280px;"></video>
    
    <script>
        // Configuration - adjust to match your signaling server
        const SIGNALING_URL = 'wss://your-signaling-server.com';
        const SESSION_ID = 'unique-session-id';
        
        let pc;
        let ws;
        
        async function start() {
            // Create peer connection
            pc = new RTCPeerConnection({
                iceServers: [{ urls: 'stun:stun.l.google.com:19302' }]
            });
            
            // Handle incoming video stream
            pc.ontrack = (event) => {
                const video = document.getElementById('remoteVideo');
                video.srcObject = event.streams[0];
            };
            
            // Handle ICE candidates
            pc.onicecandidate = (event) => {
                if (event.candidate) {
                    ws.send(JSON.stringify({
                        type: 'iceCandidate',
                        candidate: event.candidate.candidate,
                        sdpMid: event.candidate.sdpMid,
                        sdpMlineIndex: event.candidate.sdpMLineIndex
                    }));
                }
            };
            
            // Connect to signaling server
            ws = new WebSocket(SIGNALING_URL);
            
            ws.onmessage = async (event) => {
                const msg = JSON.parse(event.data);
                
                switch (msg.type) {
                    case 'offer':
                        // Set remote description
                        await pc.setRemoteDescription(new RTCSessionDescription({
                            type: 'offer',
                            sdp: msg.sdp
                        }));
                        
                        // Create and send answer
                        const answer = await pc.createAnswer();
                        await pc.setLocalDescription(answer);
                        
                        ws.send(JSON.stringify({
                            type: 'answer',
                            sdp: answer.sdp
                        }));
                        break;
                        
                    case 'iceCandidate':
                        await pc.addIceCandidate(new RTCIceCandidate({
                            candidate: msg.candidate,
                            sdpMid: msg.sdpMid,
                            sdpMLineIndex: msg.sdpMlineIndex
                        }));
                        break;
                }
            };
            
            ws.onopen = () => {
                console.log('Connected to signaling server');
                // Request to start streaming
                ws.send(JSON.stringify({ type: 'startStream' }));
            };
        }
        
        // Start when page loads
        window.onload = start;
    </script>
</body>
</html>
    "#;
}
