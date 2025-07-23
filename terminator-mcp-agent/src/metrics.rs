//! Prometheus metrics module for MCP server
//! 
//! This module provides comprehensive metrics collection for the MCP server,
//! including tool usage, execution times, HTTP request metrics, and error tracking.

#[cfg(feature = "metrics")]
use prometheus::{
    Counter, CounterVec, Histogram, HistogramVec, Gauge, Registry, Encoder, TextEncoder,
    register_counter_vec_with_registry, register_histogram_vec_with_registry,
    register_counter_with_registry, register_histogram_with_registry,
    register_gauge_with_registry,
};

#[cfg(feature = "metrics")]
use std::sync::Arc;

#[cfg(feature = "metrics")]
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};

#[cfg(feature = "metrics")]
use tower::ServiceBuilder;

#[cfg(feature = "metrics")]
lazy_static::lazy_static! {
    /// Global metrics registry
    static ref REGISTRY: Registry = Registry::new();
    
    /// Counter for total tool calls
    static ref TOOL_CALLS_TOTAL: CounterVec = register_counter_vec_with_registry!(
        "mcp_tool_calls_total",
        "Total number of tool calls made",
        &["tool_name", "status"],
        REGISTRY
    ).unwrap();
    
    /// Histogram for tool execution duration
    static ref TOOL_EXECUTION_DURATION: HistogramVec = register_histogram_vec_with_registry!(
        "mcp_tool_execution_duration_seconds",
        "Time taken to execute tools",
        &["tool_name"],
        prometheus::exponential_buckets(0.001, 2.0, 20).unwrap(),
        REGISTRY
    ).unwrap();
    
    /// Counter for HTTP requests
    static ref HTTP_REQUESTS_TOTAL: CounterVec = register_counter_vec_with_registry!(
        "mcp_http_requests_total",
        "Total HTTP requests received",
        &["method", "path", "status"],
        REGISTRY
    ).unwrap();
    
    /// Histogram for HTTP request duration
    static ref HTTP_REQUEST_DURATION: HistogramVec = register_histogram_vec_with_registry!(
        "mcp_http_request_duration_seconds",
        "HTTP request processing time",
        &["method", "path"],
        prometheus::exponential_buckets(0.001, 2.0, 20).unwrap(),
        REGISTRY
    ).unwrap();
    
    /// Counter for errors by type
    static ref ERRORS_TOTAL: CounterVec = register_counter_vec_with_registry!(
        "mcp_errors_total",
        "Total errors by type and component",
        &["error_type", "component"],
        REGISTRY
    ).unwrap();
    
    /// Counter for server restarts/initializations
    static ref SERVER_STARTS_TOTAL: Counter = register_counter_with_registry!(
        "mcp_server_starts_total",
        "Total number of server starts",
        REGISTRY
    ).unwrap();
    
    /// Histogram for connection duration (for non-stdio transports)
    static ref CONNECTION_DURATION: Histogram = register_histogram_with_registry!(
        "mcp_connection_duration_seconds",
        "Duration of client connections",
        prometheus::exponential_buckets(1.0, 2.0, 20).unwrap(),
        REGISTRY
    ).unwrap();
    
    /// Gauge for active connections
    static ref ACTIVE_CONNECTIONS: Gauge = register_gauge_with_registry!(
        "mcp_active_connections",
        "Number of currently active connections",
        REGISTRY
    ).unwrap();
}

/// Metrics collector struct
#[cfg(feature = "metrics")]
pub struct Metrics {
    registry: Arc<Registry>,
}

#[cfg(feature = "metrics")]
impl Metrics {
    /// Create a new metrics instance
    pub fn new() -> Self {
        // Initialize server start counter
        SERVER_STARTS_TOTAL.inc();
        
        Self {
            registry: Arc::new(REGISTRY.clone()),
        }
    }
    
    /// Record a tool call start
    pub fn tool_call_start(&self, tool_name: &str) -> ToolCallMetrics {
        ToolCallMetrics {
            tool_name: tool_name.to_string(),
            start_time: std::time::Instant::now(),
        }
    }
    
