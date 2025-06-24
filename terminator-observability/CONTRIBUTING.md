# Contributing to Terminator Observability

Thank you for your interest in contributing to Terminator Observability! This guide will help you get started.

## Development Setup

### Prerequisites
- Rust 1.70+ (check with `rustc --version`)
- Cargo installed
- Git

### Getting Started

1. Clone the repository:
```bash
git clone https://github.com/yourusername/terminator
cd terminator/terminator-observability
```

2. Build the project:
```bash
cargo build
```

3. Run tests:
```bash
cargo test
```

4. Run examples:
```bash
cargo run --example basic
cargo run --example human_baseline
cargo run --example custom_handlers
```

## Code Style

We follow standard Rust conventions:

### Formatting
Always run `cargo fmt` before committing:
```bash
cargo fmt --all
```

### Linting
Fix all Clippy warnings:
```bash
cargo clippy -- -D warnings
```

### Documentation
- All public APIs must have doc comments
- Include examples in doc comments when appropriate
- Run `cargo doc --open` to preview documentation

## Architecture Guidelines

### Module Organization
- `lib.rs` - Main entry point and public API
- `context.rs` - Core observability context
- `decorator.rs` - Wrapper types for SDK integration
- `telemetry.rs` - OpenTelemetry integration
- `metrics.rs` - Metrics collection
- `session.rs` - Session management
- `trace.rs` - Trace representation
- `error.rs` - Error types

### Design Principles
1. **Zero-cost abstractions** - Minimize performance overhead
2. **Ergonomic API** - Easy to use, hard to misuse
3. **Standards-based** - Use OpenTelemetry standards
4. **Extensible** - Allow custom handlers and processors

## Testing

### Unit Tests
Place unit tests in the same file as the code:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_something() {
        // Test implementation
    }
}
```

### Integration Tests
Create integration tests in `tests/` directory:
```bash
cargo test --test integration_test_name
```

### Benchmarks
Add benchmarks for performance-critical paths:
```bash
cargo bench
```

## Pull Request Process

1. **Fork** the repository
2. **Create a feature branch**: `git checkout -b feature/amazing-feature`
3. **Make your changes**
4. **Add tests** for new functionality
5. **Update documentation** as needed
6. **Run all checks**:
   ```bash
   cargo fmt --all -- --check
   cargo clippy -- -D warnings
   cargo test
   cargo doc --no-deps
   ```
7. **Commit** with descriptive message
8. **Push** to your fork
9. **Open a Pull Request**

### PR Requirements
- ✅ All tests pass
- ✅ No Clippy warnings
- ✅ Code is formatted
- ✅ Documentation updated
- ✅ Changelog entry added (if applicable)
- ✅ Examples work

## Commit Messages

Follow conventional commits:
```
feat: add human baseline comparison
fix: correct span duration calculation
docs: update README examples
test: add metrics collector tests
refactor: simplify decorator pattern
perf: optimize trace serialization
```

## Feature Development

### Adding a New Metric
1. Define the metric in `metrics.rs`
2. Add collection point in relevant decorator
3. Update documentation
4. Add test coverage
5. Update examples if needed

### Adding a New Storage Backend
1. Create new module in `storage/`
2. Implement `TraceStore` trait
3. Add feature flag in `Cargo.toml`
4. Document configuration
5. Add integration test

## Performance Considerations

- Use `Arc` for shared ownership
- Prefer `&str` over `String` in APIs
- Use `Cow` for flexible string handling
- Minimize allocations in hot paths
- Benchmark before optimizing

## Security

- Never log sensitive data (passwords, keys)
- Implement PII redaction
- Validate all inputs
- Use secure defaults

## Questions?

- Open an issue for bugs
- Start a discussion for features
- Join our Discord for chat

Thank you for contributing! 🎉