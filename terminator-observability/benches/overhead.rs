//! Benchmarks for measuring observability overhead

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use terminator_observability::prelude::*;
use std::time::Duration;

fn benchmark_decorator_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("decorator_overhead");
    
    // Setup
    let runtime = tokio::runtime::Runtime::new().unwrap();
    
    // Benchmark without observability
    group.bench_function("baseline_click", |b| {
        b.to_async(&runtime).iter(|| async {
            let desktop = terminator::Desktop::new_default().unwrap();
            let element = create_mock_element();
            black_box(element.click());
        });
    });
    
    // Benchmark with observability
    group.bench_function("observable_click", |b| {
        b.to_async(&runtime).iter(|| async {
            let observability = TerminatorObservability::builder()
                .with_sampling_ratio(1.0)
                .build()
                .unwrap();
            let desktop = observability.create_desktop().unwrap();
            let element = create_mock_observable_element(&observability);
            black_box(element.click().await);
        });
    });
    
    group.finish();
}

fn benchmark_session_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("session_overhead");
    
    let runtime = tokio::runtime::Runtime::new().unwrap();
    
    // Benchmark session creation
    group.bench_function("session_creation", |b| {
        b.to_async(&runtime).iter(|| async {
            let observability = TerminatorObservability::builder().build().unwrap();
            let mut desktop = observability.create_desktop().unwrap();
            let session = black_box(desktop.start_session("benchmark_task"));
            drop(session);
        });
    });
    
    // Benchmark session with metadata
    group.bench_function("session_with_metadata", |b| {
        b.to_async(&runtime).iter(|| async {
            let observability = TerminatorObservability::builder().build().unwrap();
            let mut desktop = observability.create_desktop().unwrap();
            let session = desktop.start_session("benchmark_task");
            
            for i in 0..10 {
                session.add_metadata(format!("key_{}", i), format!("value_{}", i));
            }
            
            black_box(session);
        });
    });
    
    group.finish();
}

fn benchmark_metrics_collection(c: &mut Criterion) {
    let mut group = c.benchmark_group("metrics_collection");
    
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let observability = runtime.block_on(async {
        TerminatorObservability::builder().build().unwrap()
    });
    
    // Benchmark metric recording
    group.bench_function("record_histogram", |b| {
        b.iter(|| {
            observability.metrics().record(
                "test.histogram",
                black_box(123.45),
                &[("tag1", "value1"), ("tag2", "value2")]
            );
        });
    });
    
    group.bench_function("record_counter", |b| {
        b.iter(|| {
            observability.metrics().increment(
                "test.counter",
                &[("status", "success")]
            );
        });
    });
    
    // Benchmark metric snapshot
    group.bench_function("metrics_snapshot", |b| {
        // Populate some metrics first
        for i in 0..100 {
            observability.metrics().record(
                &format!("test.metric.{}", i),
                i as f64,
                &[]
            );
        }
        
        b.iter(|| {
            black_box(observability.metrics().snapshot());
        });
    });
    
    group.finish();
}

fn benchmark_trace_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("trace_serialization");
    
    // Create traces of different sizes
    let small_trace = create_trace(10);
    let medium_trace = create_trace(100);
    let large_trace = create_trace(1000);
    
    group.bench_with_input(
        BenchmarkId::new("to_json", "10_spans"),
        &small_trace,
        |b, trace| {
            b.iter(|| {
                black_box(trace.to_json().unwrap());
            });
        }
    );
    
    group.bench_with_input(
        BenchmarkId::new("to_json", "100_spans"),
        &medium_trace,
        |b, trace| {
            b.iter(|| {
                black_box(trace.to_json().unwrap());
            });
        }
    );
    
    group.bench_with_input(
        BenchmarkId::new("to_json", "1000_spans"),
        &large_trace,
        |b, trace| {
            b.iter(|| {
                black_box(trace.to_json().unwrap());
            });
        }
    );
    
    group.finish();
}

// Helper functions

fn create_mock_element() -> terminator::UIElement {
    // This would create a mock element for testing
    // For now, we'll skip the actual implementation
    unimplemented!("Mock element creation not implemented for benchmark")
}

fn create_mock_observable_element(observability: &TerminatorObservability) -> ObservableUIElement {
    // This would create a mock observable element
    unimplemented!("Mock observable element creation not implemented for benchmark")
}

fn create_trace(span_count: usize) -> terminator_observability::trace::Trace {
    use terminator_observability::trace::{Trace, Span, SpanStatus};
    use std::collections::HashMap;
    
    let mut trace = Trace {
        id: uuid::Uuid::new_v4(),
        task_name: "benchmark_task".to_string(),
        start_time: chrono::Utc::now(),
        duration: Duration::from_secs(span_count as u64),
        spans: Vec::with_capacity(span_count),
        metadata: HashMap::new(),
    };
    
    for i in 0..span_count {
        let mut span = Span::new(
            format!("operation_{}", i),
            Duration::from_millis(i as u64),
        );
        span.complete(
            Duration::from_millis(10),
            SpanStatus::Ok,
        );
        span.add_attribute("index", i as i64);
        span.add_attribute("type", "benchmark");
        
        trace.spans.push(span);
    }
    
    trace
}

criterion_group!(
    benches,
    benchmark_decorator_overhead,
    benchmark_session_overhead,
    benchmark_metrics_collection,
    benchmark_trace_serialization
);
criterion_main!(benches);