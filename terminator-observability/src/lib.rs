//! # Terminator Observability
//!
//! Comprehensive observability for computer use automation agents built with Terminator SDK.
//!
//! ## Overview
//!
//! This crate provides production-ready observability features including:
//! - Automatic tracing of all SDK operations
//! - Performance metrics collection
//! - Human baseline comparison
//! - OpenTelemetry integration
//! - Real-time monitoring capabilities
//!
//! ## Quick Start
//!
//! ```rust
//! use terminator_observability::prelude::*;
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Initialize with default configuration
//! let observability = TerminatorObservability::builder()
//!     .with_service_name("my-automation")
//!     .build()?;
//!
//! // Create an observable desktop
//! let desktop = observability.create_desktop()?;
//!
//! // Start a session
//! let session = desktop.start_session("task_name");
//!
//! // Your automation code here...
//! # Ok(())
//! # }
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(
    missing_docs,
    rust_2018_idioms,
    unreachable_pub,
    missing_debug_implementations
)]

use std::sync::Arc;

pub mod context;
pub mod decorator;
pub mod error;
pub mod metrics;
pub mod session;
pub mod telemetry;
pub mod trace;

// Optional modules
#[cfg(feature = "sqlite")]
pub mod storage;

#[cfg(feature = "web-dashboard")]
#[cfg_attr(docsrs, doc(cfg(feature = "web-dashboard")))]
pub mod dashboard;

// Re-exports
pub use context::ObservabilityContext;
pub use decorator::{ObservableDesktop, ObservableLocator, ObservableUIElement};
pub use error::{Error, Result};
pub use session::{Session, SessionReport, HumanBaseline, BaselineAction};
pub use trace::{Span, Trace};

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::{
        Error, ObservabilityContext, ObservableDesktop, Result, Session, SessionReport,
        TerminatorObservability, TerminatorObservabilityBuilder, HumanBaseline,
    };
    
    pub use crate::metrics::{MetricValue, MetricsCollector};
    pub use crate::telemetry::{SpanKind, SpanStatus};
}

/// Main entry point for Terminator Observability
#[derive(Debug)]
pub struct TerminatorObservability {
    context: Arc<ObservabilityContext>,
}

impl TerminatorObservability {
    /// Create a new builder for configuring observability
    pub fn builder() -> TerminatorObservabilityBuilder {
        TerminatorObservabilityBuilder::default()
    }

    /// Create an observable desktop instance
    pub fn create_desktop(&self) -> Result<ObservableDesktop> {
        let desktop = terminator::Desktop::new_default()
            .map_err(|e| Error::TerminatorError(e.to_string()))?;
        
        Ok(ObservableDesktop::new(desktop, self.context.clone()))
    }

    /// Create an observable desktop with custom configuration
    pub fn create_desktop_with(
        &self,
        use_background_apps: bool,
        activate_app: bool,
    ) -> Result<ObservableDesktop> {
        let desktop = terminator::Desktop::new(use_background_apps, activate_app)
            .map_err(|e| Error::TerminatorError(e.to_string()))?;
        
        Ok(ObservableDesktop::new(desktop, self.context.clone()))
    }

    /// Wrap an existing desktop instance
    pub fn wrap_desktop(&self, desktop: terminator::Desktop) -> ObservableDesktop {
        ObservableDesktop::new(desktop, self.context.clone())
    }

    /// Get the metrics collector
    pub fn metrics(&self) -> &metrics::MetricsCollector {
        self.context.metrics()
    }

    /// Get the observability context
    pub fn context(&self) -> &Arc<ObservabilityContext> {
        &self.context
    }

    /// Shutdown observability and flush all pending data
    pub async fn shutdown(self) -> Result<()> {
        self.context.shutdown().await
    }
}

/// Builder for configuring TerminatorObservability
#[derive(Debug, Default)]
pub struct TerminatorObservabilityBuilder {
    service_name: Option<String>,
    service_version: Option<String>,
    otlp_endpoint: Option<String>,
    sampling_ratio: Option<f64>,
    metrics_interval: Option<std::time::Duration>,
    enable_stdout_exporter: bool,
}

impl TerminatorObservabilityBuilder {
    /// Set the service name
    pub fn with_service_name(mut self, name: impl Into<String>) -> Self {
        self.service_name = Some(name.into());
        self
    }

    /// Set the service version
    pub fn with_service_version(mut self, version: impl Into<String>) -> Self {
        self.service_version = Some(version.into());
        self
    }

    /// Set the OTLP endpoint for exporting telemetry
    #[cfg(feature = "otlp")]
    pub fn with_otlp_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.otlp_endpoint = Some(endpoint.into());
        self
    }

    /// Set the trace sampling ratio (0.0 to 1.0)
    pub fn with_sampling_ratio(mut self, ratio: f64) -> Self {
        self.sampling_ratio = Some(ratio.clamp(0.0, 1.0));
        self
    }

    /// Set the metrics collection interval
    pub fn with_metrics_interval(mut self, interval: std::time::Duration) -> Self {
        self.metrics_interval = Some(interval);
        self
    }

    /// Enable stdout exporter for debugging
    pub fn with_stdout_exporter(mut self, enable: bool) -> Self {
        self.enable_stdout_exporter = enable;
        self
    }

    /// Build the TerminatorObservability instance
    pub fn build(self) -> Result<TerminatorObservability> {
        let config = context::Config {
            service_name: self.service_name.unwrap_or_else(|| "terminator-automation".to_string()),
            service_version: self.service_version.unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string()),
            otlp_endpoint: self.otlp_endpoint,
            sampling_ratio: self.sampling_ratio.unwrap_or(1.0),
            metrics_interval: self.metrics_interval.unwrap_or(std::time::Duration::from_secs(10)),
            enable_stdout_exporter: self.enable_stdout_exporter,
        };

        let context = Arc::new(ObservabilityContext::new(config)?);

        Ok(TerminatorObservability { context })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_defaults() {
        let builder = TerminatorObservability::builder();
        // Just ensure it compiles and can be created
        let _ = builder.build();
    }

    #[test]
    fn test_builder_with_config() {
        let result = TerminatorObservability::builder()
            .with_service_name("test-service")
            .with_service_version("1.0.0")
            .with_sampling_ratio(0.5)
            .build();

        assert!(result.is_ok());
    }
}