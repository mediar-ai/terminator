# Terminator Observability

[![Crates.io](https://img.shields.io/crates/v/terminator-observability.svg)](https://crates.io/crates/terminator-observability)
[![Documentation](https://docs.rs/terminator-observability/badge.svg)](https://docs.rs/terminator-observability)
[![License](https://img.shields.io/crates/l/terminator-observability.svg)](LICENSE)
[![CI](https://github.com/yourusername/terminator/workflows/CI/badge.svg)](https://github.com/yourusername/terminator/actions)

> 🔍 **Comprehensive observability for Terminator SDK automation agents** - Track performance, compare with human baselines, and optimize your computer use automation.

## Features

- 🚀 **Near-zero overhead** - Typically <100μs per operation
- 📊 **OpenTelemetry native** - Export to Jaeger, Prometheus, Grafana
- 🎯 **Human baseline comparison** - Measure efficiency vs manual tasks
- 📈 **Real-time metrics** - Live performance dashboards
- 🔄 **Automatic retries tracking** - Understand failure patterns
- 🧠 **Smart insights** - ML-powered optimization suggestions
- 🔒 **Privacy-first** - Built-in PII redaction
- 🎨 **Beautiful traces** - Visualize automation flows

## Quick Start

```toml
[dependencies]
terminator-observability = "0.1"
```

```rust
use terminator_observability::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize observability
    let observability = TerminatorObservability::builder()
        .with_service_name("my-automation")
        .with_otlp_endpoint("http://localhost:4317")
        .build()?;

    // Wrap your desktop instance
    let desktop = observability.create_desktop()?;
    
    // Start a traced session
    let session = desktop.start_session("login_flow");
    
    // Your automation code - automatically traced!
    let app = desktop.open_application("MyApp").await?;
    let username_field = desktop.locator("name:Username").first().await?;
    username_field.type_text("user@example.com").await?;
    
    // Complete session and get performance report
    let report = session.complete();
    println!("Task completed in {:?}", report.duration);
    println!("Efficiency vs human: {:.2}x", report.efficiency_ratio);
    
    Ok(())
}
```

## Core Concepts

### Sessions
Sessions represent end-to-end automation tasks:

```rust
let session = desktop.start_session("process_invoices")
    .with_metadata("invoice_count", 50)
    .with_metadata("customer", "ACME Corp");

// ... automation code ...

let report = session.complete();
```

### Automatic Tracing
Every SDK operation is automatically traced:

```rust
// This creates a span with timing, success/failure, and metadata
let element = desktop.locator("button:Submit").first().await?;
element.click()?;  // Traced as "click_element" span
```

### Human Baseline Comparison
Compare your automation with human performance:

```rust
// Load baseline from previous recording
let baseline = HumanBaseline::load("login_flow.baseline")?;

let session = desktop.start_session("login_flow")
    .with_baseline(baseline);

// After completion, get comparison metrics
let report = session.complete();
if report.efficiency_ratio > 1.0 {
    println!("✅ Agent is {}% faster than human!", 
        (report.efficiency_ratio - 1.0) * 100.0);
}
```

### Custom Metrics
Track business-specific metrics:

```rust
observability.metrics()
    .counter("invoices_processed")
    .with_tags(&[("status", "success")])
    .increment(1);

observability.metrics()
    .histogram("processing_time_ms")
    .record(processing_time.as_millis() as f64);
```

## Architecture

### Observability Layers

```
┌─────────────────────────────────────┐
│        Your Automation Code         │
├─────────────────────────────────────┤
│      Observable Decorators          │  <- Transparent wrapper layer
├─────────────────────────────────────┤
│        Terminator SDK               │  <- Original SDK unchanged
├─────────────────────────────────────┤
│      Telemetry Pipeline             │  <- OpenTelemetry standards
├─────────────────────────────────────┤
│    Exporters (OTLP, Prometheus)    │  <- Your choice of backend
└─────────────────────────────────────┘
```

### Performance Impact

Minimal overhead by design:
- Async telemetry export (non-blocking)
- Sampling for high-frequency operations  
- Lock-free metrics collection
- Zero-allocation hot paths

## Advanced Usage

### Custom Span Attributes

```rust
desktop.with_span_processor(|span| {
    span.set_attribute("build_version", env!("CARGO_PKG_VERSION"));
    span.set_attribute("environment", "production");
});
```

### Error Tracking

```rust
// Automatic error capture with full context
match element.click() {
    Ok(_) => {},
    Err(e) => {
        // Error automatically recorded with:
        // - Full stack trace
        // - Element selector that failed
        // - Screenshot at time of failure
        // - Retry attempt number
    }
}
```

### Sampling Configuration

```rust
let observability = TerminatorObservability::builder()
    .with_trace_config(
        TraceConfig::default()
            .with_sampler(Sampler::TraceIdRatioBased(0.1)) // 10% sampling
    )
    .build()?;
```

### Export to Multiple Backends

```rust
let observability = TerminatorObservability::builder()
    // Traces to Jaeger
    .with_otlp_endpoint("http://jaeger:4317")
    // Metrics to Prometheus
    .with_prometheus_exporter(9464)
    // Custom exporter
    .with_span_exporter(MyCustomExporter::new())
    .build()?;
```

## Observability Best Practices

### 1. Meaningful Session Names
```rust
// ❌ Bad
desktop.start_session("task1")

// ✅ Good  
desktop.start_session("reconcile_bank_statement_march_2024")
```

### 2. Add Context
```rust
session.add_context("user_id", user.id);
session.add_context("batch_size", items.len());
session.add_context("retry_attempt", retry_count);
```

### 3. Track Business Metrics
```rust
// Beyond technical metrics, track what matters to your business
observability.metrics()
    .gauge("automation_roi_percentage")
    .set(calculate_roi());
```

### 4. Set SLOs
```rust
let slo = SLO::builder()
    .name("invoice_processing")
    .target_duration(Duration::from_secs(30))
    .success_rate(0.99)
    .build();

observability.add_slo(slo);
```

## Integration Examples

### With Grafana

```yaml
# docker-compose.yml
services:
  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_AUTH_ANONYMOUS_ENABLED=true
```

Import our [Grafana dashboard](dashboards/terminator-automation.json) for instant visibility.

### With Jaeger

```rust
let observability = TerminatorObservability::builder()
    .with_service_name("invoice-automation")
    .with_otlp_endpoint("http://jaeger:4317")
    .build()?;
```

### With Datadog

```rust
let observability = TerminatorObservability::builder()
    .with_datadog_agent("127.0.0.1:8126")
    .with_datadog_api_key(env::var("DD_API_KEY")?)
    .build()?;
```

## Performance Benchmarks

| Operation | Overhead | Memory |
|-----------|----------|---------|
| Click tracking | ~50μs | 128 bytes |
| Type text tracking | ~100μs | 256 bytes |
| Screenshot capture | ~200μs | 512 bytes |
| Session creation | ~500μs | 2KB |

See [benchmarks/](benchmarks/) for detailed performance analysis.

## Debugging

Enable debug logging:
```bash
RUST_LOG=terminator_observability=debug cargo run
```

View real-time traces:
```bash
# Start local Jaeger
docker run -p 16686:16686 -p 4317:4317 jaegertracing/all-in-one

# View traces at http://localhost:16686
```

## Examples

- [Basic Usage](examples/basic.rs) - Simple automation with tracing
- [Custom Handlers](examples/custom_handlers.rs) - Building custom telemetry processors
- [Human Baseline](examples/human_baseline.rs) - Recording and comparing with human performance
- [Dashboard Integration](examples/dashboard.rs) - Real-time monitoring setup
- [Error Recovery](examples/error_recovery.rs) - Tracking retry strategies

## Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Development Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/terminator
cd terminator/terminator-observability

# Install dependencies
cargo build

# Run tests
cargo test

# Run benchmarks
cargo bench

# Check formatting
cargo fmt -- --check

# Run lints
cargo clippy -- -D warnings
```

## License

This project is licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Acknowledgments

Built on top of these excellent projects:
- [OpenTelemetry](https://opentelemetry.io/) for standardized observability
- [Tokio](https://tokio.rs/) for async runtime
- [Terminator SDK](https://github.com/yourusername/terminator) for automation capabilities

---

<p align="center">Made with ❤️ by the Terminator community</p>