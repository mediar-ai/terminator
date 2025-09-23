// OpenTelemetry support for MCP workflow tracing
// This module is only compiled when the 'telemetry' feature is enabled

#[cfg(feature = "telemetry")]
pub use with_telemetry::*;

#[cfg(not(feature = "telemetry"))]
pub use without_telemetry::*;

// Implementation with telemetry enabled
#[cfg(feature = "telemetry")]
mod with_telemetry {
    use opentelemetry::global::BoxedSpan;
    use opentelemetry::{
        global,
        trace::{Span, SpanKind, Status, Tracer},
        KeyValue,
    };
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::{
        propagation::TraceContextPropagator, runtime, trace::TracerProvider as SdkTracerProvider,
        Resource,
    };
    use opentelemetry_semantic_conventions::{
        attribute::{SERVICE_NAME, SERVICE_VERSION},
        SCHEMA_URL,
    };
    use std::time::Duration;
    use tracing::{info, debug};

    pub struct WorkflowSpan {
        span: BoxedSpan,
    }

    impl WorkflowSpan {
        pub fn new(name: &str) -> Self {
            let tracer = global::tracer("terminator-mcp");
            let mut span = tracer
                .span_builder(name.to_string())
                .with_kind(SpanKind::Server)
                .start(&tracer);
            span.set_attribute(KeyValue::new("workflow.name", name.to_string()));
            WorkflowSpan { span }
        }

        pub fn add_event(&mut self, name: &str, attributes: Vec<(&str, String)>) {
            let kvs: Vec<KeyValue> = attributes
                .into_iter()
                .map(|(k, v)| KeyValue::new(k.to_string(), v))
                .collect();
            self.span.add_event(name.to_string(), kvs);
        }

        pub fn set_attribute(&mut self, key: &str, value: String) {
            self.span
                .set_attribute(KeyValue::new(key.to_string(), value));
        }

        pub fn set_status(&mut self, success: bool, message: &str) {
            let status = if success {
                Status::Ok
            } else {
                Status::error(message.to_string())
            };
            self.span.set_status(status);
        }

        pub fn end(mut self) {
            self.span.end();
        }
    }

    pub struct StepSpan {
        span: BoxedSpan,
    }

    impl StepSpan {
        pub fn new(tool_name: &str, step_id: Option<&str>) -> Self {
            let tracer = global::tracer("terminator-mcp");
            let mut span = tracer
                .span_builder(format!("step.{tool_name}"))
                .with_kind(SpanKind::Internal)
                .start(&tracer);

            span.set_attribute(KeyValue::new("tool.name", tool_name.to_string()));
            if let Some(id) = step_id {
                span.set_attribute(KeyValue::new("step.id", id.to_string()));
            }

            StepSpan { span }
        }

        pub fn set_attribute(&mut self, key: &str, value: String) {
            self.span
                .set_attribute(KeyValue::new(key.to_string(), value));
        }

        pub fn set_status(&mut self, success: bool, error: Option<&str>) {
            let status = if success {
                Status::Ok
            } else {
                let message = error.unwrap_or("Failed");
                Status::error(message.to_string())
            };
            self.span.set_status(status);
        }

        pub fn end(mut self) {
            self.span.end();
        }
    }

    /// Check if the OpenTelemetry collector is available
    fn check_collector_availability(endpoint: &str) -> bool {
        use std::net::{TcpStream, SocketAddr};
        use std::time::Duration;

        // Extract host and port from endpoint
        if let Ok(url) = reqwest::Url::parse(endpoint) {
            if let Some(host) = url.host_str() {
                let port = url.port().unwrap_or(4318);
                let addr = format!("{}:{}", host, port);

                // Try to connect with a short timeout
                if let Ok(addr) = addr.parse::<SocketAddr>() {
                    return TcpStream::connect_timeout(&addr, Duration::from_millis(100)).is_ok();
                } else {
                    // Try DNS resolution
                    use std::net::ToSocketAddrs;
                    if let Ok(mut addrs) = addr.to_socket_addrs() {
                        if let Some(addr) = addrs.next() {
                            return TcpStream::connect_timeout(&addr, Duration::from_millis(100)).is_ok();
                        }
                    }
                }
            }
        }
        false
    }

