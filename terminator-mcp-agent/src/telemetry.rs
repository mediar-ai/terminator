// Enhanced OpenTelemetry support with metrics, detailed tracing, and best practices
// This module provides comprehensive telemetry for legacy software automation

#[cfg(feature = "telemetry")]
pub use with_telemetry::*;

#[cfg(not(feature = "telemetry"))]
pub use without_telemetry::*;

#[cfg(feature = "telemetry")]
mod with_telemetry {
    use opentelemetry::global::BoxedSpan;
    use opentelemetry::{
        global,
        metrics::{Counter, Histogram},
        trace::{Span, SpanKind, Status, TraceContextExt, Tracer},
        Context, KeyValue,
    };
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::{
        metrics::{MeterProviderBuilder, PeriodicReader},
        propagation::TraceContextPropagator,
        runtime,
        trace::{RandomIdGenerator, Sampler, TracerProvider as SdkTracerProvider},
        Resource,
    };
    use opentelemetry_semantic_conventions::{
        attribute::{SERVICE_NAME, SERVICE_VERSION},
        SCHEMA_URL,
    };
    use std::time::{Duration, Instant};
    use tracing::info;
    use uuid::Uuid;

    // Global metrics
    lazy_static::lazy_static! {
        static ref TOOL_EXECUTION_COUNTER: Counter<u64> = global::meter("terminator-mcp")
            .u64_counter("tool.executions")
            .with_description("Total number of tool executions")
            .build();

        static ref TOOL_SUCCESS_COUNTER: Counter<u64> = global::meter("terminator-mcp")
            .u64_counter("tool.successes")
            .with_description("Total number of successful tool executions")
            .build();

        static ref TOOL_FAILURE_COUNTER: Counter<u64> = global::meter("terminator-mcp")
            .u64_counter("tool.failures")
            .with_description("Total number of failed tool executions")
            .build();

        static ref TOOL_DURATION_HISTOGRAM: Histogram<f64> = global::meter("terminator-mcp")
            .f64_histogram("tool.duration")
            .with_description("Duration of tool executions in milliseconds")
            .build();

        static ref ELEMENT_SEARCH_TIME: Histogram<f64> = global::meter("terminator-mcp")
            .f64_histogram("element.search_time")
            .with_description("Time taken to find UI elements in milliseconds")
            .build();

        static ref WORKFLOW_COUNTER: Counter<u64> = global::meter("terminator-mcp")
            .u64_counter("workflow.executions")
            .with_description("Total number of workflow executions")
            .build();

        static ref RETRY_COUNTER: Counter<u64> = global::meter("terminator-mcp")
            .u64_counter("tool.retries")
            .with_description("Total number of retry attempts")
            .build();
    }

    // Enhanced WorkflowSpan with metrics and detailed tracking
    pub struct WorkflowSpan {
        span: BoxedSpan,
        start_time: Instant,
        workflow_id: String,
        correlation_id: String,
    }

    impl WorkflowSpan {
        pub fn new(name: &str) -> Self {
            let tracer = global::tracer("terminator-mcp");
            let workflow_id = Uuid::new_v4().to_string();
            let correlation_id = Uuid::new_v4().to_string();

            let span = tracer
                .span_builder(name.to_string())
                .with_kind(SpanKind::Server)
                .with_attributes(vec![
                    KeyValue::new("workflow.name", name.to_string()),
                    KeyValue::new("workflow.id", workflow_id.clone()),
                    KeyValue::new("correlation.id", correlation_id.clone()),
                    KeyValue::new("workflow.start_time", chrono::Utc::now().to_rfc3339()),
                ])
                .start(&tracer);

            // Record workflow start
            WORKFLOW_COUNTER.add(1, &[KeyValue::new("workflow.name", name.to_string())]);

            WorkflowSpan {
                span,
                start_time: Instant::now(),
                workflow_id,
                correlation_id,
            }
        }

        pub fn add_event(&mut self, name: &str, attributes: Vec<(&str, String)>) {
            let mut kvs: Vec<KeyValue> = attributes
                .into_iter()
                .map(|(k, v)| KeyValue::new(k.to_string(), v))
                .collect();

            // Add workflow context to events
            kvs.push(KeyValue::new("workflow.id", self.workflow_id.clone()));
            kvs.push(KeyValue::new("event.timestamp", chrono::Utc::now().to_rfc3339()));

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

            // Record workflow outcome
            self.span.set_attribute(KeyValue::new("workflow.success", success));
            if !success {
                self.span.set_attribute(KeyValue::new("workflow.error", message.to_string()));
            }
        }

