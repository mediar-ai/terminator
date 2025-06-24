//! Telemetry implementation using OpenTelemetry

use crate::{context::Config, error::Result, Error};
use opentelemetry::{
    global,
    trace::{Span as OtelSpan, SpanId, TraceContextExt, Tracer, TracerProvider as _},
    Context, KeyValue,
};
use opentelemetry_sdk::{
    trace::{Config as TraceConfig, RandomIdGenerator, Sampler, TracerProvider},
    Resource,
};
use std::sync::Arc;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Registry};

/// Telemetry provider for managing traces
#[derive(Debug, Clone)]
pub struct TelemetryProvider {
    tracer: Arc<opentelemetry_sdk::trace::Tracer>,
}

impl TelemetryProvider {
    /// Create a new telemetry provider
    pub fn new(config: &Config) -> Result<Self> {
        // Create resource with service information
        let resource = Resource::new(vec![
            KeyValue::new("service.name", config.service_name.clone()),
            KeyValue::new("service.version", config.service_version.clone()),
        ]);

        // Configure trace settings
        let trace_config = TraceConfig::default()
            .with_sampler(Sampler::TraceIdRatioBased(config.sampling_ratio))
            .with_id_generator(RandomIdGenerator::default())
            .with_resource(resource);

        // Create tracer provider
        let provider = TracerProvider::builder().with_config(trace_config).build();

        // Set up exporters
        #[cfg(feature = "otlp")]
        if let Some(endpoint) = &config.otlp_endpoint {
            use opentelemetry_otlp::WithExportConfig;

            let exporter = opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(endpoint);

            let trace_exporter = opentelemetry_otlp::new_pipeline()
                .tracing()
                .with_exporter(exporter)
                .with_trace_config(trace_config.clone())
                .install_batch(opentelemetry_sdk::runtime::Tokio)
                .map_err(|e| {
                    Error::TelemetryInit(format!("Failed to create OTLP exporter: {}", e))
                })?;
        }

        if config.enable_stdout_exporter {
            // Add stdout exporter for debugging
            let stdout_exporter = opentelemetry_sdk::export::trace::stdout::new_pipeline()
                .with_trace_config(trace_config.clone())
                .install_simple();
        }

        // Get tracer
        let tracer = provider.tracer("terminator-observability");

        // Set global provider
        global::set_tracer_provider(provider);

        // Initialize tracing subscriber
        let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer.clone());

        Registry::default()
            .with(telemetry_layer)
            .with(tracing_subscriber::EnvFilter::from_default_env())
            .try_init()
            .ok(); // Ignore if already initialized

        Ok(Self {
            tracer: Arc::new(tracer),
        })
    }

    /// Create a new span builder
    pub fn span(&self, name: &str) -> SpanBuilder {
        SpanBuilder::new(self.tracer.clone(), name.to_string())
    }

    /// Shutdown telemetry and flush all data
    pub fn shutdown(&self) -> Result<()> {
        global::shutdown_tracer_provider();
        Ok(())
    }
}

/// Builder for creating spans
pub struct SpanBuilder {
    tracer: Arc<opentelemetry_sdk::trace::Tracer>,
    name: String,
    kind: SpanKind,
    attributes: Vec<KeyValue>,
    parent: Option<SpanId>,
}

impl SpanBuilder {
    fn new(tracer: Arc<opentelemetry_sdk::trace::Tracer>, name: String) -> Self {
        Self {
            tracer,
            name,
            kind: SpanKind::Internal,
            attributes: Vec::new(),
            parent: None,
        }
    }

    /// Set the span kind
    pub fn with_kind(mut self, kind: SpanKind) -> Self {
        self.kind = kind;
        self
    }

    /// Add an attribute to the span
    pub fn with_attribute<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Into<opentelemetry::Value>,
    {
        self.attributes
            .push(KeyValue::new(key.into(), value.into()));
        self
    }

    /// Set the parent span
    pub fn with_parent(mut self, parent: SpanId) -> Self {
        self.parent = Some(parent);
        self
    }

    /// Start the span
    pub fn start(self) -> ActiveSpan {
        let mut span_builder = self
            .tracer
            .span_builder(self.name)
            .with_kind(self.kind.into())
            .with_attributes(self.attributes);

        if let Some(parent) = self.parent {
            // TODO: Set parent context
        }

        let span = span_builder.start(&*self.tracer);
        ActiveSpan::new(span)
    }
}

/// An active span that records telemetry
pub struct ActiveSpan {
    span: Box<dyn OtelSpan>,
}

impl ActiveSpan {
    fn new(span: Box<dyn OtelSpan>) -> Self {
        Self { span }
    }

    /// Set the span status
    pub fn set_status(&self, status: SpanStatus) {
        use opentelemetry::trace::Status;

        let otel_status = match status {
            SpanStatus::Ok => Status::Ok,
            SpanStatus::Error { description } => Status::error(description),
            SpanStatus::Unset => Status::Unset,
        };

        self.span.set_status(otel_status);
    }

    /// Add an attribute to the span
    pub fn set_attribute<K, V>(&self, key: K, value: V)
    where
        K: Into<String>,
        V: Into<opentelemetry::Value>,
    {
        self.span
            .set_attribute(KeyValue::new(key.into(), value.into()));
    }

    /// Add an event to the span
    pub fn add_event<T>(&self, name: T, attributes: Vec<KeyValue>)
    where
        T: Into<std::borrow::Cow<'static, str>>,
    {
        self.span.add_event(name, attributes);
    }

    /// End the span
    pub fn end(self) {
        self.span.end();
    }
}

/// Span kind enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanKind {
    /// Internal operation
    Internal,
    /// Client operation (outgoing request)
    Client,
    /// Server operation (incoming request)
    Server,
    /// Producer operation (async producer)
    Producer,
    /// Consumer operation (async consumer)
    Consumer,
}

impl From<SpanKind> for opentelemetry::trace::SpanKind {
    fn from(kind: SpanKind) -> Self {
        match kind {
            SpanKind::Internal => opentelemetry::trace::SpanKind::Internal,
            SpanKind::Client => opentelemetry::trace::SpanKind::Client,
            SpanKind::Server => opentelemetry::trace::SpanKind::Server,
            SpanKind::Producer => opentelemetry::trace::SpanKind::Producer,
            SpanKind::Consumer => opentelemetry::trace::SpanKind::Consumer,
        }
    }
}

/// Span status enumeration
#[derive(Debug, Clone)]
pub enum SpanStatus {
    /// The operation completed successfully
    Ok,
    /// The operation failed
    Error { description: String },
    /// Status not set
    Unset,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_kind_conversion() {
        assert_eq!(
            opentelemetry::trace::SpanKind::from(SpanKind::Client),
            opentelemetry::trace::SpanKind::Client
        );
    }
}
