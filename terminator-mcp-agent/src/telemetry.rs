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
    use tracing::info;

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
            // End the span without blocking on export
            // Drop the span in a detached task to avoid blocking the response
            std::thread::spawn(move || {
                self.span.end();
            });
        }
    }

    pub struct StepSpan {
        span: BoxedSpan,
        start_time: std::time::Instant,
        _tool_name: String,
    }

    impl StepSpan {
        pub fn new(tool_name: &str, step_id: Option<&str>) -> Self {
            let tracer = global::tracer("terminator-mcp");
            let mut span = tracer
                .span_builder(format!("step.{tool_name}"))
                .with_kind(SpanKind::Internal)
                .start(&tracer);

            span.set_attribute(KeyValue::new("tool.name", tool_name.to_string()));
            span.set_attribute(KeyValue::new(
                "tool.start_time",
                chrono::Utc::now().to_rfc3339(),
            ));
            if let Some(id) = step_id {
                span.set_attribute(KeyValue::new("step.id", id.to_string()));
            }

            StepSpan {
                span,
                start_time: std::time::Instant::now(),
                _tool_name: tool_name.to_string(),
            }
        }

        pub fn set_attribute(&mut self, key: &str, value: String) {
            self.span
                .set_attribute(KeyValue::new(key.to_string(), value));
        }

        pub fn add_event(&mut self, name: &str, attributes: Vec<(&str, String)>) {
            let kvs: Vec<KeyValue> = attributes
                .into_iter()
                .map(|(k, v)| KeyValue::new(k.to_string(), v))
                .collect();
            self.span.add_event(name.to_string(), kvs);
        }

        pub fn record_retry(&mut self, attempt: u32, reason: &str) {
            self.span
                .set_attribute(KeyValue::new("retry.attempt", attempt as i64));
            self.span
                .set_attribute(KeyValue::new("retry.reason", reason.to_string()));
            self.add_event(
                "retry",
                vec![
                    ("attempt", attempt.to_string()),
                    ("reason", reason.to_string()),
                ],
            );
        }

        pub fn set_status(&mut self, success: bool, error: Option<&str>) {
            let duration_ms = self.start_time.elapsed().as_millis() as i64;

            // Add duration and status attributes
            self.span
                .set_attribute(KeyValue::new("tool.duration_ms", duration_ms));
            self.span
                .set_attribute(KeyValue::new("tool.success", success));

            let status = if success {
                Status::Ok
            } else {
                let message = error.unwrap_or("Failed");
                self.span
                    .set_attribute(KeyValue::new("error.message", message.to_string()));
                self.span
                    .set_attribute(KeyValue::new("error.type", classify_error(message)));
                Status::error(message.to_string())
            };
            self.span.set_status(status);
        }

        pub fn end(mut self) {
            self.span.set_attribute(KeyValue::new(
                "tool.end_time",
                chrono::Utc::now().to_rfc3339(),
            ));
            self.span.end();
        }
    }

    fn classify_error(error: &str) -> String {
        let lower = error.to_lowercase();
        if lower.contains("not found") || lower.contains("unable to find") {
            "element_not_found".to_string()
        } else if lower.contains("timeout") {
            "timeout".to_string()
        } else if lower.contains("permission") || lower.contains("access") {
            "permission_denied".to_string()
        } else if lower.contains("network") || lower.contains("connection") {
            "network_error".to_string()
        } else if lower.contains("invalid") || lower.contains("validation") {
            "validation_error".to_string()
        } else {
            "other".to_string()
        }
    }

    /// Check if the OpenTelemetry collector is available
    fn check_collector_availability(endpoint: &str) -> bool {
        use std::net::{SocketAddr, TcpStream};
        use std::time::Duration;

        // Extract host and port from endpoint
        if let Ok(url) = reqwest::Url::parse(endpoint) {
            if let Some(host) = url.host_str() {
                let port = url.port().unwrap_or(4318);
                let addr = format!("{host}:{port}");

                // Try to connect with a short timeout
                if let Ok(addr) = addr.parse::<SocketAddr>() {
                    return TcpStream::connect_timeout(&addr, Duration::from_millis(100)).is_ok();
                } else {
                    // Try DNS resolution
                    use std::net::ToSocketAddrs;
                    if let Ok(mut addrs) = addr.to_socket_addrs() {
                        if let Some(addr) = addrs.next() {
                            return TcpStream::connect_timeout(&addr, Duration::from_millis(100))
                                .is_ok();
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

        // Check if running in CI environment
        let is_ci = std::env::var("CI").unwrap_or_default() == "true"
            || std::env::var("GITHUB_ACTIONS").unwrap_or_default() == "true";

        if is_ci {
            info!("Running in CI environment, disabling OpenTelemetry to avoid blocking");
            return Ok(());
        }

        // Set up propagator early
        global::set_text_map_propagator(TraceContextPropagator::new());

        // Configure OTLP exporter
        let otlp_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:4318".to_string());

        // Get retry configuration from environment
        let retry_duration_mins = std::env::var("OTEL_RETRY_DURATION_MINS")
            .unwrap_or_else(|_| "15".to_string())
            .parse::<u64>()
            .unwrap_or(15);
        let retry_interval_secs = std::env::var("OTEL_RETRY_INTERVAL_SECS")
            .unwrap_or_else(|_| "30".to_string())
            .parse::<u64>()
            .unwrap_or(30);

        // Check if we should skip collector check entirely
        let skip_check = std::env::var("OTEL_SKIP_COLLECTOR_CHECK")
            .unwrap_or_default()
            .eq_ignore_ascii_case("true");

        if skip_check {
            info!("Skipping collector availability check (OTEL_SKIP_COLLECTOR_CHECK=true)");
            // Initialize telemetry immediately without checking
            return init_telemetry_provider(&otlp_endpoint);
        }

        info!(
            "OpenTelemetry configuration: endpoint={}, retry_duration={}m, retry_interval={}s",
            otlp_endpoint, retry_duration_mins, retry_interval_secs
        );

        // Spawn telemetry initialization in a background thread to avoid blocking
        std::thread::spawn(move || {
            let start_time = std::time::Instant::now();
            let max_duration = Duration::from_secs(retry_duration_mins * 60);
            let retry_interval = Duration::from_secs(retry_interval_secs);

            let mut attempt = 0;
            loop {
                attempt += 1;
                let collector_available = check_collector_availability(&otlp_endpoint);

                if collector_available {
                    info!(
                        "OpenTelemetry collector is available at {} (attempt {})",
                        otlp_endpoint, attempt
                    );

                    // Initialize telemetry now that collector is available
                    if let Err(e) = init_telemetry_provider(&otlp_endpoint) {
                        tracing::error!("Failed to initialize telemetry provider: {}", e);
                    }
                    break;
                }

                let elapsed = start_time.elapsed();
                if elapsed >= max_duration {
                    info!(
                        "OpenTelemetry collector not available at {} after {} minutes. Telemetry will be disabled.",
                        otlp_endpoint,
                        retry_duration_mins
                    );
                    break;
                }

                info!(
                    "OpenTelemetry collector not available at {} (attempt {}). Retrying in {} seconds... ({:.1} minutes elapsed)",
                    otlp_endpoint,
                    attempt,
                    retry_interval_secs,
                    elapsed.as_secs_f64() / 60.0
                );
                std::thread::sleep(retry_interval);
            }
        });

        // Return immediately to avoid blocking the main thread
        Ok(())
    }

    fn init_telemetry_provider(otlp_endpoint: &str) -> anyhow::Result<()> {
        info!(
            "Initializing OpenTelemetry with endpoint: {}",
            otlp_endpoint
        );

        // Shared resource for both traces and logs
        let resource = Resource::from_schema_url(
            [
                KeyValue::new(SERVICE_NAME, "terminator-mcp-agent"),
                KeyValue::new(SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
            ],
            SCHEMA_URL,
        );

        // Initialize traces exporter
        let trace_exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .with_endpoint(format!("{otlp_endpoint}/v1/traces"))
            .with_timeout(Duration::from_millis(500))
            .build()?;

        // Create tracer provider with OTLP exporter
        let tracer_provider = SdkTracerProvider::builder()
            .with_batch_exporter(trace_exporter, runtime::Tokio)
            .with_resource(resource)
            .build();

        global::set_tracer_provider(tracer_provider);

        info!("OpenTelemetry traces initialized successfully");
        Ok(())
    }

    pub fn shutdown_telemetry() {
        // Shutdown with a short timeout to avoid hanging
        global::shutdown_tracer_provider();
        // Note: Logger provider shutdown is handled by the tracing subscriber layer
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
        pub fn add_event(&mut self, _name: &str, _attributes: Vec<(&str, String)>) {}
        pub fn record_retry(&mut self, _attempt: u32, _reason: &str) {}
        pub fn set_status(&mut self, _success: bool, _error: Option<&str>) {}
        pub fn end(self) {}
    }

    pub fn init_telemetry() -> anyhow::Result<()> {
        Ok(())
    }

    pub fn shutdown_telemetry() {
        debug!("Telemetry disabled: shutdown_telemetry (no-op)");
    }
}