    pub fn init_telemetry() -> anyhow::Result<()> {
        // Check if telemetry is enabled via environment variable
        if std::env::var("OTEL_SDK_DISABLED").unwrap_or_default() == "true" {
            info!("OpenTelemetry is disabled via OTEL_SDK_DISABLED");
            return Ok(());
        }

        // Set up propagator
        global::set_text_map_propagator(TraceContextPropagator::new());

        // Configure OTLP exporter
        let otlp_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:4318".to_string());

        // Check if collector is available (optional, for early detection)
        // Can be disabled with OTEL_SKIP_COLLECTOR_CHECK=true for environments where
        // the collector starts after the application
        let skip_check = std::env::var("OTEL_SKIP_COLLECTOR_CHECK")
            .unwrap_or_default()
            .eq_ignore_ascii_case("true");

        if !skip_check {
            let collector_available = check_collector_availability(&otlp_endpoint);
            if !collector_available {
                debug!("OpenTelemetry collector not available at {}. Telemetry will be disabled. Set OTEL_SKIP_COLLECTOR_CHECK=true to skip this check.", otlp_endpoint);
                // Silently disable telemetry if collector is not available
                return Ok(());
            }
        }

        info!(
            "Initializing OpenTelemetry with endpoint: {}",
            otlp_endpoint
        );

        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .with_endpoint(format!("{}/v1/traces", &otlp_endpoint))
            .with_timeout(Duration::from_secs(3))
            .build()?;

        // Create tracer provider with OTLP exporter
        let provider = SdkTracerProvider::builder()
            .with_batch_exporter(exporter, runtime::Tokio)
            .with_resource(Resource::from_schema_url(
                [
                    KeyValue::new(SERVICE_NAME, "terminator-mcp-agent"),
                    KeyValue::new(SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
                ],
                SCHEMA_URL,
            ))
            .build();

        global::set_tracer_provider(provider);

        info!("OpenTelemetry telemetry initialized successfully");
        Ok(())
    }

    pub fn shutdown_telemetry() {
        // Shutdown with a short timeout to avoid hanging
        let _ = global::shutdown_tracer_provider();
    }
}

// Stub implementation when telemetry is disabled
#[cfg(not(feature = "telemetry"))]
mod without_telemetry {
    use tracing::debug;

    pub struct WorkflowSpan;

    impl WorkflowSpan {
        pub fn new(_name: &str) -> Self {
            debug!("Telemetry disabled: WorkflowSpan created (no-op)");
            WorkflowSpan
        }

        pub fn add_event(&mut self, _name: &str, _attributes: Vec<(&str, String)>) {}
        pub fn set_attribute(&mut self, _key: &str, _value: String) {}
        pub fn set_status(&mut self, _success: bool, _message: &str) {}
        pub fn end(self) {}
    }

    pub struct StepSpan;

    impl StepSpan {
        pub fn new(_tool_name: &str, _step_id: Option<&str>) -> Self {
            debug!("Telemetry disabled: StepSpan created (no-op)");
            StepSpan
        }

        pub fn set_attribute(&mut self, _key: &str, _value: String) {}
        pub fn set_status(&mut self, _success: bool, _error: Option<&str>) {}
        pub fn end(self) {}
    }

    pub fn init_telemetry() -> anyhow::Result<()> {
        debug!("Telemetry disabled: init_telemetry (no-op)");
        Ok(())
    }

    pub fn shutdown_telemetry() {
        debug!("Telemetry disabled: shutdown_telemetry (no-op)");
    }
}
