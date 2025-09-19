use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "version")]
pub enum ProtocolMessage {
    V1(ProtocolV1),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolV1 {
    pub message_type: MessageType,
    pub payload: serde_json::Value,
    pub metadata: MessageMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetadata {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub correlation_id: String,
    pub session_id: Option<String>,
    pub retry_count: u32,
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MessageType {
    Request,
    Response,
    Event,
    Heartbeat,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRequest {
    pub requests: Vec<RemoteBatchAction>,
    pub parallel: bool,
    pub stop_on_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteBatchAction {
    pub id: String,
    pub action: crate::remote_server::RemoteAction,
    pub depends_on: Vec<String>,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResponse {
    pub results: Vec<BatchActionResult>,
    pub total_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchActionResult {
    pub id: String,
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventNotification {
    pub event_type: EventType,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    ElementChanged,
    WindowOpened,
    WindowClosed,
    ApplicationStarted,
    ApplicationClosed,
    FocusChanged,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityInfo {
    pub capabilities: Vec<Capability>,
    pub platform: PlatformInfo,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub name: String,
    pub supported: bool,
    pub options: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformInfo {
    pub os: String,
    pub arch: String,
    pub ui_framework: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    pub enabled: bool,
    pub algorithm: CompressionAlgorithm,
    pub threshold_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompressionAlgorithm {
    Gzip,
    Zstd,
    Lz4,
}

impl Default for MessageMetadata {
    fn default() -> Self {
        Self {
            timestamp: chrono::Utc::now(),
            correlation_id: uuid::Uuid::new_v4().to_string(),
            session_id: None,
            retry_count: 0,
            headers: HashMap::new(),
        }
    }
}

pub trait ProtocolEncoder {
    fn encode(&self) -> Result<Vec<u8>, anyhow::Error>;
}

pub trait ProtocolDecoder {
    fn decode(data: &[u8]) -> Result<Self, anyhow::Error>
    where
        Self: Sized;
}

impl ProtocolEncoder for ProtocolMessage {
    fn encode(&self) -> Result<Vec<u8>, anyhow::Error> {
        let json = serde_json::to_vec(self)?;
        Ok(json)
    }
}

impl ProtocolDecoder for ProtocolMessage {
    fn decode(data: &[u8]) -> Result<Self, anyhow::Error> {
        let message = serde_json::from_slice(data)?;
        Ok(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_encoding_decoding() {
        let message = ProtocolMessage::V1(ProtocolV1 {
            message_type: MessageType::Request,
            payload: serde_json::json!({"test": "data"}),
            metadata: MessageMetadata::default(),
        });

        let encoded = message.encode().unwrap();
        let decoded = ProtocolMessage::decode(&encoded).unwrap();

        match decoded {
            ProtocolMessage::V1(v1) => {
                assert_eq!(v1.payload, serde_json::json!({"test": "data"}));
            }
        }
    }

    #[test]
    fn test_batch_request_serialization() {
        let batch = BatchRequest {
            requests: vec![],
            parallel: true,
            stop_on_error: false,
        };

        let json = serde_json::to_string(&batch).unwrap();
        let deserialized: BatchRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.parallel, batch.parallel);
        assert_eq!(deserialized.stop_on_error, batch.stop_on_error);
    }
}