        pub fn end(mut self) {
            let duration_ms = self.start_time.elapsed().as_millis() as i64;
            self.span.set_attribute(KeyValue::new("workflow.duration_ms", duration_ms));
            self.span.set_attribute(KeyValue::new("workflow.end_time", chrono::Utc::now().to_rfc3339()));
            self.span.end();
        }

        pub fn get_correlation_id(&self) -> &str {
            &self.correlation_id
        }

        pub fn get_workflow_id(&self) -> &str {
            &self.workflow_id
        }
    }

    // Enhanced StepSpan with detailed metrics
    pub struct StepSpan {
        span: BoxedSpan,
        start_time: Instant,
        tool_name: String,
    }

    impl StepSpan {
        pub fn new(tool_name: &str, step_id: Option<&str>) -> Self {
            Self::new_with_context(tool_name, step_id, None)
        }

        pub fn new_with_context(
            tool_name: &str,
            step_id: Option<&str>,
            parent_context: Option<Context>,
        ) -> Self {
            let tracer = global::tracer("terminator-mcp");
            let context = parent_context.unwrap_or_else(Context::current);

            // Start span with parent context if available
            let mut span = if context.has_active_span() {
                tracer
                    .span_builder(format!("tool.{tool_name}"))
                    .with_kind(SpanKind::Internal)
                    .with_attributes(vec![
                        KeyValue::new("tool.name", tool_name.to_string()),
                        KeyValue::new("tool.start_time", chrono::Utc::now().to_rfc3339()),
                        KeyValue::new("host.name", hostname::get().unwrap_or_default().to_string_lossy().to_string()),
                        KeyValue::new("process.pid", std::process::id() as i64),
                    ])
                    .start_with_context(&tracer, &context)
            } else {
                tracer
                    .span_builder(format!("tool.{tool_name}"))
                    .with_kind(SpanKind::Internal)
                    .with_attributes(vec![
                        KeyValue::new("tool.name", tool_name.to_string()),
                        KeyValue::new("tool.start_time", chrono::Utc::now().to_rfc3339()),
                        KeyValue::new("host.name", hostname::get().unwrap_or_default().to_string_lossy().to_string()),
                        KeyValue::new("process.pid", std::process::id() as i64),
                    ])
                    .start(&tracer)
            };

            if let Some(id) = step_id {
                span.set_attribute(KeyValue::new("step.id", id.to_string()));
            }

            // Increment execution counter
            TOOL_EXECUTION_COUNTER.add(
                1,
                &[KeyValue::new("tool.name", tool_name.to_string())],
            );

            StepSpan {
                span,
                start_time: Instant::now(),
                tool_name: tool_name.to_string(),
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
            RETRY_COUNTER.add(
                1,
                &[
                    KeyValue::new("tool.name", self.tool_name.clone()),
                    KeyValue::new("retry.reason", reason.to_string()),
                ],
            );
            self.span.set_attribute(KeyValue::new("retry.attempt", attempt as i64));
            self.span.set_attribute(KeyValue::new("retry.reason", reason.to_string()));
            self.add_event("retry", vec![
                ("attempt", attempt.to_string()),
                ("reason", reason.to_string()),
            ]);
        }

        pub fn record_element_search(&mut self, selector: &str, found: bool, search_time_ms: u64) {
            ELEMENT_SEARCH_TIME.record(
                search_time_ms as f64,
                &[
                    KeyValue::new("selector.type", Self::extract_selector_type(selector)),
                    KeyValue::new("found", found),
                ],
            );

            self.span.set_attribute(KeyValue::new("element.selector", selector.to_string()));
            self.span.set_attribute(KeyValue::new("element.found", found));
            self.span.set_attribute(KeyValue::new("element.search_time_ms", search_time_ms as i64));
        }

        fn extract_selector_type(selector: &str) -> String {
            if selector.starts_with('#') {
                "id".to_string()
            } else if selector.contains("role:") {
                "role".to_string()
            } else if selector.contains("name:") {
                "name".to_string()
            } else if selector.contains('|') {
                "composite".to_string()
            } else {
                "other".to_string()
            }
        }

        pub fn set_status(&mut self, success: bool, error: Option<&str>) {
            let duration_ms = self.start_time.elapsed().as_millis() as f64;

            // Record metrics
            TOOL_DURATION_HISTOGRAM.record(
                duration_ms,
                &[
                    KeyValue::new("tool.name", self.tool_name.clone()),
                    KeyValue::new("success", success),
                ],
            );

            if success {
                TOOL_SUCCESS_COUNTER.add(
                    1,
                    &[KeyValue::new("tool.name", self.tool_name.clone())],
                );
            } else {
                TOOL_FAILURE_COUNTER.add(
                    1,
                    &[
                        KeyValue::new("tool.name", self.tool_name.clone()),
                        KeyValue::new("error.type", Self::classify_error(error.unwrap_or("unknown"))),
                    ],
                );
            }

            // Set span status
            let status = if success {
                Status::Ok
            } else {
                let message = error.unwrap_or("Failed");
                self.span.set_attribute(KeyValue::new("error.message", message.to_string()));
                Status::error(message.to_string())
            };
            self.span.set_status(status);

            // Set final attributes
            self.span.set_attribute(KeyValue::new("tool.duration_ms", duration_ms as i64));
            self.span.set_attribute(KeyValue::new("tool.success", success));
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

        pub fn end(mut self) {
            self.span.set_attribute(KeyValue::new("tool.end_time", chrono::Utc::now().to_rfc3339()));
            self.span.end();
        }
    }

    // Helper for tracking automation-specific metrics
    pub struct AutomationMetrics;

    impl AutomationMetrics {
        pub fn record_window_change(from: Option<&str>, to: &str, duration_ms: u64) {
            global::meter("terminator-mcp")
                .f64_histogram("window.switch_time")
                .with_description("Time taken to switch windows in milliseconds")
                .build()
                .record(
                    duration_ms as f64,
                    &[
                        KeyValue::new("from_window", from.unwrap_or("unknown").to_string()),
                        KeyValue::new("to_window", to.to_string()),
                    ],
                );
        }

        pub fn record_clipboard_operation(operation: &str, size_bytes: usize, duration_ms: u64) {
            global::meter("terminator-mcp")
                .f64_histogram("clipboard.operation_time")
                .with_description("Time taken for clipboard operations in milliseconds")
                .build()
                .record(
                    duration_ms as f64,
                    &[
                        KeyValue::new("operation", operation.to_string()),
                        KeyValue::new("size_bytes", size_bytes as i64),
                    ],
                );
        }

        pub fn record_screenshot(format: &str, size_bytes: usize, duration_ms: u64) {
            global::meter("terminator-mcp")
                .f64_histogram("screenshot.capture_time")
                .with_description("Time taken to capture screenshots in milliseconds")
                .build()
                .record(
                    duration_ms as f64,
                    &[
                        KeyValue::new("format", format.to_string()),
                        KeyValue::new("size_bytes", size_bytes as i64),
                    ],
                );
        }

        pub fn record_application_launch(app_name: &str, success: bool, launch_time_ms: u64) {
            global::meter("terminator-mcp")
                .f64_histogram("application.launch_time")
                .with_description("Time taken to launch applications in milliseconds")
                .build()
                .record(
                    launch_time_ms as f64,
                    &[
                        KeyValue::new("application", app_name.to_string()),
                        KeyValue::new("success", success),
                    ],
                );
        }

        pub fn record_ui_tree_extraction(node_count: usize, depth: usize, duration_ms: u64) {
            global::meter("terminator-mcp")
                .f64_histogram("ui_tree.extraction_time")
                .with_description("Time taken to extract UI tree in milliseconds")
                .build()
                .record(
                    duration_ms as f64,
                    &[
                        KeyValue::new("node_count", node_count as i64),
                        KeyValue::new("max_depth", depth as i64),
                    ],
                );
        }
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

        // Set up propagator
        global::set_text_map_propagator(TraceContextPropagator::new());

        // Configure OTLP endpoint
        let otlp_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:4318".to_string());

        info!("Initializing enhanced OpenTelemetry with endpoint: {}", otlp_endpoint);

        // Create resource with comprehensive metadata
        let resource = Resource::from_schema_url(
            [
                KeyValue::new(SERVICE_NAME, "terminator-mcp-agent"),
                KeyValue::new(SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
                KeyValue::new(
                    "deployment.environment",
                    std::env::var("ENVIRONMENT").unwrap_or_else(|_| "production".to_string()),
                ),
                KeyValue::new(
                    "host.name",
                    hostname::get()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                ),
                KeyValue::new("os.type", std::env::consts::OS),
                KeyValue::new("process.pid", std::process::id() as i64),
            ],
            SCHEMA_URL,
        );

        // Initialize tracing
        let trace_exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .with_endpoint(format!("{}/v1/traces", &otlp_endpoint))
            .with_timeout(Duration::from_secs(10))
            .build()?;

        let trace_provider = SdkTracerProvider::builder()
            .with_batch_exporter(trace_exporter, runtime::Tokio)
            .with_id_generator(RandomIdGenerator::default())
            .with_sampler(Sampler::AlwaysOn)
            .with_resource(resource.clone())
            .build();

        global::set_tracer_provider(trace_provider);

        // Initialize metrics
        let metrics_exporter = opentelemetry_otlp::MetricExporter::builder()
            .with_http()
            .with_endpoint(format!("{}/v1/metrics", &otlp_endpoint))
            .with_timeout(Duration::from_secs(10))
            .build()?;

        let reader = PeriodicReader::builder(metrics_exporter, runtime::Tokio)
            .with_interval(Duration::from_secs(30))
            .with_timeout(Duration::from_secs(10))
            .build();

        let meter_provider = MeterProviderBuilder::default()
            .with_resource(resource)
            .with_reader(reader)
            .build();

        global::set_meter_provider(meter_provider);

        info!("Enhanced OpenTelemetry telemetry initialized successfully with tracing and metrics");
        Ok(())
    }

    pub fn shutdown_telemetry() {
        info!("Shutting down OpenTelemetry providers");
        global::shutdown_tracer_provider();
    }
}

// Stub implementation when telemetry is disabled
#[cfg(not(feature = "telemetry"))]
mod without_telemetry {
    use tracing::debug;

    pub struct WorkflowSpan {
        workflow_id: String,
        correlation_id: String,
    }

    impl WorkflowSpan {
        pub fn new(_name: &str) -> Self {
            debug!("Telemetry disabled: WorkflowSpan created (no-op)");
            WorkflowSpan {
                workflow_id: uuid::Uuid::new_v4().to_string(),
                correlation_id: uuid::Uuid::new_v4().to_string(),
            }
        }

        pub fn add_event(&mut self, _name: &str, _attributes: Vec<(&str, String)>) {}
        pub fn set_attribute(&mut self, _key: &str, _value: String) {}
        pub fn set_status(&mut self, _success: bool, _message: &str) {}
        pub fn end(self) {}
        pub fn get_correlation_id(&self) -> &str { &self.correlation_id }
        pub fn get_workflow_id(&self) -> &str { &self.workflow_id }
    }

    pub struct StepSpan;

    impl StepSpan {
        pub fn new(_tool_name: &str, _step_id: Option<&str>) -> Self {
            debug!("Telemetry disabled: StepSpan created (no-op)");
            StepSpan
        }

        pub fn new_with_context(_tool_name: &str, _step_id: Option<&str>, _parent: Option<opentelemetry::Context>) -> Self {
            debug!("Telemetry disabled: StepSpan created (no-op)");
            StepSpan
        }

        pub fn set_attribute(&mut self, _key: &str, _value: String) {}
        pub fn add_event(&mut self, _name: &str, _attributes: Vec<(&str, String)>) {}
        pub fn record_retry(&mut self, _attempt: u32, _reason: &str) {}
        pub fn record_element_search(&mut self, _selector: &str, _found: bool, _search_time_ms: u64) {}
        pub fn set_status(&mut self, _success: bool, _error: Option<&str>) {}
        pub fn end(self) {}
    }

    pub struct AutomationMetrics;

    impl AutomationMetrics {
        pub fn record_window_change(_from: Option<&str>, _to: &str, _duration_ms: u64) {}
        pub fn record_clipboard_operation(_operation: &str, _size_bytes: usize, _duration_ms: u64) {}
        pub fn record_screenshot(_format: &str, _size_bytes: usize, _duration_ms: u64) {}
        pub fn record_application_launch(_app_name: &str, _success: bool, _launch_time_ms: u64) {}
        pub fn record_ui_tree_extraction(_node_count: usize, _depth: usize, _duration_ms: u64) {}
    }

    pub fn init_telemetry() -> anyhow::Result<()> {
        Ok(())
    }

    pub fn shutdown_telemetry() {
        debug!("Telemetry disabled: shutdown_telemetry (no-op)");
    }
}