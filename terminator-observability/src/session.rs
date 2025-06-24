//! Session management for tracking automation tasks

use crate::{
    context::ObservabilityContext,
    telemetry::{SpanBuilder, SpanKind},
    trace::{Span, Trace},
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Represents an active observability session
#[derive(Debug)]
pub struct Session {
    /// Unique session ID
    pub id: Uuid,
    /// Session name/task name
    pub name: String,
    /// Session start time
    pub start_time: Instant,
    /// Metadata associated with the session
    pub metadata: Mutex<HashMap<String, serde_json::Value>>,
    /// Context for observability
    context: Arc<ObservabilityContext>,
    /// Spans collected in this session
    spans: Mutex<Vec<Span>>,
    /// Human baseline for comparison
    baseline: Option<HumanBaseline>,
}

impl Session {
    /// Create a new session
    pub fn new(name: String, context: Arc<ObservabilityContext>) -> Self {
        let id = Uuid::new_v4();

        // Record session start
        context.record_metric("terminator.session.started", 1.0, &[("task_name", &name)]);

        Self {
            id,
            name,
            start_time: Instant::now(),
            metadata: Mutex::new(HashMap::new()),
            context,
            spans: Mutex::new(Vec::new()),
            baseline: None,
        }
    }

    /// Add metadata to the session
    pub fn add_metadata<K, V>(&self, key: K, value: V)
    where
        K: Into<String>,
        V: Serialize,
    {
        if let Ok(json_value) = serde_json::to_value(value) {
            self.metadata.lock().insert(key.into(), json_value);
        }
    }

    /// Set the human baseline for comparison
    pub fn with_baseline(mut self, baseline: HumanBaseline) -> Self {
        self.baseline = Some(baseline);
        self
    }

    /// Create a new span within this session
    pub fn create_span(&self, name: &str) -> SpanBuilder {
        self.context
            .create_span(name)
            .with_attribute("session.id", self.id.to_string())
            .with_attribute("session.name", &self.name)
    }

    /// Record a completed span
    pub fn record_span(&self, span: Span) {
        self.spans.lock().push(span);
    }

    /// Complete the session and generate a report
    pub fn complete(self) -> SessionReport {
        let duration = self.start_time.elapsed();
        let spans = self.spans.into_inner();

        // Record session completion
        self.context.record_metric(
            "terminator.session.duration",
            duration.as_millis() as f64,
            &[("task_name", &self.name)],
        );

        // Calculate metrics
        let action_count = spans.len();
        let error_count = spans.iter().filter(|s| s.status.is_error()).count();
        let success_rate = if action_count > 0 {
            (action_count - error_count) as f64 / action_count as f64
        } else {
            1.0
        };

        // Compare with baseline if available
        let (efficiency_ratio, accuracy_score) = if let Some(baseline) = &self.baseline {
            let efficiency = baseline.average_duration.as_secs_f64() / duration.as_secs_f64();
            let accuracy = success_rate; // Simple accuracy based on success rate

            self.context.record_metric(
                "terminator.baseline.efficiency_ratio",
                efficiency,
                &[("task_name", &self.name)],
            );

            (efficiency, accuracy)
        } else {
            (1.0, success_rate)
        };

        // Create trace
        let trace = Trace {
            id: self.id,
            task_name: self.name.clone(),
            start_time: chrono::Utc::now() - chrono::Duration::from_std(duration).unwrap(),
            duration,
            spans,
            metadata: self.metadata.into_inner(),
        };

        // Store trace if storage is available
        #[cfg(feature = "sqlite")]
        if let Some(storage) = self.context.storage() {
            if let Err(e) = futures::executor::block_on(storage.save_trace(&trace)) {
                tracing::error!("Failed to save trace: {}", e);
            }
        }

        SessionReport {
            session_id: self.id,
            task_name: self.name,
            duration,
            action_count,
            error_count,
            success_rate,
            efficiency_ratio,
            accuracy_score,
            trace,
            baseline: self.baseline,
        }
    }
}

/// Report generated after completing a session
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionReport {
    /// Session ID
    pub session_id: Uuid,
    /// Task name
    pub task_name: String,
    /// Total duration
    pub duration: Duration,
    /// Number of actions performed
    pub action_count: usize,
    /// Number of errors encountered
    pub error_count: usize,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
    /// Efficiency compared to human baseline
    pub efficiency_ratio: f64,
    /// Accuracy score (0.0 to 1.0)
    pub accuracy_score: f64,
    /// Full trace data
    pub trace: Trace,
    /// Human baseline if available
    pub baseline: Option<HumanBaseline>,
}

impl SessionReport {
    /// Check if the automation was faster than human
    pub fn is_faster_than_human(&self) -> bool {
        self.efficiency_ratio > 1.0
    }

    /// Get the percentage improvement over human
    pub fn improvement_percentage(&self) -> f64 {
        (self.efficiency_ratio - 1.0) * 100.0
    }

    /// Generate a summary string
    pub fn summary(&self) -> String {
        format!(
            "Task '{}' completed in {:?} with {:.1}% success rate. {}",
            self.task_name,
            self.duration,
            self.success_rate * 100.0,
            if self.is_faster_than_human() {
                format!(
                    "{:.0}% faster than human baseline",
                    self.improvement_percentage()
                )
            } else {
                format!(
                    "{:.0}% slower than human baseline",
                    -self.improvement_percentage()
                )
            }
        )
    }
}

/// Human baseline data for comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanBaseline {
    /// Task name
    pub task_name: String,
    /// Average duration for human to complete task
    pub average_duration: Duration,
    /// Number of samples
    pub sample_count: usize,
    /// Individual action timings
    pub actions: Vec<BaselineAction>,
    /// Recording date
    pub recorded_at: chrono::DateTime<chrono::Utc>,
}

impl HumanBaseline {
    /// Create a new baseline
    pub fn new(task_name: String, average_duration: Duration) -> Self {
        Self {
            task_name,
            average_duration,
            sample_count: 1,
            actions: Vec::new(),
            recorded_at: chrono::Utc::now(),
        }
    }

    /// Load baseline from file
    pub fn load(path: &str) -> crate::Result<Self> {
        let data = std::fs::read_to_string(path)?;
        let baseline = serde_json::from_str(&data)?;
        Ok(baseline)
    }

    /// Save baseline to file
    pub fn save(&self, path: &str) -> crate::Result<()> {
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(path, data)?;
        Ok(())
    }
}

/// Individual action in human baseline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineAction {
    /// Action name
    pub name: String,
    /// Average duration
    pub duration: Duration,
    /// Action type
    pub action_type: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_report_summary() {
        let report = SessionReport {
            session_id: Uuid::new_v4(),
            task_name: "test_task".to_string(),
            duration: Duration::from_secs(10),
            action_count: 10,
            error_count: 1,
            success_rate: 0.9,
            efficiency_ratio: 1.5,
            accuracy_score: 0.9,
            trace: Trace {
                id: Uuid::new_v4(),
                task_name: "test_task".to_string(),
                start_time: chrono::Utc::now(),
                duration: Duration::from_secs(10),
                spans: Vec::new(),
                metadata: HashMap::new(),
            },
            baseline: None,
        };

        assert!(report.is_faster_than_human());
        assert_eq!(report.improvement_percentage(), 50.0);
        assert!(report.summary().contains("50% faster"));
    }
}
