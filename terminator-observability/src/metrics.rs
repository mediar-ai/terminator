//! Metrics collection and export

use crate::{context::Config, error::Result, Error};
use dashmap::DashMap;
use metrics::{counter, gauge, histogram, Key, KeyName, Label, Recorder, Unit};
use metrics_util::{
    layers::{Layer, PrefixLayer},
    registry::{Registry, Storage},
};
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Metrics collector for gathering performance data
#[derive(Debug, Clone)]
pub struct MetricsCollector {
    recorder: Arc<TerminatorRecorder>,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new(config: &Config) -> Result<Self> {
        let recorder = Arc::new(TerminatorRecorder::new());

        // Install as global recorder
        metrics::set_boxed_recorder(Box::new(recorder.clone()))
            .map_err(|_| Error::MetricsError("Failed to set metrics recorder".to_string()))?;

        Ok(Self { recorder })
    }

    /// Record a metric value
    pub fn record(&self, name: &str, value: f64, tags: &[(&str, &str)]) {
        let labels: Vec<Label> = tags.iter().map(|(k, v)| Label::new(*k, *v)).collect();

        histogram!(name, value, labels);
    }

    /// Record a counter increment
    pub fn increment(&self, name: &str, tags: &[(&str, &str)]) {
        let labels: Vec<Label> = tags.iter().map(|(k, v)| Label::new(*k, *v)).collect();

        counter!(name, 1, labels);
    }

    /// Set a gauge value
    pub fn gauge(&self, name: &str, value: f64, tags: &[(&str, &str)]) {
        let labels: Vec<Label> = tags.iter().map(|(k, v)| Label::new(*k, *v)).collect();

        gauge!(name, value, labels);
    }

    /// Get current metric values
    pub fn snapshot(&self) -> MetricsSnapshot {
        self.recorder.snapshot()
    }

    /// Flush all metrics
    pub fn flush(&self) -> Result<()> {
        // TODO: Implement metric export
        Ok(())
    }
}

/// Internal recorder implementation
#[derive(Debug)]
struct TerminatorRecorder {
    counters: Arc<DashMap<Key, f64>>,
    gauges: Arc<DashMap<Key, f64>>,
    histograms: Arc<DashMap<Key, HistogramData>>,
}

impl TerminatorRecorder {
    fn new() -> Self {
        Self {
            counters: Arc::new(DashMap::new()),
            gauges: Arc::new(DashMap::new()),
            histograms: Arc::new(DashMap::new()),
        }
    }

    fn snapshot(&self) -> MetricsSnapshot {
        let mut snapshot = MetricsSnapshot::default();

        // Collect counters
        for entry in self.counters.iter() {
            snapshot
                .counters
                .insert(entry.key().clone(), *entry.value());
        }

        // Collect gauges
        for entry in self.gauges.iter() {
            snapshot.gauges.insert(entry.key().clone(), *entry.value());
        }

        // Collect histograms
        for entry in self.histograms.iter() {
            snapshot
                .histograms
                .insert(entry.key().clone(), entry.value().clone());
        }

        snapshot
    }
}

impl Recorder for TerminatorRecorder {
    fn describe_counter(&self, _key: KeyName, _unit: Option<Unit>, _description: &'static str) {
        // Not implemented for now
    }

    fn describe_gauge(&self, _key: KeyName, _unit: Option<Unit>, _description: &'static str) {
        // Not implemented for now
    }

    fn describe_histogram(&self, _key: KeyName, _unit: Option<Unit>, _description: &'static str) {
        // Not implemented for now
    }

    fn register_counter(&self, key: &Key) -> metrics::Counter {
        metrics::Counter::from_arc(Arc::new(CounterHandle {
            key: key.clone(),
            counters: self.counters.clone(),
        }))
    }

    fn register_gauge(&self, key: &Key) -> metrics::Gauge {
        metrics::Gauge::from_arc(Arc::new(GaugeHandle {
            key: key.clone(),
            gauges: self.gauges.clone(),
        }))
    }

