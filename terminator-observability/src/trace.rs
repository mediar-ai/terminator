//! Trace representation for automation execution

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Represents a complete trace of an automation session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trace {
    /// Unique trace ID
    pub id: Uuid,
    /// Task name
    pub task_name: String,
    /// Start timestamp
    pub start_time: chrono::DateTime<chrono::Utc>,
    /// Total duration
    pub duration: Duration,
    /// Spans within the trace
    pub spans: Vec<Span>,
    /// Metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Trace {
    /// Get the root spans (spans without parents)
    pub fn root_spans(&self) -> Vec<&Span> {
        self.spans
            .iter()
            .filter(|s| s.parent_span_id.is_none())
            .collect()
    }

    /// Get child spans of a given parent
    pub fn child_spans(&self, parent_id: Uuid) -> Vec<&Span> {
        self.spans
            .iter()
            .filter(|s| s.parent_span_id == Some(parent_id))
            .collect()
    }

    /// Calculate total error count
    pub fn error_count(&self) -> usize {
        self.spans.iter().filter(|s| s.status.is_error()).count()
    }

    /// Calculate success rate
    pub fn success_rate(&self) -> f64 {
        if self.spans.is_empty() {
            1.0
        } else {
            (self.spans.len() - self.error_count()) as f64 / self.spans.len() as f64
        }
    }

    /// Export as JSON
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    /// Create from JSON
    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }
}

/// Represents a single span in a trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    /// Unique span ID
    pub id: Uuid,
    /// Parent span ID (if nested)
    pub parent_span_id: Option<Uuid>,
    /// Operation name
    pub operation: String,
    /// Start time relative to trace start
    pub start_time: Duration,
    /// Duration
    pub duration: Duration,
    /// Status
    pub status: SpanStatus,
    /// Attributes
    pub attributes: HashMap<String, AttributeValue>,
    /// Events that occurred during the span
    pub events: Vec<SpanEvent>,
}

impl Span {
    /// Create a new span
    pub fn new(operation: String, start_time: Duration) -> Self {
        Self {
            id: Uuid::new_v4(),
            parent_span_id: None,
            operation,
            start_time,
            duration: Duration::default(),
            status: SpanStatus::Unset,
            attributes: HashMap::new(),
            events: Vec::new(),
        }
    }

    /// Set the parent span
    pub fn with_parent(mut self, parent_id: Uuid) -> Self {
        self.parent_span_id = Some(parent_id);
        self
    }

    /// Add an attribute
    pub fn add_attribute<K, V>(&mut self, key: K, value: V)
    where
        K: Into<String>,
        V: Into<AttributeValue>,
    {
        self.attributes.insert(key.into(), value.into());
    }

    /// Add an event
    pub fn add_event(&mut self, event: SpanEvent) {
        self.events.push(event);
    }

    /// Complete the span
    pub fn complete(&mut self, duration: Duration, status: SpanStatus) {
        self.duration = duration;
        self.status = status;
    }
}

/// Span status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SpanStatus {
    /// Successful completion
    Ok,
    /// Error occurred
    Error { description: String },
    /// Status not set
    Unset,
}

impl SpanStatus {
    /// Check if this is an error status
    pub fn is_error(&self) -> bool {
        matches!(self, SpanStatus::Error { .. })
    }
}

/// Attribute value types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AttributeValue {
    /// String value
    String(String),
    /// Integer value
    Int(i64),
    /// Float value
    Float(f64),
    /// Boolean value
    Bool(bool),
    /// Array of strings
    StringArray(Vec<String>),
}

impl From<String> for AttributeValue {
    fn from(v: String) -> Self {
        AttributeValue::String(v)
    }
}

impl From<&str> for AttributeValue {
    fn from(v: &str) -> Self {
        AttributeValue::String(v.to_string())
    }
}

impl From<i64> for AttributeValue {
    fn from(v: i64) -> Self {
        AttributeValue::Int(v)
    }
}

impl From<f64> for AttributeValue {
    fn from(v: f64) -> Self {
        AttributeValue::Float(v)
    }
}

impl From<bool> for AttributeValue {
    fn from(v: bool) -> Self {
        AttributeValue::Bool(v)
    }
}

impl From<Vec<String>> for AttributeValue {
    fn from(v: Vec<String>) -> Self {
        AttributeValue::StringArray(v)
    }
}

/// An event that occurred during a span
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanEvent {
    /// Event name
    pub name: String,
    /// Timestamp relative to span start
    pub timestamp: Duration,
    /// Event attributes
    pub attributes: HashMap<String, AttributeValue>,
}

impl SpanEvent {
    /// Create a new event
    pub fn new(name: String, timestamp: Duration) -> Self {
        Self {
            name,
            timestamp,
            attributes: HashMap::new(),
        }
    }

    /// Add an attribute to the event
    pub fn with_attribute<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Into<AttributeValue>,
    {
        self.attributes.insert(key.into(), value.into());
        self
    }
}

/// Builder for creating traces
pub struct TraceBuilder {
    task_name: String,
    metadata: HashMap<String, serde_json::Value>,
}

impl TraceBuilder {
    /// Create a new trace builder
    pub fn new(task_name: impl Into<String>) -> Self {
        Self {
            task_name: task_name.into(),
            metadata: HashMap::new(),
        }
    }

    /// Add metadata
    pub fn with_metadata<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Serialize,
    {
        if let Ok(json_value) = serde_json::to_value(value) {
            self.metadata.insert(key.into(), json_value);
        }
        self
    }

    /// Build the trace
    pub fn build(self) -> Trace {
        Trace {
            id: Uuid::new_v4(),
            task_name: self.task_name,
            start_time: chrono::Utc::now(),
            duration: Duration::default(),
            spans: Vec::new(),
            metadata: self.metadata,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_creation() {
        let trace = TraceBuilder::new("test_task")
            .with_metadata("version", "1.0.0")
            .build();

        assert_eq!(trace.task_name, "test_task");
        assert!(trace.metadata.contains_key("version"));
    }

    #[test]
    fn test_span_attributes() {
        let mut span = Span::new("test_operation".to_string(), Duration::from_secs(0));
        span.add_attribute("key1", "value1");
        span.add_attribute("key2", 42i64);
        span.add_attribute("key3", 3.14f64);
        span.add_attribute("key4", true);

        assert_eq!(span.attributes.len(), 4);
    }
}
