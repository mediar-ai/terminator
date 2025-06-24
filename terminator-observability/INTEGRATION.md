# Terminator Observability Integration Guide

This guide will help you integrate observability into your existing Terminator automation projects.

## Quick Start

### 1. Add Dependency

Add to your `Cargo.toml`:

```toml
[dependencies]
terminator-observability = "0.1"
```

### 2. Basic Integration

Replace your existing Desktop initialization:

```rust
// Before:
let desktop = Desktop::new_default()?;

// After:
use terminator_observability::prelude::*;

let observability = TerminatorObservability::builder()
    .with_service_name("my-automation")
    .build()?;
    
let desktop = observability.create_desktop()?;
```

### 3. Wrap Existing Code

Your existing automation code works without changes:

```rust
// This code remains the same!
let app = desktop.open_application("MyApp").await?;
let button = desktop.locator("name:Submit").first().await?;
button.click().await?;
```

## Configuration Options

### OpenTelemetry Export

Export traces to Jaeger, Zipkin, or other OTLP-compatible backends:

```rust
let observability = TerminatorObservability::builder()
    .with_service_name("invoice-processor")
    .with_otlp_endpoint("http://localhost:4317")
    .build()?;
```

### Sampling

Reduce overhead by sampling traces:

```rust
let observability = TerminatorObservability::builder()
    .with_sampling_ratio(0.1) // Sample 10% of traces
    .build()?;
```

### Debug Mode

Enable console output for debugging:

```rust
let observability = TerminatorObservability::builder()
    .with_stdout_exporter(true)
    .build()?;
```

## Session Management

### Basic Sessions

Track end-to-end automation tasks:

```rust
let mut desktop = observability.create_desktop()?;
let session = desktop.start_session("process_invoices");

// Your automation code...

let report = session.complete();
println!("Task completed in {:?}", report.duration);
```

### Session Metadata

Add context to your sessions:

```rust
let session = desktop.start_session("data_entry");
session.add_metadata("form_type", "customer_registration");
session.add_metadata("record_count", 50);
session.add_metadata("environment", "production");
```

## Metrics Collection

### Built-in Metrics

The following metrics are automatically collected:

- `terminator.*.duration` - Operation durations
- `terminator.*.count` - Operation counts
- `terminator.session.duration` - Session durations
- `terminator.typing.chars_per_second` - Typing speed

### Custom Metrics

Add your own business metrics:

```rust
// Count processed items
observability.metrics().increment(
    "invoices.processed",
    &[("status", "success"), ("customer", "acme")]
);

// Record processing time
observability.metrics().record(
    "invoice.processing_time_ms",
    processing_time.as_millis() as f64,
    &[("type", "standard")]
);

// Set current queue size
observability.metrics().gauge(
    "processing.queue_size",
    queue.len() as f64,
    &[]
);
```

## Human Baseline Comparison

### Recording Baselines

Create baselines from human performance data:

```rust
use terminator_observability::HumanBaseline;

let baseline = HumanBaseline::new(
    "login_flow".to_string(),
    Duration::from_secs(30), // Human average
);

baseline.save("baselines/login_flow.json")?;
```

### Using Baselines

Compare automation performance:

```rust
let baseline = HumanBaseline::load("baselines/login_flow.json")?;

// Use in session - requires modifying the session after creation
// (See examples for detailed implementation)
```

## Integration with CI/CD

### GitHub Actions Example

```yaml
- name: Run Automation with Observability
  env:
    TERMINATOR_OBSERVABILITY_ENDPOINT: ${{ secrets.OTLP_ENDPOINT }}
  run: |
    cargo run --release
    
- name: Check Performance
  run: |
    # Parse the session report
    jq '.efficiency_ratio' session_report.json
```

### Performance Gates

Fail builds if performance degrades:

```rust
let report = session.complete();

if report.efficiency_ratio < 0.8 {
    eprintln!("Performance degraded: {} slower than baseline", 
        report.improvement_percentage().abs());
    std::process::exit(1);
}
```

## Production Best Practices

### 1. Environment-based Configuration

```rust
let observability = TerminatorObservability::builder()
    .with_service_name(&env::var("SERVICE_NAME")?)
    .with_otlp_endpoint(&env::var("OTLP_ENDPOINT").ok())
    .with_sampling_ratio(
        env::var("SAMPLING_RATIO")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.1)
    )
    .build()?;
```

### 2. Error Handling

The observability layer handles errors gracefully:

```rust
// If observability fails, automation continues
let desktop = match TerminatorObservability::builder().build() {
    Ok(obs) => obs.create_desktop()?,
    Err(e) => {
        eprintln!("Observability disabled: {}", e);
        Desktop::new_default()?
    }
};
```

### 3. Resource Management

Always shutdown properly:

```rust
// Ensure data is flushed
observability.shutdown().await?;
```

### 4. Sensitive Data

Never log sensitive information:

```rust
// Bad
session.add_metadata("password", user_password);

// Good
session.add_metadata("user_id", user_id);
session.add_metadata("has_password", true);
```

## Troubleshooting

### No Data Appearing

1. Check OTLP endpoint is accessible
2. Verify sampling ratio isn't 0
3. Enable debug logging: `RUST_LOG=terminator_observability=debug`
4. Use stdout exporter to verify data generation

### High Overhead

1. Reduce sampling ratio
2. Disable debug exporters in production
3. Use batched export (default)
4. Check network latency to OTLP endpoint

### Memory Usage

1. Traces are kept in memory until exported
2. Configure shorter export intervals if needed
3. Monitor with: `observability.metrics().snapshot()`

## Example Projects

See the `examples/` directory for complete examples:

- `basic.rs` - Simple integration
- `human_baseline.rs` - Performance comparison
- `custom_handlers.rs` - Advanced customization

## Support

- GitHub Issues: Report bugs
- Discussions: Feature requests
- Discord: Community chat