    /// Record an HTTP request start
    pub fn http_request_start(&self, method: &str, path: &str) -> HttpRequestMetrics {
        HttpRequestMetrics {
            method: method.to_string(),
            path: path.to_string(),
            start_time: std::time::Instant::now(),
        }
    }
    
    /// Record an error
    pub fn record_error(&self, error_type: &str, component: &str) {
        ERRORS_TOTAL.with_label_values(&[error_type, component]).inc();
    }
    
    /// Increment active connections
    pub fn connection_opened(&self) {
        ACTIVE_CONNECTIONS.inc();
    }
    
    /// Decrement active connections and record connection duration
    pub fn connection_closed(&self, duration: std::time::Duration) {
        ACTIVE_CONNECTIONS.dec();
        CONNECTION_DURATION.observe(duration.as_secs_f64());
    }
    
    /// Get metrics as Prometheus text format
    pub fn render(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer)?;
        Ok(String::from_utf8(buffer)?)
    }
}

/// Metrics for tracking tool call execution
#[cfg(feature = "metrics")]
pub struct ToolCallMetrics {
    tool_name: String,
    start_time: std::time::Instant,
}

#[cfg(feature = "metrics")]
impl ToolCallMetrics {
    /// Mark the tool call as completed successfully
    pub fn success(self) {
        let duration = self.start_time.elapsed();
        TOOL_CALLS_TOTAL.with_label_values(&[&self.tool_name, "success"]).inc();
        TOOL_EXECUTION_DURATION.with_label_values(&[&self.tool_name]).observe(duration.as_secs_f64());
    }
    
    /// Mark the tool call as failed
    pub fn error(self, error_type: &str) {
        let duration = self.start_time.elapsed();
        TOOL_CALLS_TOTAL.with_label_values(&[&self.tool_name, "error"]).inc();
        TOOL_EXECUTION_DURATION.with_label_values(&[&self.tool_name]).observe(duration.as_secs_f64());
        ERRORS_TOTAL.with_label_values(&[error_type, "tool_execution"]).inc();
    }
}

/// Metrics for tracking HTTP request processing
#[cfg(feature = "metrics")]
pub struct HttpRequestMetrics {
    method: String,
    path: String,
    start_time: std::time::Instant,
}

#[cfg(feature = "metrics")]
impl HttpRequestMetrics {
    /// Mark the HTTP request as completed
    pub fn complete(self, status_code: u16) {
        let duration = self.start_time.elapsed();
        let status = status_code.to_string();
        
        HTTP_REQUESTS_TOTAL.with_label_values(&[&self.method, &self.path, &status]).inc();
        HTTP_REQUEST_DURATION.with_label_values(&[&self.method, &self.path]).observe(duration.as_secs_f64());
        
        // Record errors for 4xx and 5xx status codes
        if status_code >= 400 {
            let error_type = if status_code >= 500 { "server_error" } else { "client_error" };
            ERRORS_TOTAL.with_label_values(&[error_type, "http"]).inc();
        }
    }
}

