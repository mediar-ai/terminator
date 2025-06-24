//! Observability context management

use crate::{error::Result, metrics::MetricsCollector, telemetry::TelemetryProvider};
use std::sync::Arc;
use std::time::Duration;

/// Configuration for the observability system
#[derive(Debug, Clone)]
pub struct Config {
    /// Service name for telemetry
    pub service_name: String,
    /// Service version
    pub service_version: String,
    /// OTLP endpoint for exporting telemetry
    pub otlp_endpoint: Option<String>,
    /// Trace sampling ratio (0.0 to 1.0)
    pub sampling_ratio: f64,
    /// Metrics collection interval
    pub metrics_interval: Duration,
    /// Enable stdout exporter for debugging
    pub enable_stdout_exporter: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            service_name: "terminator-automation".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            otlp_endpoint: None,
            sampling_ratio: 1.0,
            metrics_interval: Duration::from_secs(10),
            enable_stdout_exporter: false,
        }
    }
}

/// Central observability context that coordinates all components
#[derive(Debug)]
pub struct ObservabilityContext {
    config: Config,
    telemetry: TelemetryProvider,
    metrics: MetricsCollector,
    #[cfg(feature = "sqlite")]
    storage: Option<Arc<crate::storage::Storage>>,
}

impl ObservabilityContext {
    /// Create a new observability context with the given configuration
    pub fn new(config: Config) -> Result<Self> {
        // Initialize telemetry provider
        let telemetry = TelemetryProvider::new(&config)?;
        
        // Initialize metrics collector
        let metrics = MetricsCollector::new(&config)?;
        
        // Initialize storage if enabled
        #[cfg(feature = "sqlite")]
        let storage = if let Ok(db_path) = std::env::var("TERMINATOR_OBSERVABILITY_DB") {
            Some(Arc::new(crate::storage::Storage::new(&db_path)?))
        } else {
            None
        };
        
        Ok(Self {
            config,
            telemetry,
            metrics,
            #[cfg(feature = "sqlite")]
            storage,
        })
    }
    
    /// Get the configuration
    pub fn config(&self) -> &Config {
        &self.config
    }
    
    /// Get the telemetry provider
    pub fn telemetry(&self) -> &TelemetryProvider {
        &self.telemetry
    }
    
    /// Get the metrics collector
    pub fn metrics(&self) -> &MetricsCollector {
        &self.metrics
    }
    
    /// Get the storage backend
    #[cfg(feature = "sqlite")]
    pub fn storage(&self) -> Option<&Arc<crate::storage::Storage>> {
        self.storage.as_ref()
    }
    
    /// Create a new root span
    pub fn create_span(&self, name: &str) -> crate::telemetry::SpanBuilder {
        self.telemetry.span(name)
    }
    
    /// Record a metric value
    pub fn record_metric(&self, name: &str, value: f64, tags: &[(&str, &str)]) {
        self.metrics.record(name, value, tags);
    }
    
    /// Shutdown the observability system and flush all data
    pub async fn shutdown(self) -> Result<()> {
        // Flush metrics
        self.metrics.flush()?;
        
        // Shutdown telemetry
        self.telemetry.shutdown()?;
        
        // Close storage
        #[cfg(feature = "sqlite")]
        if let Some(storage) = self.storage {
            storage.close().await?;
        }
        
        Ok(())
    }
}

/// Global context holder for convenience
static GLOBAL_CONTEXT: once_cell::sync::OnceCell<Arc<ObservabilityContext>> = once_cell::sync::OnceCell::new();

impl ObservabilityContext {
    /// Set the global context (can only be called once)
    pub fn set_global(context: Arc<ObservabilityContext>) -> Result<()> {
        GLOBAL_CONTEXT
            .set(context)
            .map_err(|_| crate::Error::ConfigError("Global context already initialized".to_string()))
    }
    
    /// Get the global context
    pub fn global() -> Option<&'static Arc<ObservabilityContext>> {
        GLOBAL_CONTEXT.get()
    }
    
    /// Get the global context or panic
    pub fn global_unchecked() -> &'static Arc<ObservabilityContext> {
        GLOBAL_CONTEXT.get().expect("Global observability context not initialized")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.service_name, "terminator-automation");
        assert_eq!(config.sampling_ratio, 1.0);
        assert!(!config.enable_stdout_exporter);
    }
    
    #[test]
    fn test_context_creation() {
        let config = Config::default();
        let context = ObservabilityContext::new(config);
        assert!(context.is_ok());
    }
}