    fn register_histogram(&self, key: &Key) -> metrics::Histogram {
        metrics::Histogram::from_arc(Arc::new(HistogramHandle {
            key: key.clone(),
            histograms: self.histograms.clone(),
        }))
    }
}

/// Counter handle implementation
struct CounterHandle {
    key: Key,
    counters: Arc<DashMap<Key, f64>>,
}

impl metrics::CounterFn for CounterHandle {
    fn increment(&self, value: u64) {
        self.counters
            .entry(self.key.clone())
            .and_modify(|v| *v += value as f64)
            .or_insert(value as f64);
    }

    fn absolute(&self, value: u64) {
        self.counters.insert(self.key.clone(), value as f64);
    }
}

/// Gauge handle implementation
struct GaugeHandle {
    key: Key,
    gauges: Arc<DashMap<Key, f64>>,
}

impl metrics::GaugeFn for GaugeHandle {
    fn increment(&self, value: f64) {
        self.gauges
            .entry(self.key.clone())
            .and_modify(|v| *v += value)
            .or_insert(value);
    }

    fn decrement(&self, value: f64) {
        self.gauges
            .entry(self.key.clone())
            .and_modify(|v| *v -= value)
            .or_insert(-value);
    }

    fn set(&self, value: f64) {
        self.gauges.insert(self.key.clone(), value);
    }
}

/// Histogram handle implementation
struct HistogramHandle {
    key: Key,
    histograms: Arc<DashMap<Key, HistogramData>>,
}

impl metrics::HistogramFn for HistogramHandle {
    fn record(&self, value: f64) {
        self.histograms
            .entry(self.key.clone())
            .and_modify(|h| h.record(value))
            .or_insert_with(|| {
                let mut h = HistogramData::new();
                h.record(value);
                h
            });
    }
}

/// Histogram data storage
#[derive(Debug, Clone)]
pub struct HistogramData {
    count: u64,
    sum: f64,
    min: f64,
    max: f64,
    values: Vec<f64>,
}

impl HistogramData {
    fn new() -> Self {
        Self {
            count: 0,
            sum: 0.0,
            min: f64::MAX,
            max: f64::MIN,
            values: Vec::new(),
        }
    }

    fn record(&mut self, value: f64) {
        self.count += 1;
        self.sum += value;
        self.min = self.min.min(value);
        self.max = self.max.max(value);
        self.values.push(value);

        // Keep only last 1000 values to prevent unbounded growth
        if self.values.len() > 1000 {
            self.values.remove(0);
        }
    }

    /// Calculate mean value
    pub fn mean(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum / self.count as f64
        }
    }

    /// Calculate percentile
    pub fn percentile(&self, p: f64) -> f64 {
        if self.values.is_empty() {
            return 0.0;
        }

        let mut sorted = self.values.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let index = ((p / 100.0) * (sorted.len() - 1) as f64).round() as usize;
        sorted[index]
    }
}

/// Snapshot of current metrics
#[derive(Debug, Default)]
pub struct MetricsSnapshot {
    /// Counter values
    pub counters: DashMap<Key, f64>,
    /// Gauge values
    pub gauges: DashMap<Key, f64>,
    /// Histogram data
    pub histograms: DashMap<Key, HistogramData>,
}

/// Metric value types
#[derive(Debug, Clone)]
pub enum MetricValue {
    /// Counter value
    Counter(f64),
    /// Gauge value
    Gauge(f64),
    /// Histogram statistics
    Histogram {
        count: u64,
        sum: f64,
        min: f64,
        max: f64,
        mean: f64,
        p50: f64,
        p95: f64,
        p99: f64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_histogram_data() {
        let mut hist = HistogramData::new();
        hist.record(1.0);
        hist.record(2.0);
        hist.record(3.0);

        assert_eq!(hist.count, 3);
        assert_eq!(hist.sum, 6.0);
        assert_eq!(hist.min, 1.0);
        assert_eq!(hist.max, 3.0);
        assert_eq!(hist.mean(), 2.0);
    }
}