/// HTTP handler for metrics endpoint
#[cfg(feature = "metrics")]
pub async fn metrics_handler(State(metrics): State<Arc<Metrics>>) -> Response {
    match metrics.render() {
        Ok(metrics_text) => (
            StatusCode::OK,
            [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
            metrics_text,
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error generating metrics: {}", e),
        ).into_response(),
    }
}

/// Middleware for HTTP metrics collection
#[cfg(feature = "metrics")]
pub fn metrics_middleware(
    metrics: Arc<Metrics>,
) -> impl tower::Layer<axum::Router> + Clone {
    tower::ServiceBuilder::new().layer(tower::layer::layer_fn(move |inner| {
        let metrics = metrics.clone();
        tower::service_fn(move |request: axum::http::Request<axum::body::Body>| {
            let metrics = metrics.clone();
            let inner = inner.clone();
            async move {
                let method = request.method().to_string();
                let path = request.uri().path().to_string();
                let tracker = metrics.http_request_start(&method, &path);
                
                let response = inner.call(request).await?;
                tracker.complete(response.status().as_u16());
                
                Ok(response)
            }
        })
    }))
}

/// No-op implementations when metrics feature is disabled
#[cfg(not(feature = "metrics"))]
pub struct Metrics;

#[cfg(not(feature = "metrics"))]
impl Metrics {
    pub fn new() -> Self {
        Self
    }
    
    pub fn tool_call_start(&self, _tool_name: &str) -> ToolCallMetrics {
        ToolCallMetrics
    }
    
    pub fn http_request_start(&self, _method: &str, _path: &str) -> HttpRequestMetrics {
        HttpRequestMetrics
    }
    
    pub fn record_error(&self, _error_type: &str, _component: &str) {}
    
    pub fn connection_opened(&self) {}
    
    pub fn connection_closed(&self, _duration: std::time::Duration) {}
}

#[cfg(not(feature = "metrics"))]
pub struct ToolCallMetrics;

#[cfg(not(feature = "metrics"))]
impl ToolCallMetrics {
    pub fn success(self) {}
    pub fn error(self, _error_type: &str) {}
}

#[cfg(not(feature = "metrics"))]
pub struct HttpRequestMetrics;

#[cfg(not(feature = "metrics"))]
impl HttpRequestMetrics {
    pub fn complete(self, _status_code: u16) {}
}

#[cfg(not(feature = "metrics"))]
pub async fn metrics_handler() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "Metrics not enabled")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "metrics")]
    #[test]
    fn test_metrics_creation() {
        let metrics = Metrics::new();
        assert!(metrics.registry.gather().len() > 0);
    }

    #[cfg(feature = "metrics")]
    #[test]
    fn test_tool_call_tracking() {
        let metrics = Metrics::new();
        let tracker = metrics.tool_call_start("test_tool");
        
        // Test successful completion
        tracker.success();
        
        // Verify metrics were recorded
        let metric_families = metrics.registry.gather();
        let tool_calls_metric = metric_families
            .iter()
            .find(|mf| mf.get_name() == "mcp_tool_calls_total");
        assert!(tool_calls_metric.is_some());
    }

    #[cfg(feature = "metrics")]
    #[test]
    fn test_tool_call_error_tracking() {
        let metrics = Metrics::new();
        let tracker = metrics.tool_call_start("test_tool");
        
        // Test error completion
        tracker.error("test_error");
        
        // Verify metrics were recorded
        let metric_families = metrics.registry.gather();
        let tool_calls_metric = metric_families
            .iter()
            .find(|mf| mf.get_name() == "mcp_tool_calls_total");
        assert!(tool_calls_metric.is_some());
    }

    #[cfg(feature = "metrics")]
    #[test]
    fn test_http_request_tracking() {
        let metrics = Metrics::new();
        let tracker = metrics.http_request_start("GET", "/test");
        
        tracker.finish(200);
        
        // Verify metrics were recorded
        let metric_families = metrics.registry.gather();
        let http_requests_metric = metric_families
            .iter()
            .find(|mf| mf.get_name() == "mcp_http_requests_total");
        assert!(http_requests_metric.is_some());
    }

    #[cfg(feature = "metrics")]
    #[test]
    fn test_prometheus_export() {
        let metrics = Metrics::new();
        
        // Record some sample metrics
        let tool_tracker = metrics.tool_call_start("sample_tool");
        tool_tracker.success();
        
        let http_tracker = metrics.http_request_start("POST", "/mcp");
        http_tracker.finish(200);
        
        // Test that we can export metrics
        let metric_families = metrics.registry.gather();
        assert!(metric_families.len() > 0);
        
        // Test text format encoding
        let encoder = TextEncoder::new();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        let metrics_text = String::from_utf8(buffer).unwrap();
        
        assert!(metrics_text.contains("mcp_tool_calls_total"));
        assert!(metrics_text.contains("mcp_http_requests_total"));
    }
}