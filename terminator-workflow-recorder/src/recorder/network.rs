//! Network recorder implementation (optional)
//! This module is only compiled when the "network_capture" cargo feature is enabled.
//!
//! For now we rely on the `pcap` crate to capture raw packets on the first non-loopback
//! network interface. We emit very lightweight `NetworkEvent` objects that contain basic
//! metadata about each captured packet (protocol, src/dst, ports and payload size).
//!
//! NOTE: Capturing network traffic usually requires elevated privileges on most
//! operating systems. If the capture cannot be started due to insufficient
//! permissions or missing libpcap installation, the recorder will gracefully
//! fall back to a no-op implementation rather than crashing the workflow
//! recorder. This way the rest of the recording pipeline continues to work.

#[cfg(feature = "network_capture")]
use {
    crate::{events::NetworkEvent, EventMetadata, WorkflowEvent, WorkflowRecorderError},
    pcap::{Capture, Device},
    tokio::{sync::broadcast::Sender, task::JoinHandle},
    tracing::{info, warn},
};

/// Recorder that passively captures network packets and pushes them to the shared broadcast
/// channel so they become part of the workflow recording.
#[cfg(feature = "network_capture")]
pub struct NetworkRecorder {
    handle: JoinHandle<()>,
}

#[cfg(feature = "network_capture")]
impl NetworkRecorder {
    /// Start capturing packets on the first available non-loopback interface.
    pub async fn new(event_tx: Sender<WorkflowEvent>) -> Result<Self, WorkflowRecorderError> {
        // Spawn the capture task on a blocking thread because libpcap is blocking
        let handle = tokio::task::spawn_blocking(move || {
            // Select the first active, non-loopback interface
            let device = match Device::list().ok().and_then(|d| d.into_iter().find(|d| !d.flags.contains(pcap::DeviceFlags::LOOPBACK))) {
                Some(dev) => dev,
                None => {
                    warn!("No suitable network device found for capture – network recording disabled");
                    return;
                }
            };

            info!("Starting network capture on {}", device.name);
            let mut cap = match Capture::from_device(device).and_then(|c| c.promisc(true).immediate_mode(true).open()) {
                Ok(c) => c,
                Err(e) => {
                    warn!("Failed to start network capture: {e}");
                    return;
                }
            };

            // Main capture loop
            while let Ok(packet) = cap.next() {
                // Very lightweight parsing – we only care about a few header fields
                if let Some(net_event) = Self::build_event(&packet) {
                    // Ignore errors if there are no active listeners
                    let _ = event_tx.send(WorkflowEvent::Network(net_event));
                }
            }
        });

        Ok(Self { handle })
    }

    /// Stop capturing by aborting the underlying task.
    pub async fn stop(self) -> Result<(), WorkflowRecorderError> {
        self.handle.abort();
        let _ = self.handle.await; // Drop any error
        Ok(())
    }

    /// Very small helper to convert a `pcap::Packet` into our `NetworkEvent`.
    fn build_event(packet: &pcap::Packet) -> Option<NetworkEvent> {
        use etherparse::InternetSlice::Ipv4;
        use etherparse::{SlicedPacket, TransportSlice};

        // Parse headers with etherparse for convenience
        let sliced = SlicedPacket::from_ethernet(&packet.data).ok()?;

        let (src_ip, dest_ip) = match sliced.ip {
            Some(Ipv4(header, _)) => (header.source_addr().to_string(), header.destination_addr().to_string()),
            Some(etherparse::InternetSlice::Ipv6(header, _)) => (header.source_addr().to_string(), header.destination_addr().to_string()),
            _ => return None,
        };

        let (protocol, src_port, dest_port) = match sliced.transport {
            Some(TransportSlice::Tcp(tcp)) => ("TCP".to_string(), Some(tcp.source_port()), Some(tcp.destination_port())),
            Some(TransportSlice::Udp(udp)) => ("UDP".to_string(), Some(udp.source_port()), Some(udp.destination_port())),
            Some(TransportSlice::Icmpv4(_)) => ("ICMP".to_string(), None, None),
            Some(TransportSlice::Icmpv6(_)) => ("ICMPv6".to_string(), None, None),
            _ => ("OTHER".to_string(), None, None),
        };

        Some(NetworkEvent {
            protocol,
            src_ip,
            src_port,
            dest_ip,
            dest_port,
            packet_length: packet.header.len as usize,
            metadata: EventMetadata::with_timestamp(),
        })
    }
}

/// Stub implementation compiled when the feature is NOT enabled – keeps API surface identical.
#[cfg(not(feature = "network_capture"))]
pub struct NetworkRecorder;

#[cfg(not(feature = "network_capture"))]
impl NetworkRecorder {
    pub async fn new(_: tokio::sync::broadcast::Sender<crate::WorkflowEvent>) -> Result<Self, crate::WorkflowRecorderError> {
        Ok(Self)
    }

    pub async fn stop(self) -> Result<(), crate::WorkflowRecorderError> {
        Ok(())
    }
}