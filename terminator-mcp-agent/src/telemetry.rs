#![allow(dead_code)]

use anyhow::Result;
use once_cell::sync::OnceCell;
use std::time::Duration;
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};

#[cfg(feature = "telemetry")]
use {
    metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle},
    opentelemetry::{global, KeyValue},
    opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge,
    opentelemetry_otlp::WithExportConfig,
    tracing_opentelemetry::OpenTelemetryLayer,
};

#[cfg(feature = "telemetry")]
static PROM_HANDLE: OnceCell<PrometheusHandle> = OnceCell::new();

#[cfg(feature = "telemetry")]
fn init_prometheus_recorder() -> Option<PrometheusHandle> {
    // Install Prometheus recorder for the `metrics` crate
    let builder = PrometheusBuilder::new();
    match builder.install_recorder() {
        Ok(handle) => Some(handle),
        Err(_e) => None,
    }
}

#[cfg(feature = "telemetry")]
fn build_otel_layer() -> Option<OpenTelemetryLayer<Registry, opentelemetry::trace::Tracer>> {
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .ok()
        .unwrap_or_else(|| "http://127.0.0.1:4317".to_string());

    let service_name = std::env::var("OTEL_SERVICE_NAME")
        .ok()
        .unwrap_or_else(|| "terminator-mcp-agent".to_string());

    // Resource with common attributes
    let resource = opentelemetry::sdk::Resource::new(vec![
        KeyValue::new("service.name", service_name),
        KeyValue::new("service.namespace", "terminator"),
        KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
    ]);

    // Build tracer provider with OTLP exporter (gRPC)
    let tracer_provider = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(endpoint),
        )
        .with_trace_config(
            opentelemetry::sdk::trace::Config::default().with_resource(resource),
        )
        .install_batch(opentelemetry::runtime::Tokio)
        .ok()?;

    Some(tracing_opentelemetry::layer().with_tracer(tracer_provider.tracer("mcp")))
}

#[cfg(feature = "telemetry")]
fn build_otel_log_layer() -> Option<OpenTelemetryTracingBridge> {
    // Install OTLP log exporter/provider so the tracing bridge has a sink
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .ok()
        .unwrap_or_else(|| "http://127.0.0.1:4317".to_string());

    let _logger_provider = opentelemetry_otlp::new_pipeline()
        .logging()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(endpoint),
        )
        .install_simple()
        .ok()?;

    // Bridge tracing events to OpenTelemetry logs (if supported by collector)
    Some(OpenTelemetryTracingBridge::new())
}

/// Initialize tracing/logging (with OpenTelemetry, Prometheus) under the `telemetry` feature.
/// Falls back to plain fmt logging if OpenTelemetry setup fails.
pub fn init_observability(log_level: tracing::Level) -> Result<()> {
    #[cfg(feature = "telemetry")]
    {
        let env_filter = EnvFilter::from_default_env().add_directive(log_level.into());

        // Build layers: fmt + otel tracing + otel logs
        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_ansi(false);

        let otel_layer = build_otel_layer();
        let otel_log_layer = build_otel_log_layer();

        // Install Prometheus recorder and keep a handle for /metrics route
        if let Some(handle) = init_prometheus_recorder() {
            let _ = PROM_HANDLE.set(handle);
        }

        let registry = Registry::default().with(env_filter).with(fmt_layer);
        let registry = if let Some(layer) = otel_layer { registry.with(layer) } else { registry };
        let registry = if let Some(layer) = otel_log_layer { registry.with(layer) } else { registry };

        registry.init();

        // Ensure provider shutdown on exit (best-effort)
        tokio::spawn(async move {
            // Give exporter time to flush on shutdown
            tokio::signal::ctrl_c().await.ok();
            global::shutdown_tracer_provider();
            // Not all versions expose shutdown for logger; best-effort flush is handled by exporter drops
        });

        return Ok(());
    }

    // If telemetry feature is disabled, set up plain fmt logging here as a fallback helper.
    let env_filter = EnvFilter::from_default_env().add_directive(log_level.into());
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();
    Ok(())
}

#[cfg(feature = "telemetry")]
pub fn prometheus_scrape() -> Option<String> {
    PROM_HANDLE.get().map(|h| h.render())
}

#[cfg(not(feature = "telemetry"))]
pub fn prometheus_scrape() -> Option<String> { None }

/// Record a tool invocation metric and annotate current tracing span.
pub fn record_tool_outcome(tool_name: &str, status: &str, duration_ms: i64) {
    // Add fields to current span when present
    tracing::Span::current().record("tool", &tool_name);
    tracing::Span::current().record("status", &status);
    tracing::Span::current().record("duration_ms", &duration_ms);

    #[cfg(feature = "telemetry")]
    {
        use metrics::{counter, histogram};
        counter!("mcp_tool_invocations_total", "tool" => tool_name.to_string(), "status" => status.to_string()).increment(1);
        histogram!("mcp_tool_duration_ms", "tool" => tool_name.to_string(), "status" => status.to_string()).record(duration_ms as f64);
    }
}

/// Record a generic request metric (e.g., HTTP MCP request).
pub fn record_request_metrics(route: &str, method: &str, status_code: u16, duration: Duration) {
    #[cfg(feature = "telemetry")]
    {
        use metrics::{counter, histogram};
        counter!("mcp_http_requests_total", "route" => route.to_string(), "method" => method.to_string(), "status" => status_code.to_string()).increment(1);
        histogram!("mcp_http_request_duration_ms", "route" => route.to_string(), "method" => method.to_string(), "status" => status_code.to_string())
            .record(duration.as_millis() as f64);
    }
}

