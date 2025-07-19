# Prometheus Metrics for Terminator MCP Server

The Terminator MCP Server includes comprehensive Prometheus metrics support behind a feature flag. This allows you to monitor tool usage, performance, errors, and HTTP request patterns.

## Features

- **Tool Usage Tracking**: Monitor which tools are being called and how often
- **Performance Metrics**: Track execution times for tools and HTTP requests  
- **Error Monitoring**: Count and categorize different types of errors
- **HTTP Request Metrics**: Monitor request rates, response times, and status codes
- **Connection Monitoring**: Track active connections and their durations

## Enabling Metrics

### 1. Build with Metrics Feature

```bash
cargo build --features metrics
```

### 2. Run with Metrics Enabled

```bash
# HTTP transport (recommended for metrics)
cargo run --features metrics -- --transport http --enable-metrics

# Other transports also support metrics but without HTTP endpoint
cargo run --features metrics -- --transport stdio --enable-metrics
```

### 3. Access Metrics Endpoint

When using HTTP transport with metrics enabled:

- **MCP Server**: `http://127.0.0.1:3000/mcp`
- **Health Check**: `http://127.0.0.1:3000/health`  
- **Metrics**: `http://127.0.0.1:3000/metrics`

## Available Metrics

### Tool Metrics

| Metric | Type | Description | Labels |
|--------|------|-------------|--------|
| `mcp_tool_calls_total` | Counter | Total number of tool calls | `tool_name`, `status` |
| `mcp_tool_execution_duration_seconds` | Histogram | Tool execution time | `tool_name` |

### HTTP Metrics

| Metric | Type | Description | Labels |
|--------|------|-------------|--------|
| `mcp_http_requests_total` | Counter | Total HTTP requests | `method`, `path`, `status` |
| `mcp_http_request_duration_seconds` | Histogram | HTTP request processing time | `method`, `path` |

### Error Metrics

| Metric | Type | Description | Labels |
|--------|------|-------------|--------|
| `mcp_errors_total` | Counter | Total errors by type | `error_type`, `component` |

### Server Metrics

| Metric | Type | Description | Labels |
|--------|------|-------------|--------|
| `mcp_server_starts_total` | Counter | Server restart counter | None |
| `mcp_active_connections` | Gauge | Currently active connections | None |
| `mcp_connection_duration_seconds` | Histogram | Connection duration | None |

## Prometheus Configuration

Add this to your `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'terminator-mcp'
    static_configs:
      - targets: ['localhost:3000']
    metrics_path: '/metrics'
    scrape_interval: 15s
```

## Example Queries

### Tool Usage Analysis

```promql
# Tool call rate (calls per second)
rate(mcp_tool_calls_total[5m])

# Most popular tools
topk(10, sum by (tool_name) (rate(mcp_tool_calls_total[5m])))

# Tool error rate
rate(mcp_tool_calls_total{status="error"}[5m]) / rate(mcp_tool_calls_total[5m])
```

### Performance Monitoring

```promql
# Average tool execution time
rate(mcp_tool_execution_duration_seconds_sum[5m]) / rate(mcp_tool_execution_duration_seconds_count[5m])

# P95 tool execution time
histogram_quantile(0.95, rate(mcp_tool_execution_duration_seconds_bucket[5m]))

# Slowest tools
topk(10, histogram_quantile(0.95, rate(mcp_tool_execution_duration_seconds_bucket[5m])) by (tool_name))
```

### HTTP Monitoring  

```promql
# HTTP request rate
rate(mcp_http_requests_total[5m])

# HTTP error rate (4xx + 5xx)
rate(mcp_http_requests_total{status=~"4.."}[5m]) + rate(mcp_http_requests_total{status=~"5.."}[5m])

# P95 HTTP response time
histogram_quantile(0.95, rate(mcp_http_request_duration_seconds_bucket[5m]))
```

### Error Analysis

```promql
# Error rate by component
rate(mcp_errors_total[5m]) by (component)

# Error distribution by type
sum by (error_type) (rate(mcp_errors_total[5m]))
```

## Grafana Dashboard

Here's a sample Grafana dashboard configuration:

```json
{
  "dashboard": {
    "title": "Terminator MCP Server",
    "panels": [
      {
        "title": "Tool Call Rate",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(mcp_tool_calls_total[5m])",
            "legendFormat": "{{tool_name}} ({{status}})"
          }
        ]
      },
      {
        "title": "Tool Execution Time (P95)",
        "type": "graph", 
        "targets": [
          {
            "expr": "histogram_quantile(0.95, rate(mcp_tool_execution_duration_seconds_bucket[5m])) by (tool_name)",
            "legendFormat": "{{tool_name}}"
          }
        ]
      },
      {
        "title": "HTTP Requests",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(mcp_http_requests_total[5m])",
            "legendFormat": "{{method}} {{path}} ({{status}})"
          }
        ]
      },
      {
        "title": "Active Connections",
        "type": "singlestat",
        "targets": [
          {
            "expr": "mcp_active_connections"
          }
        ]
      }
    ]
  }
}
```

## Performance Impact

The metrics collection is designed to have minimal performance impact:

- **When disabled**: Zero overhead - all metrics code is compiled out
- **When enabled**: Microsecond-level overhead per tool call/HTTP request
- **Memory usage**: Approximately 1-5MB additional memory for metrics storage

## Troubleshooting

### Metrics Not Available

1. Ensure you built with the `metrics` feature:
   ```bash
   cargo build --features metrics
   ```

2. Check that you're using HTTP transport:
   ```bash
   cargo run --features metrics -- --transport http --enable-metrics
   ```

3. Verify the metrics endpoint is accessible:
   ```bash
   curl http://localhost:3000/metrics
   ```

### Missing Metrics

- Tool metrics are only recorded when tools are actually called
- HTTP metrics require using the HTTP transport mode
- Error metrics are only recorded when errors occur

### High Memory Usage

If metrics memory usage becomes problematic:

1. Reduce Prometheus scrape frequency
2. Use metric relabeling to drop unnecessary labels
3. Consider implementing metric rotation (custom implementation)

## Development

### Adding New Metrics

1. Define the metric in `src/metrics.rs`:
   ```rust
   static ref MY_METRIC: CounterVec = register_counter_vec_with_registry!(
       "mcp_my_metric_total",
       "Description of my metric", 
       &["label1", "label2"],
       REGISTRY
   ).unwrap();
   ```

2. Add instrumentation where needed:
   ```rust
   MY_METRIC.with_label_values(&["value1", "value2"]).inc();
   ```

### Testing Metrics

```bash
# Run the example
cargo run --example metrics-demo

# Build and run with metrics
cargo run --features metrics -- --transport http --enable-metrics

# Test metrics endpoint
curl http://localhost:3000/metrics
```